//! "Gather a party": create or join a lobby, pick heroes, ready up.

use crate::art::Art;
use crate::battle::Btn;
use crate::l10n::L10n;
use crate::menu::menu_button;
use crate::net::{self, Inbox, Net, NetStatus};
use crate::session::Db;
use crate::steam::Steam;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage, STAGE_H, STAGE_W};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

pub fn plugin(app: &mut App) {
    app.init_resource::<CodeEntry>()
        .add_systems(OnEnter(AppState::Coop), enter_coop)
        .add_systems(Update, (type_code, typed_on_screen, coop_labels).run_if(in_state(AppState::Coop)))
        .add_systems(OnEnter(AppState::Coop), |mut w: Single<&mut Window>| w.ime_enabled = crate::touch::enabled())
        .add_systems(OnExit(AppState::Coop), |mut w: Single<&mut Window>| w.ime_enabled = false)
        .add_systems(OnEnter(AppState::Lobby), |mut status: ResMut<NetStatus>| status.dirty = true)
        .init_resource::<HeroRowAt>()
        .add_systems(Update, (draw_lobby, copy_code, roll_heroes).chain().run_if(in_state(AppState::Lobby)));
}

/// The most that can be typed: a Steam code is six letters, an address with its port more.
const MOST_TYPED: usize = 24;

/// The join code being typed, or the host's address.
#[derive(Resource, Default)]
pub struct CodeEntry(pub String);

#[derive(Component)]
enum CoopLabel {
    Code,
    Status,
}

#[derive(Component)]
struct LobbyRoot;

fn column(state: AppState, stage: Entity) -> impl Bundle {
    (
        DespawnOnExit(state),
        Node {
            position_type: PositionType::Absolute,
            width: px(STAGE_W),
            height: px(STAGE_H),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(20),
            ..default()
        },
        BackgroundColor(theme::SOOT),
        Pickable::IGNORE,
        ChildOf(stage),
    )
}

fn enter_coop(
    mut commands: Commands,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut entry: ResMut<CodeEntry>,
    mut status: ResMut<NetStatus>,
    steam: Option<Res<Steam>>,
    profile: Res<crate::profile::Profile>,
) {
    entry.0.clear();
    // a party given up after the line dropped says so here
    let lost = status.error.take().filter(|e| e == "coop-error-lost" || e == "coop-error-build");
    status.error = std::mem::take(&mut status.host_gone).then(|| "coop-error-host-gone".into()).or(lost);
    status.busy = false;
    let has_steam = steam.is_some();
    commands.spawn(column(AppState::Coop, *stage)).with_children(|p| {
        p.spawn(crate::menu::backdrop(&art, &assets, "bg/coop", 0.7));
        p.spawn(theme::text(l10n.t("coop-title"), &fonts.title, 72.0, theme::BONE));
        // the party's last run, as this machine hosted it, can be carried on: a lobby opens with
        // it, and the friends who come in take their seats back
        if let Some(save) = profile.load_party() {
            p.spawn((
                menu_button(l10n.tf("menu-continue-party", &[("act", (save.run.act as i32).into())]), &fonts),
                observe(|_: On<Activate>, mut commands: Commands, profile: Res<crate::profile::Profile>, steam: Option<Res<Steam>>, inbox: Option<Res<Inbox>>, mut status: ResMut<NetStatus>, mut next: ResMut<NextState<AppState>>| {
                    status.resume = profile.load_party();
                    match (steam, inbox) {
                        (Some(steam), Some(inbox)) => net::create_lobby(&steam, &mut status, &inbox),
                        _ => {
                            if crate::lan::host(&mut commands, &mut status) {
                                next.set(AppState::Lobby);
                            }
                        }
                    }
                }),
            ));
        }
        if has_steam {
            p.spawn((
                menu_button(l10n.t("coop-create"), &fonts),
                observe(|_: On<Activate>, steam: Option<Res<Steam>>, inbox: Option<Res<Inbox>>, mut status: ResMut<NetStatus>| {
                    match (steam, inbox) {
                        (Some(steam), Some(inbox)) => net::create_lobby(&steam, &mut status, &inbox),
                        _ => status.error = Some("coop-error-steam".into()),
                    }
                }),
            ));
        }
        p.spawn((
            menu_button(l10n.t("coop-create-lan"), &fonts),
            observe(|_: On<Activate>, mut commands: Commands, mut status: ResMut<NetStatus>, mut next: ResMut<NextState<AppState>>| {
                if crate::lan::host(&mut commands, &mut status) {
                    next.set(AppState::Lobby);
                }
            }),
        ));
        p.spawn((theme::text(l10n.t(if has_steam { "coop-join-hint" } else { "coop-join-hint-lan" }), &fonts.body, 24.0, theme::DIM), Node { margin: UiRect::top(px(24)), ..default() }));
        p.spawn((
            theme::panel(Node {
                width: px(520),
                height: px(84),
                border: UiRect::all(px(3)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            }),
            children![(CoopLabel::Code, theme::text("", &fonts.num, 52.0, theme::CANDLE))],
        ));
        p.spawn((
            menu_button(l10n.t(if has_steam { "coop-join" } else { "coop-join-lan" }), &fonts),
            observe(|_: On<Activate>, mut commands: Commands, steam: Option<Res<Steam>>, inbox: Option<Res<Inbox>>, entry: Res<CodeEntry>, mut status: ResMut<NetStatus>| {
                // an address has dots in it; a Steam code never does
                if entry.0.contains(['.', ':']) {
                    crate::lan::dial(&mut commands, &entry.0, &mut status);
                    return;
                }
                if entry.0.len() != 6 {
                    status.error = Some("coop-error-code".into());
                    return;
                }
                match (steam, inbox) {
                    (Some(steam), Some(inbox)) => net::join_by_code(&steam, &mut status, &inbox, &entry.0),
                    _ => status.error = Some("coop-error-steam".into()),
                }
            }),
        ));
        #[cfg(feature = "dev")]
        p.spawn((
            menu_button(l10n.t("coop-local"), &fonts),
            observe(|_: On<Activate>, mut commands: Commands, db: Res<Db>, mut next: ResMut<NextState<AppState>>| {
                net::start_offline(&mut commands, &db.0, "dev", 3);
                next.set(AppState::Lobby);
            }),
        ));
        // a party on this network, a phone's hotspot among them, found without typing its address
        p.spawn((
            menu_button(l10n.t("coop-find-lan"), &fonts),
            observe(|_: On<Activate>, mut commands: Commands, mut status: ResMut<NetStatus>, searching: Option<Res<crate::lan::Search>>| {
                if searching.is_none() {
                    crate::lan::search(&mut commands, &mut status);
                }
            }),
        ));
        p.spawn((CoopLabel::Status, theme::text("", &fonts.body, 24.0, theme::FRESH_BLOOD)));
        p.spawn((
            menu_button(l10n.t("coop-back"), &fonts),
            observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu)),
        ));
    });
}

/// What a code or an address may hold.
fn typable(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-')
}

/// The character a key stands for on the US layout, so a code or an address can be typed with
/// any keyboard layout on (a Russian one gives Cyrillic letters, which no code holds).
fn latin(code: KeyCode, shift: bool) -> Option<char> {
    use KeyCode::*;
    let letters = [KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ];
    if let Some(i) = letters.iter().position(|k| *k == code) {
        return Some((b'A' + i as u8) as char);
    }
    let digits = [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
    let pad = [Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9];
    if let Some(i) = digits.iter().position(|k| *k == code).or_else(|| pad.iter().position(|k| *k == code)) {
        return Some((b'0' + i as u8) as char);
    }
    match code {
        Period | NumpadDecimal => Some('.'),
        Semicolon if shift => Some(':'),
        Minus | NumpadSubtract => Some('-'),
        _ => None,
    }
}

use crate::platform::{clipboard_get, clipboard_set};

/// Ctrl+C in the lobby copies the party's code, or the first address friends can reach.
fn copy_code(held: Res<ButtonInput<KeyCode>>, net: Option<Res<Net>>) {
    let ctrl = held.pressed(KeyCode::ControlLeft) || held.pressed(KeyCode::ControlRight);
    if let (true, true, Some(net)) = (ctrl, held.just_pressed(KeyCode::KeyC), net) {
        if !net.code.is_empty() && net.code != "LAN" {
            clipboard_set(net.code.split("  /  ").next().unwrap_or_default());
        }
    }
}

fn type_code(mut keys: MessageReader<KeyboardInput>, held: Res<ButtonInput<KeyCode>>, mut entry: ResMut<CodeEntry>) {
    let ctrl = held.pressed(KeyCode::ControlLeft) || held.pressed(KeyCode::ControlRight);
    let shift = held.pressed(KeyCode::ShiftLeft) || held.pressed(KeyCode::ShiftRight);
    for k in keys.read() {
        if !k.state.is_pressed() {
            continue;
        }
        // Ctrl+V pastes a code or an address copied from a chat; Ctrl+C copies what is typed
        if ctrl && k.key_code == KeyCode::KeyV {
            if let Some(text) = clipboard_get() {
                entry.0 = text.chars().filter(|c| typable(*c)).map(|c| c.to_ascii_uppercase()).take(MOST_TYPED).collect();
            }
            continue;
        }
        if ctrl && k.key_code == KeyCode::KeyC {
            clipboard_set(&entry.0);
            continue;
        }
        if ctrl {
            continue;
        }
        match &k.logical_key {
            Key::Backspace => {
                entry.0.pop();
            }
            Key::Character(s) => {
                // a letter of another layout is the key's Latin letter
                let typed: Vec<char> = if s.chars().all(typable) { s.chars().collect() } else { latin(k.key_code, shift).into_iter().collect() };
                for ch in typed {
                    if entry.0.len() < MOST_TYPED {
                        entry.0.push(ch.to_ascii_uppercase());
                    }
                }
            }
            _ => {}
        }
    }
}

/// A phone's on-screen keyboard (and a desktop's input method) hands over whole text, not keys.
fn typed_on_screen(mut ime: MessageReader<bevy::window::Ime>, mut entry: ResMut<CodeEntry>) {
    for e in ime.read() {
        if let bevy::window::Ime::Commit { value, .. } = e {
            for ch in value.chars().filter(|c| typable(*c)) {
                if entry.0.len() < MOST_TYPED {
                    entry.0.push(ch.to_ascii_uppercase());
                }
            }
        }
    }
}

fn coop_labels(entry: Res<CodeEntry>, status: Res<NetStatus>, l10n: Res<L10n>, mut labels: Query<(&CoopLabel, &mut Text)>) {
    for (label, mut text) in &mut labels {
        let s = match label {
            // a code is six slots, the untyped ones shown as dots; an address is shown as typed
            CoopLabel::Code if entry.0.len() > 6 || entry.0.contains(['.', ':']) => entry.0.clone(),
            CoopLabel::Code => (0..6).map(|i| entry.0.chars().nth(i).unwrap_or('·').to_string()).collect::<Vec<_>>().join(" "),
            CoopLabel::Status => match (&status.error, status.busy) {
                (Some(key), _) => l10n.t(key),
                (None, true) => l10n.t("coop-busy"),
                _ => String::new(),
            },
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

/// A button sized for the lobby's bottom bar.
fn bar_button(label: String, fonts: &Fonts, width: f32, base: Color) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        Btn(base),
        Node { width: px(width), padding: UiRect::axes(px(16), px(14)), border: UiRect::all(px(3)), justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(base),
        BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
        children![theme::text(label, &fonts.title, 26.0, theme::BONE)],
    )
}

fn absolute(left: f32, top: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: px(left), top: px(top), ..default() }
}

/// The party sits round a fire: the seats along the top, the chosen hero large in the middle
/// with what they bring, every hero's face along the bottom to pick from.
fn draw_lobby(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    stage: Single<Entity, With<Stage>>,
    old: Query<Entity, With<LobbyRoot>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    profile: Res<crate::profile::Profile>,
    row_at: Res<HeroRowAt>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(net) = net else {
        next.set(AppState::Menu);
        return;
    };
    if !status.dirty {
        return;
    }
    status.dirty = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    let lobby = &net.node.lobby;
    let me = net.me();
    let mine = lobby.players.iter().find(|p| p.peer == me);
    let is_host = net.node.is_host();
    let frame = Color::srgb(0.42, 0.33, 0.22);

    let root = commands
        .spawn((
            LobbyRoot,
            DespawnOnExit(AppState::Lobby),
            Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() },
            BackgroundColor(theme::SOOT),
            Pickable::IGNORE,
            ChildOf(*stage),
        ))
        .id();
    commands.spawn((crate::menu::backdrop(&art, &assets, "bg/lobby", 0.8), ChildOf(root)));
    // the faces to pick from sit on a darker band, so they read over the firelight
    // fading in from above, so it reads as the ground darkening away from the fire, not a slab
    commands.spawn((
        ImageNode { color: theme::SOOT.with_alpha(0.75), ..ImageNode::new(assets.load(crate::art::path("fx/fade_down.png"))) },
        Node { position_type: PositionType::Absolute, left: px(0), bottom: px(260), width: px(STAGE_W), height: px(200), ..default() },
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: px(0), bottom: px(0), width: px(STAGE_W), height: px(260), ..default() },
        BackgroundColor(theme::SOOT.with_alpha(0.75)),
        Pickable::IGNORE,
        ChildOf(root),
    ));

    // title and the join code
    commands.spawn((
        Node { position_type: PositionType::Absolute, top: px(22), width: px(STAGE_W), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
        Pickable::IGNORE,
        ChildOf(root),
    ))
    .with_children(|p| {
        p.spawn(theme::text(l10n.t("lobby-title"), &fonts.title, 56.0, theme::BONE));
        if net.lobby.is_some() {
            p.spawn(theme::text(l10n.tf("lobby-code", &[("code", net.code.clone().into())]), &fonts.num, 34.0, theme::CANDLE));
        }
    });
    // the code, or the first address friends can reach, goes to the clipboard for a chat
    if !net.code.is_empty() && net.code != "LAN" {
        let code = net.code.split("  /  ").next().unwrap_or_default().to_string();
        commands.spawn((
            Node { position_type: PositionType::Absolute, right: px(40), top: px(30), ..default() },
            ChildOf(root),
            children![(
                menu_button(l10n.t("lobby-copy-code"), &fonts),
                observe(move |_: On<Activate>| clipboard_set(&code)),
            )],
        ));
    }

    // where friends can reach a party over the local network: under the seats, in plain sight
    if net.lobby.is_none() && net.code != "LAN" && !net.code.is_empty() {
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: px(0), top: px(240), width: px(STAGE_W), justify_content: JustifyContent::Center, ..default() },
            Pickable::IGNORE,
            ChildOf(root),
            children![theme::text(l10n.tf("lobby-address", &[("code", net.code.clone().into())]), &fonts.num, 34.0, theme::CANDLE)],
        ));
    }

    // seats: each player's face, name, hero and readiness
    let seats_top = if net.lobby.is_some() { 132.0 } else { 104.0 };
    commands.spawn((
        Node { position_type: PositionType::Absolute, top: px(seats_top), width: px(STAGE_W), column_gap: px(18), justify_content: JustifyContent::Center, ..default() },
        Pickable::IGNORE,
        ChildOf(root),
    ))
    .with_children(|row| {
        for slot in 0..nav_net::MAX_PLAYERS {
            let player = lobby.players.get(slot);
            let hero = player.and_then(|p| p.hero.as_deref());
            let color = hero.map(theme::hero_color).unwrap_or(frame);
            row.spawn((
                theme::panel_edged(
                    Node { width: px(380), height: px(112), border: UiRect::all(px(3)), padding: UiRect::all(px(8)), column_gap: px(12), align_items: AlignItems::Center, ..default() },
                    color,
                ),
                Pickable::IGNORE,
            ))
            .with_children(|seat| {
                let face = hero.and_then(|h| art.image(&assets, &format!("portraits/{h}")));
                seat.spawn((
                    Node { width: px(94), height: px(94), border: UiRect::all(px(2)), ..default() },
                    BorderColor::all(color),
                    BackgroundColor(theme::SOOT),
                    Pickable::IGNORE,
                ))
                .with_children(|f| {
                    if let Some(face) = face {
                        f.spawn((ImageNode::new(face), Node { width: percent(100), height: percent(100), ..default() }, Pickable::IGNORE));
                    }
                });
                seat.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(2), ..default() }, Pickable::IGNORE)).with_children(|col| match player {
                    Some(pl) => {
                        let mut name = pl.name.clone();
                        if pl.peer == lobby.host {
                            name.push_str(&format!("  ·  {}", l10n.t("lobby-host")));
                        }
                        col.spawn(theme::text(name, &fonts.body, 20.0, theme::BONE));
                        match hero {
                            Some(h) => col.spawn(theme::text(l10n.t(&format!("hero-{h}")), &fonts.title, 24.0, color.lighter(0.2))),
                            None => col.spawn(theme::text(l10n.t("lobby-no-hero"), &fonts.body, 18.0, theme::DIM)),
                        };
                        let state = if !pl.connected {
                            ("lobby-dropped", theme::FRESH_BLOOD)
                        } else if pl.ready {
                            ("battle-ready", theme::ROTLIGHT)
                        } else {
                            ("lobby-choosing", theme::DIM)
                        };
                        col.spawn(theme::text(l10n.t(state.0), &fonts.body, 18.0, state.1));
                    }
                    None => {
                        col.spawn(theme::text(l10n.t("lobby-empty"), &fonts.body, 20.0, theme::DIM));
                    }
                });
            });
        }
    });

    // the chosen hero, large, with what they bring
    match mine.and_then(|m| m.hero.as_ref().map(|h| (m, h))).and_then(|(m, h)| db.0.heroes.get(h).map(|d| (m, h, d))) {
        Some((m, hero, def)) => {
            let color = theme::hero_color(hero);
            let figure_h = 460.0;
            let figure_w = figure_h * art.aspect(&format!("heroes/{hero}")).unwrap_or(0.75);
            if let Some(figure) = art.image(&assets, &format!("heroes/{hero}")) {
                // standing on the ground by the fire: a soft shadow under the feet
                let feet = 222.0 + figure_h * 0.97;
                commands.spawn((
                    ImageNode { color: Color::WHITE.with_alpha(0.85), ..ImageNode::new(assets.load(crate::art::path("fx/shadow.png"))) },
                    Node { width: px(figure_w * 0.9), height: px(46), ..absolute(560.0 - figure_w * 0.45, feet - 23.0) },
                    Pickable::IGNORE,
                    ChildOf(root),
                ));
                commands.spawn((
                    ImageNode::new(figure),
                    Node { width: px(figure_w), height: px(figure_h), ..absolute(560.0 - figure_w / 2.0, 222.0) },
                    Pickable::IGNORE,
                    ChildOf(root),
                ));
            }
            commands
                .spawn((
                    theme::panel_edged(
                        Node {
                            width: px(820),
                            padding: UiRect::axes(px(28), px(20)),
                            border: UiRect::all(px(3)),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(12),
                            ..absolute(840.0, 236.0)
                        },
                        color,
                    ),
                    Pickable::IGNORE,
                    ChildOf(root),
                ))
                .with_children(|p| {
                    p.spawn(theme::text(l10n.t(&format!("hero-{hero}")), &fonts.title, 60.0, color.lighter(0.25)));
                    // the level, with what it gives told on hover: one for solo and the party alike
                    let level = profile.hero_level(hero);
                    let next = match profile.hero_progress(hero) {
                        Some((have, need)) => l10n.tf("lobby-level-next", &[("have", (have as i32).into()), ("need", (need as i32).into())]),
                        None => l10n.t("lobby-level-top"),
                    };
                    // what the next level opens, counted from the cards themselves
                    let opens = db.0.cards.values().filter(|d| d.hero == *hero && !matches!(d.rarity, nav_core::Rarity::Starter | nav_core::Rarity::Special) && nav_core::card_level(d) as u32 == level + 1).count();
                    let next = if opens > 0 { format!("{next}
{}", l10n.tf("lobby-level-opens", &[("n", ((level + 1) as i32).into()), ("k", (opens as i32).into())])) } else { next };
                    p.spawn(theme::text(l10n.n("lobby-hp", def.max_hp), &fonts.num, 26.0, theme::CANDLE));
                    // a text takes no pointer (see `theme::text`): the hover is the box round it
                    p.spawn((
                        Node::default(),
                        bevy::picking::hover::Hovered::default(),
                        crate::run_ui::CardHints::below(vec![(l10n.n("lobby-level", level as i32), format!("{}

{next}", l10n.t("lobby-level-help")))]),
                    ))
                    .with_child(theme::text(format!("{} (?)", l10n.n("lobby-level", level as i32)), &fonts.num, 26.0, theme::CANDLE));
                    // what the hero's own mechanic does, told on hover: nothing else says it before a fight
                    p.spawn((
                        Node::default(),
                        bevy::picking::hover::Hovered::default(),
                        crate::run_ui::CardHints::below(vec![(l10n.t(&format!("mech-{:?}-name", def.mechanic)), l10n.t(&format!("mech-{:?}", def.mechanic)))]),
                    ))
                    .with_child(theme::text(format!("{} (?)", l10n.t(&format!("hero-{hero}-desc"))), &fonts.body, 22.0, theme::BONE));
                    p.spawn((theme::text(l10n.t("lobby-relic"), &fonts.body, 18.0, theme::DIM), Node { margin: UiRect::top(px(6)), ..default() }));
                    // the shrine lets a hero start with another relic: each choice is a button
                    let shrine = profile.building("shrine").min(2) as usize;
                    let choices = std::iter::once(&def.relic).chain(def.alt_relics.iter().take(shrine));
                    let choices: Vec<&String> = choices.collect();
                    // however many there are, they are a row of pictures that wraps; what each does
                    // is told on hover, and the chosen one's in full beneath them
                    if choices.len() > 1 {
                        p.spawn((Node { flex_wrap: FlexWrap::Wrap, column_gap: px(8), row_gap: px(8), ..default() }, Pickable::IGNORE)).with_children(|row| {
                            for (k, relic) in choices.iter().enumerate() {
                                let picked = m.starter as usize == k;
                                let base = if picked { theme::SWAMP.with_alpha(0.9) } else { theme::SOOT.with_alpha(0.7) };
                                let hint = crate::run_ui::CardHints::new(vec![(l10n.t(&format!("relic-{relic}")), l10n.t(&format!("relic-{relic}-desc")))]);
                                let mut chip = row.spawn((
                                    Button,
                                    Hovered::default(),
                                    Btn(base),
                                    hint,
                                    Node { width: px(72), height: px(72), padding: UiRect::all(px(4)), border: UiRect::all(px(2)), ..default() },
                                    BackgroundColor(base),
                                    BorderColor::all(if picked { theme::BONE } else { Color::srgb(0.42, 0.33, 0.22) }),
                                ));
                                chip.with_child((art.node(&assets, &format!("relics/{relic}"), theme::PEAT), Node { width: percent(100), height: percent(100), ..default() }, Pickable::IGNORE));
                                chip.observe(move |_: On<Activate>, mut net: ResMut<Net>| net.node.pick_starter(k as u8));
                            }
                        });
                    }
                    if let Some(relic) = choices.get(m.starter as usize).or(choices.first()) {
                        p.spawn((Node { column_gap: px(14), align_items: AlignItems::Center, padding: UiRect::all(px(6)), ..default() }, BackgroundColor(theme::SOOT.with_alpha(0.7)), Pickable::IGNORE)).with_children(|r| {
                            r.spawn((art.node(&assets, &format!("relics/{relic}"), theme::PEAT), Node { width: px(64), height: px(64), flex_shrink: 0.0, ..default() }, Pickable::IGNORE));
                            r.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(2), flex_shrink: 1.0, ..default() }, Pickable::IGNORE)).with_children(|t| {
                                t.spawn(theme::text(l10n.t(&format!("relic-{relic}")), &fonts.title, 22.0, theme::CANDLE));
                                t.spawn(theme::text(l10n.t(&format!("relic-{relic}-desc")), &fonts.body, 18.0, theme::BONE));
                            });
                        });
                    }
                });
        }
        None => {
            commands.spawn((
                theme::text(l10n.t("lobby-pick"), &fonts.title, 52.0, theme::BONE),
                Node { position_type: PositionType::Absolute, top: px(400), width: px(STAGE_W), justify_content: JustifyContent::Center, ..default() },
                TextLayout::justify(Justify::Center),
                ChildOf(root),
            ));
        }
    }

    // every hero's face: a hero another player took is dimmed and says whose it is. The row is
    // narrower than the screen and rolls sideways, by the wheel or by the slider under it
    let window = commands
        .spawn((
            HeroRow,
            Node {
                position_type: PositionType::Absolute,
                top: px(670),
                left: px((STAGE_W - HERO_ROW_W) / 2.0),
                width: px(HERO_ROW_W),
                height: px(250),
                overflow: Overflow::scroll_x(),
                ..default()
            },
            ScrollPosition(Vec2::new(row_at.0, 0.0)),
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        HeroSlider,
        Button,
        Hovered::default(),
        Node {
            position_type: PositionType::Absolute,
            top: px(930),
            left: px((STAGE_W - HERO_ROW_W) / 2.0),
            width: px(HERO_ROW_W),
            height: px(14),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(7)),
            ..default()
        },
        BackgroundColor(theme::SOOT),
        BorderColor::all(theme::DIM.with_alpha(0.5)),
        children![(
            HeroThumb,
            Node { position_type: PositionType::Absolute, top: px(1), height: px(10), width: percent(30), left: percent(0), border_radius: BorderRadius::all(px(5)), ..default() },
            BackgroundColor(theme::CANDLE.with_alpha(0.8)),
            Pickable::IGNORE,
        )],
        ChildOf(root),
    ));
    commands.spawn((
        Node { column_gap: px(22), align_items: AlignItems::End, padding: UiRect::axes(px(12), px(4)), flex_shrink: 0.0, ..default() },
        Pickable { should_block_lower: false, is_hoverable: false },
        ChildOf(window),
    ))
    .with_children(|row| {
        let order: Vec<&String> = crate::library::HEROES.iter().filter_map(|h| db.0.heroes.get_key_value(*h).map(|(k, _)| k)).chain(db.0.heroes.keys().filter(|k| !crate::library::HEROES.contains(&k.as_str()))).collect();
        for hero in order {
            let owner = lobby.players.iter().find(|pl| pl.peer != me && pl.hero.as_deref() == Some(hero.as_str()));
            let picked = mine.is_some_and(|m| m.hero.as_deref() == Some(hero.as_str()));
            let color = theme::hero_color(hero);
            let size = if picked { 168.0 } else { 148.0 };
            let id = hero.clone();
            let mut button = row.spawn((
                Button,
                Hovered::default(),
                Btn(theme::SOOT),
                Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(6), padding: UiRect::all(px(4)), flex_shrink: 0.0, ..default() },
                BackgroundColor(theme::SOOT),
            ));
            button.with_children(|b| {
                let (mut face, _) = art.node(&assets, &format!("portraits/{hero}"), color);
                if owner.is_some() {
                    face.color = Color::srgb(0.3, 0.3, 0.3);
                }
                b.spawn((
                    face,
                    Node { width: px(size), height: px(size), border: UiRect::all(px(if picked { 5 } else { 3 })), ..default() },
                    BorderColor::all(if picked { theme::BONE } else { color }),
                    Pickable::IGNORE,
                ));
                b.spawn(theme::text(l10n.t(&format!("hero-{hero}")), &fonts.title, 22.0, if owner.is_some() { theme::DIM } else { color.lighter(0.3) }));
                let under = match owner {
                    Some(o) => l10n.tf("lobby-taken", &[("name", o.name.clone().into())]),
                    None => l10n.n("lobby-level", profile.hero_level(hero) as i32),
                };
                b.spawn(theme::text(under, &fonts.body, 16.0, theme::DIM));
            });
            if owner.is_none() {
                button.observe(move |_: On<Activate>, mut net: ResMut<Net>, profile: Res<crate::profile::Profile>| {
                    let level = profile.hero_level(&id);
                    let hero = if picked { None } else { Some(id.clone()) };
                    net.node.pick_hero(hero, level);
                });
            }
        }
    });

    // the bottom bar: leave on the left, the run's terms in the middle, setting out on the right
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(50),
            right: px(50),
            bottom: px(26),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::End,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
    ))
    .with_children(|bar| {
        bar.spawn((
            bar_button(l10n.t("lobby-leave"), &fonts, 330.0, theme::PEAT),
            observe(|_: On<Activate>, mut commands: Commands, steam: Option<Res<Steam>>, net: Res<Net>, mut next: ResMut<NextState<AppState>>| {
                net::leave(&mut commands, steam.as_deref(), &net);
                next.set(AppState::Menu);
            }),
        ));
        bar.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(8), ..default() }, Pickable::IGNORE)).with_children(|mid| {
            if is_host && !lobby.can_start() {
                mid.spawn(theme::text(l10n.t("lobby-waiting"), &fonts.body, 18.0, theme::DIM));
            }
            mid.spawn((Node { column_gap: px(18), align_items: AlignItems::Center, ..default() }, Pickable::IGNORE)).with_children(|row| {
                // Circle of Nav: the host may pick any circle they have opened
                let circle = lobby.circle;
                let max = profile.circle_unlocked;
                let step = |row: &mut ChildSpawnerCommands, label: &str, to: Option<u8>| {
                    let base = if to.is_some() { theme::PEAT } else { theme::SOOT };
                    let mut e = row.spawn((
                        Button,
                        Hovered::default(),
                        Btn(base),
                        Node { width: px(48), height: px(48), border: UiRect::all(px(2)), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
                        BackgroundColor(base),
                        BorderColor::all(frame),
                        children![theme::text(label, &fonts.num, 28.0, if to.is_some() { theme::BONE } else { theme::DIM })],
                    ));
                    if let Some(to) = to {
                        e.observe(move |_: On<Activate>, mut net: ResMut<Net>| net.node.set_circle(to));
                    }
                };
                if is_host {
                    step(row, "−", (circle > 0).then(|| circle - 1));
                }
                row.spawn(theme::text(l10n.n("lobby-circle", circle as i32), &fonts.title, 24.0, if circle > 0 { theme::FRESH_BLOOD } else { theme::BONE }));
                // what the circles do, told on hover of a little "i"
                {
                    let mut help = l10n.t("lobby-circle-help");
                    for k in 1..=10 {
                        help.push_str(&format!("
{k}. {}", l10n.t(&format!("lobby-circle-{k}"))));
                    }
                    row.spawn((
                        Hovered::default(),
                        crate::run_ui::CardHints::above(vec![(l10n.t("lobby-circle-help-title"), help)]),
                        Node { width: px(32), height: px(32), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
                        BackgroundColor(theme::SOOT),
                        BorderColor::all(theme::CANDLE),
                        children![theme::text("i", &fonts.title, 22.0, theme::CANDLE)],
                    ));
                }
                if is_host {
                    step(row, "+", (circle < max).then(|| circle + 1));
                }
                // turn timer: off by default; the host may set 60, 90 or 120 seconds
                if lobby.players.len() > 1 {
                    let timer = lobby.turn_timer;
                    let label = match timer {
                        Some(s) => l10n.n("lobby-timer", s as i32),
                        None => l10n.t("lobby-timer-off"),
                    };
                    if is_host {
                        row.spawn((
                            bar_button(label, &fonts, 400.0, theme::PEAT),
                            observe(move |_: On<Activate>, mut net: ResMut<Net>| {
                                let next = match timer {
                                    None => Some(60),
                                    Some(60) => Some(90),
                                    Some(90) => Some(120),
                                    _ => None,
                                };
                                net.node.set_turn_timer(next);
                            }),
                        ));
                    } else {
                        row.spawn(theme::text(label, &fonts.body, 20.0, theme::DIM));
                    }
                }
            });
        });
        bar.spawn((Node { column_gap: px(14), ..default() }, Pickable::IGNORE)).with_children(|row| {
            if net.lobby.is_some() {
                row.spawn((
                    bar_button(l10n.t("lobby-invite"), &fonts, 260.0, theme::PEAT),
                    observe(|_: On<Activate>, steam: Option<Res<Steam>>, net: Res<Net>| {
                        if let Some(steam) = steam {
                            net::invite(&steam, &net);
                        }
                    }),
                ));
            }
            let ready = mine.is_some_and(|m| m.ready);
            let can_ready = mine.is_some_and(|m| m.hero.is_some());
            row.spawn((
                bar_button(l10n.t(if ready { "battle-unready" } else { "battle-ready" }), &fonts, 240.0, if ready { theme::SWAMP } else if can_ready { theme::PEAT } else { theme::SOOT }),
                observe(move |_: On<Activate>, mut net: ResMut<Net>| net.node.set_ready(!ready)),
            ));
            if is_host {
                let base = if lobby.can_start() { theme::RUST_BLOOD } else { theme::SOOT };
                row.spawn((
                    bar_button(l10n.t("lobby-start"), &fonts, 260.0, base),
                    observe(|_: On<Activate>, mut net: ResMut<Net>, db: Res<Db>, time: Res<Time>, profile: Res<crate::profile::Profile>| {
                        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) ^ time.elapsed().as_nanos() as u64;
                        // the host's village decides which relics, potions and omens can turn up
                        net.node.secrets = profile.secret_relics();
                        net.node.start(&db.0, seed, profile.village());
                    }),
                ));
            }
        });
    });
}

/// How wide the row of heroes' faces is; what does not fit rolls sideways.
const HERO_ROW_W: f32 = 1240.0;

/// The row of faces, the slider under it and its moving piece.
#[derive(Component)]
struct HeroRow;
#[derive(Component)]
struct HeroSlider;
#[derive(Component)]
struct HeroThumb;

/// How far the row is rolled, kept while the lobby is drawn anew.
#[derive(Resource, Default)]
struct HeroRowAt(f32);

/// The wheel rolls the row of heroes sideways, and so does its slider, dragged or pressed anywhere
/// along it; the slider's piece is as long as the share of the row in sight.
#[allow(clippy::too_many_arguments)]
fn roll_heroes(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    pointers: Query<(&bevy::picking::pointer::PointerId, &bevy::picking::pointer::PointerLocation)>,
    mut rows: Query<(&mut ScrollPosition, &ComputedNode, &Children), With<HeroRow>>,
    nodes: Query<&ComputedNode, Without<HeroRow>>,
    sliders: Query<(&ComputedNode, &UiGlobalTransform, &Hovered), With<HeroSlider>>,
    mut thumbs: Query<&mut Node, With<HeroThumb>>,
    mut at_: ResMut<HeroRowAt>,
    mut held: Local<bool>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    let turned: f32 = wheel.read().map(|w| if w.unit == MouseScrollUnit::Line { (w.y + w.x) * 110.0 } else { w.y + w.x }).sum();
    let Ok((mut at, frame, kids)) = rows.single_mut() else { return };
    let Some(inner) = kids.first().and_then(|k| nodes.get(*k).ok()) else { return };
    let seen = frame.size().x * frame.inverse_scale_factor();
    let all = inner.size().x * inner.inverse_scale_factor();
    let most = (all - seen).max(0.0);
    let share = if all > 0.0 { (seen / all).clamp(0.08, 1.0) } else { 1.0 };
    let mut to = at.0.x - turned;
    if let Ok((track, place, over)) = sliders.single() {
        if mouse.just_pressed(MouseButton::Left) && over.get() && most > 1.0 {
            *held = true;
        }
        if !mouse.pressed(MouseButton::Left) {
            *held = false;
        }
        let cursor = pointers.iter().find(|(id, _)| id.is_mouse()).and_then(|(_, l)| l.location.as_ref()).map(|l| l.position * window.scale_factor());
        if let (true, Some(cursor)) = (*held, cursor) {
            let length = track.size().x;
            let left = place.translation.x - length / 2.0;
            let piece = length * share;
            if length > piece {
                to = ((cursor.x - left - piece / 2.0) / (length - piece)).clamp(0.0, 1.0) * most;
            }
        }
    }
    let to = to.clamp(0.0, most);
    if (at.0.x - to).abs() > 0.01 {
        at.0.x = to;
    }
    at_.0 = to;
    for mut thumb in &mut thumbs {
        let (width, left) = (percent(share * 100.0), percent(if most > 0.0 { to / most * (1.0 - share) * 100.0 } else { 0.0 }));
        if thumb.width != width || thumb.left != left {
            thumb.width = width;
            thumb.left = left;
        }
    }
}
