//! Main menu and the "start Steam" wall.

use crate::art::Art;
use crate::battle::Btn;
use crate::l10n::L10n;
use crate::session::Db;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage, STAGE_H, STAGE_W};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Menu), enter_menu)
        .add_systems(OnEnter(AppState::Settings), enter_settings)
        // wherever the settings lie open: on their own screen, or over a run
        .add_systems(Update, drag_sliders.run_if(any_with_component::<SliderStep>))
        .add_systems(Update, toggle_full_screen)
        .add_systems(OnEnter(AppState::Village), enter_village)
        .add_systems(OnEnter(AppState::Chronicle), enter_chronicle)
        .add_systems(Update, crate::profile::note_enemies)
        .add_systems(Update, crate::battle::style_buttons.run_if(not(in_state(AppState::Battle))));
}

pub(crate) fn screen(state: AppState, stage: Entity) -> impl Bundle {
    (
        DespawnOnExit(state),
        Node {
            position_type: PositionType::Absolute,
            width: px(STAGE_W),
            height: px(STAGE_H),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(22),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(stage),
    )
}

/// A painting behind a whole screen, darkened by `shade` (1 = as painted) so text reads over it.
pub fn backdrop(art: &Art, assets: &AssetServer, stem: &str, shade: f32) -> impl Bundle {
    let (mut image, _) = art.node(assets, stem, theme::SOOT);
    image.color = Color::srgb(shade, shade, shade);
    (image, Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: px(STAGE_W), height: px(STAGE_H), ..default() }, Pickable::IGNORE)
}

pub fn menu_button(label: String, fonts: &Fonts) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        Btn(theme::PEAT),
        Node {
            width: px(520),
            padding: UiRect::axes(px(30), px(16)),
            border: UiRect::all(px(3)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(theme::PEAT),
        BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
        children![(theme::text(label, &fonts.title, 32.0, theme::BONE), TextLayout::justify(Justify::Center))],
    )
}

fn enter_menu(
    mut commands: Commands,
    profile: Res<crate::profile::Profile>,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    commands.spawn((
        DespawnOnExit(AppState::Menu),
        Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() },
        art.node(&assets, "bg/menu", theme::SOOT),
        Pickable::IGNORE,
        ChildOf(*stage),
    ));
    commands.spawn((
        DespawnOnExit(AppState::Menu),
        theme::text(
            l10n.tf("menu-profile", &[("oberegi", (profile.oberegi as i32).into()), ("wins", (profile.wins as i32).into()), ("runs", (profile.runs as i32).into()), ("circle", (profile.circle_unlocked as i32).into())]),
            &fonts.body,
            22.0,
            theme::DIM,
        ),
        Node { position_type: PositionType::Absolute, left: px(24), bottom: px(20), ..default() },
        ChildOf(*stage),
    ));
    let column = commands
        .spawn((
        screen(AppState::Menu, *stage),
        children![
            theme::text(l10n.t("game-title"), &fonts.title, 120.0, theme::BONE),
            (theme::text(l10n.t("game-subtitle"), &fonts.body, 30.0, theme::CANDLE), Node { margin: UiRect::bottom(px(40)), ..default() }),
            (
                menu_button(l10n.t("menu-solo"), &fonts),
                observe(|_: On<Activate>, mut commands: Commands, db: Res<Db>, steam: Option<Res<crate::steam::Steam>>, l10n: Res<L10n>, mut next: ResMut<NextState<AppState>>| {
                    // alone is just a party of one: same lobby, same rules, no Steam traffic
                    let name = steam.map(|s| s.client.friends().name()).unwrap_or_else(|| l10n.t("lobby-wanderer"));
                    crate::net::start_offline(&mut commands, &db.0, &name, 0);
                    next.set(AppState::Lobby);
                }),
            ),
            (
                menu_button(l10n.t("menu-coop"), &fonts),
                observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Coop)),
            ),
            (
                menu_button(l10n.t("menu-village"), &fonts),
                observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Village)),
            ),
            (
                menu_button(l10n.t("menu-library"), &fonts),
                observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Library)),
            ),
            (
                menu_button(l10n.t("menu-settings"), &fonts),
                observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Settings)),
            ),
            (
                menu_button(l10n.t("menu-lang"), &fonts),
                observe(|_: On<Activate>, mut l10n: ResMut<L10n>, art: Res<Art>, mut profile: ResMut<crate::profile::Profile>, mut next: ResMut<NextState<AppState>>| {
                    *l10n = L10n::load(&art, l10n.lang.other());
                    // and it stays so the next time the game starts
                    profile.settings.text_lang = l10n.lang.saved();
                    profile.save();
                    // re-enter the menu so every label is rebuilt in the new language
                    next.set(AppState::Menu);
                }),
            ),
            (
                menu_button(l10n.t("menu-quit"), &fonts),
                observe(|_: On<Activate>, mut exit: MessageWriter<AppExit>| {
                    exit.write(AppExit::Success);
                }),
            ),
        ],
    ))
        .id();
    // a solo run left unfinished can be picked up again: first of the buttons, under the title.
    // Wider than the rest, for the hero's name and the act to fit on one line
    if let Some(run) = profile.load_run() {
        let label = l10n.tf("menu-continue", &[("hero", l10n.t(&format!("hero-{}", run.players[0].hero)).into()), ("act", (run.act as i32).into())]);
        let button = commands
            .spawn((
                menu_button(label, &fonts),
                observe(|_: On<Activate>, mut commands: Commands, profile: Res<crate::profile::Profile>, steam: Option<Res<crate::steam::Steam>>, l10n: Res<L10n>| {
                    if let Some(run) = profile.load_run() {
                        let name = steam.map(|s| s.client.friends().name()).unwrap_or_else(|| l10n.t("lobby-wanderer"));
                        // the stage router opens whichever screen the run was left on
                        crate::net::resume_solo(&mut commands, &name, run);
                    }
                }),
            ))
            .insert(Node { width: px(760), padding: UiRect::axes(px(30), px(16)), border: UiRect::all(px(3)), justify_content: JustifyContent::Center, ..default() })
            .id();
        commands.entity(column).insert_children(2, &[button]);
    }
}

/// Pushes the saved settings into the resources the game reads each frame.
pub fn apply_settings(
    profile: Res<crate::profile::Profile>,
    mut speed: ResMut<crate::session::AnimSpeed>,
    mut comfort: ResMut<crate::battle::Comfort>,
    mut mixer: ResMut<crate::audio::Mixer>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    let s = profile.settings;
    // a dev screenshot run keeps its hidden window as it is
    if !(cfg!(feature = "dev") && std::env::var("TROPA_SHOT").is_ok()) {
        for mut window in &mut windows {
            use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode};
            let mode = match s.screen {
                1 => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                2 => WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current),
                _ => WindowMode::Windowed,
            };
            if window.mode != mode {
                window.mode = mode;
            }
            if mode == WindowMode::Windowed {
                let (w, h) = crate::profile::WINDOW_SIZES[(s.window_size as usize).min(crate::profile::WINDOW_SIZES.len() - 1)];
                if (window.resolution.width() - w as f32).abs() > 0.5 || (window.resolution.height() - h as f32).abs() > 0.5 {
                    window.resolution.set(w as f32, h as f32);
                    window.position = bevy::window::WindowPosition::Centered(MonitorSelection::Current);
                }
            }
        }
    }
    speed.0 = s.anim_speed.clamp(1.0, 2.0);
    comfort.less_shake = s.less_shake;
    comfort.ink = s.ink;
    // an unattended dev run keeps its silence
    if !(cfg!(feature = "dev") && std::env::var("TROPA_SHOT").is_ok()) {
        mixer.music = s.music;
        mixer.sfx = s.sfx;
        mixer.narrator = s.narrator;
        mixer.voices = s.voices;
    }
}

/// The settings laid over a run, opened from the pause menu: the same rows as the menu's screen.
#[derive(Component)]
pub struct SettingsOverlay;

fn enter_settings(
    mut commands: Commands,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    profile: Res<crate::profile::Profile>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    commands.spawn((DespawnOnExit(AppState::Settings), backdrop(&art, &assets, "bg/menu", 0.45), ChildOf(*stage)));
    let root = commands.spawn(screen(AppState::Settings, *stage)).id();
    settings_rows(&mut commands, root, false, &fonts, &l10n, &profile);
}

/// Lays the settings over the pause menu (anew, if they lie there already: a row has changed).
pub fn open_settings_overlay(
    mut commands: Commands,
    old: Query<Entity, With<SettingsOverlay>>,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    profile: Res<crate::profile::Profile>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let root = commands
        .spawn((
            SettingsOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: px(STAGE_W),
                height: px(STAGE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(22),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.97)),
            // over the pause menu it was opened from
            GlobalZIndex(1001),
            ChildOf(*stage),
        ))
        .id();
    settings_rows(&mut commands, root, true, &fonts, &l10n, &profile);
}

/// Shows a changed setting where the settings are open: on their own screen, or over a run.
fn redraw_settings(mut commands: Commands, state: Res<State<AppState>>, mut next: ResMut<NextState<AppState>>, overlay: Query<(), With<SettingsOverlay>>) {
    if *state.get() == AppState::Settings {
        next.set(AppState::Settings);
    } else if !overlay.is_empty() {
        commands.run_system_cached(open_settings_overlay);
    }
}

/// The rows of the settings under `root`. `in_run`: laid over a run, where "back" only folds them
/// away and the updater, which leads out to the menu, is left off.
fn settings_rows(commands: &mut Commands, root: Entity, in_run: bool, fonts: &Fonts, l10n: &L10n, profile: &crate::profile::Profile) {
    use crate::profile::{Profile, Settings};
    let s = profile.settings;
    let on_off = |v: bool| l10n.t(if v { "settings-on" } else { "settings-off" });
    let pct = |v: f32| format!("{}%", (v * 100.0).round() as i32);
    // each row: its label with the current value, and how one click changes the settings
    let rows: Vec<(String, fn(&mut Settings))> = vec![
        (format!("{}: {}x", l10n.t("settings-speed"), s.anim_speed), |s| s.anim_speed = if s.anim_speed < 1.25 { 1.5 } else if s.anim_speed < 1.75 { 2.0 } else { 1.0 }),
        (format!("{}: {}", l10n.t("settings-shake"), on_off(s.less_shake)), |s| s.less_shake = !s.less_shake),
        (format!("{}: {}", l10n.t("settings-ink"), on_off(s.ink)), |s| s.ink = !s.ink),
        (format!("{}: {}", l10n.t("settings-hand"), l10n.t(if s.raised_hand { "settings-hand-raised" } else { "settings-hand-default" })), |s| s.raised_hand = !s.raised_hand),
        (format!("{}: {}", l10n.t("settings-voice-lang"), l10n.t(&format!("settings-voice-lang-{}", s.voice_lang.min(3)))), |s| s.voice_lang = (s.voice_lang.min(3) + 1) % 4),
        (format!("{}: {}", l10n.t("settings-screen"), l10n.t(&format!("settings-screen-{}", s.screen.min(2)))), |s| s.screen = (s.screen.min(2) + 1) % 3),
        ({
            let (w, h) = crate::profile::WINDOW_SIZES[(s.window_size as usize).min(crate::profile::WINDOW_SIZES.len() - 1)];
            format!("{}: {w}x{h}", l10n.t("settings-window-size"))
        }, |s| s.window_size = (s.window_size + 1) % crate::profile::WINDOW_SIZES.len() as u8),
    ];
    commands.entity(root).with_children(|p| {
        p.spawn(theme::text(l10n.t("menu-settings"), &fonts.title, 72.0, theme::BONE));
        for (label, change) in rows {
            p.spawn((
                // nine rows and a title have to fit the screen
                Node { margin: UiRect::vertical(px(-9)), ..default() },
                children![(
                menu_button(label, fonts),
                observe(move |_: On<Activate>, mut profile: ResMut<Profile>, mut commands: Commands| {
                    change(&mut profile.settings);
                    profile.save();
                    commands.run_system_cached(apply_settings);
                    // redraw with the new value
                    commands.run_system_cached(redraw_settings);
                }),
                )],
            ));
        }
        // the volumes are sliders: a bar of steps, set by a click or dragged along with the button held
        for (which, key) in [(0u8, "settings-music"), (1, "settings-narrator"), (2, "settings-voices"), (3, "settings-sfx")] {
            let v = volume(&s, which);
            p.spawn((Node { align_items: AlignItems::Center, column_gap: px(18), margin: UiRect::vertical(px(2)), ..default() }, Pickable::IGNORE)).with_children(|row| {
                row.spawn((theme::text(l10n.t(key), &fonts.body, 26.0, theme::BONE), Node { width: px(260), ..default() }));
                row.spawn((Node { column_gap: px(3), ..default() }, Pickable::IGNORE)).with_children(|bar| {
                    for step in 1..=SLIDER_STEPS {
                        bar.spawn((
                            SliderStep { which, step },
                            Hovered::default(),
                            Node { width: px(20), height: px(30), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(3)), ..default() },
                            BackgroundColor(step_color(step, v)),
                            BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
                        ));
                    }
                });
                row.spawn((SliderValue(which), theme::text(pct(v), &fonts.num, 26.0, theme::CANDLE), Node { width: px(80), ..default() }));
            });
        }
        if in_run {
            p.spawn((
                menu_button(l10n.t("coop-back"), fonts),
                observe(|_: On<Activate>, mut commands: Commands, open: Query<Entity, With<SettingsOverlay>>| {
                    for e in &open {
                        commands.entity(e).despawn();
                    }
                }),
            ));
        } else {
            p.spawn((
                menu_button(l10n.t("coop-back"), fonts),
                observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu)),
            ));
            // off to the side: game version and check updates button
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(40),
                    bottom: px(40),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexStart,
                    row_gap: px(8),
                    ..default()
                },
                children![
                    (theme::text(format!("v{}", crate::updater::SemVer::current().to_string()), &fonts.num, 20.0, theme::DIM)),
                    (
                        menu_button(l10n.t("updater-check-btn"), fonts),
                        observe(|_: On<Activate>, mut updater: ResMut<crate::updater::UpdaterState>, mut next: ResMut<NextState<AppState>>| {
                            updater.dismissed = false;
                            crate::updater::trigger_check(&mut updater);
                            next.set(AppState::Menu);
                        }),
                    ),
                ],
            ));
        }

        // off to the side, or the rows would not fit: where to find what to send with a bug report
        p.spawn((
            Node { position_type: PositionType::Absolute, right: px(40), bottom: px(40), ..default() },
            children![(
                menu_button(l10n.t("settings-logs"), fonts),
                observe(|_: On<Activate>| crate::logfile::show_folder()),
            )],
        ));
    });
}

/// F11, or Alt+Enter, takes the game between a window and the whole screen, and keeps the choice.
fn toggle_full_screen(keys: Res<ButtonInput<KeyCode>>, mut profile: ResMut<crate::profile::Profile>, mut commands: Commands) {
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if !(keys.just_pressed(KeyCode::F11) || (alt && keys.just_pressed(KeyCode::Enter))) {
        return;
    }
    let s = &mut profile.settings;
    s.screen = if s.screen == 0 { 1 } else { 0 };
    profile.save();
    commands.run_system_cached(apply_settings);
    // the settings, where they are open, show the new choice
    commands.run_system_cached(redraw_settings);
}

const SLIDER_STEPS: u8 = 20;

/// One step of a volume bar: `which` volume, and its share at `step` of `SLIDER_STEPS`.
#[derive(Component)]
struct SliderStep {
    which: u8,
    step: u8,
}

/// The percent shown beside a volume bar.
#[derive(Component)]
struct SliderValue(u8);

fn volume(s: &crate::profile::Settings, which: u8) -> f32 {
    match which {
        0 => s.music,
        1 => s.narrator,
        2 => s.voices,
        _ => s.sfx,
    }
}

fn step_color(step: u8, v: f32) -> Color {
    if (step as f32) <= (v * SLIDER_STEPS as f32).round() {
        theme::CANDLE
    } else {
        theme::SOOT
    }
}

/// While the button is held over a step of a bar, that volume becomes the step's; the first step
/// pressed twice in a row silences it.
fn drag_sliders(
    mouse: Res<ButtonInput<MouseButton>>,
    mut profile: ResMut<crate::profile::Profile>,
    mut steps: Query<(&SliderStep, &Hovered, &mut BackgroundColor)>,
    mut values: Query<(&SliderValue, &mut Text)>,
    mut commands: Commands,
    mut unsaved: Local<bool>,
) {
    let mut changed = false;
    if mouse.pressed(MouseButton::Left) {
        for (st, hovered, _) in &steps {
            if !hovered.get() {
                continue;
            }
            let now = volume(&profile.settings, st.which);
            let mut want = st.step as f32 / SLIDER_STEPS as f32;
            if st.step == 1 && mouse.just_pressed(MouseButton::Left) && (now - want).abs() < 0.01 {
                want = 0.0;
            }
            if (now - want).abs() > 0.001 {
                let s = &mut profile.settings;
                match st.which {
                    0 => s.music = want,
                    1 => s.narrator = want,
                    2 => s.voices = want,
                    _ => s.sfx = want,
                }
                changed = true;
            }
        }
    }
    if changed {
        *unsaved = true;
        commands.run_system_cached(apply_settings);
        for (st, _, mut bg) in &mut steps {
            bg.0 = step_color(st.step, volume(&profile.settings, st.which));
        }
        for (v, mut text) in &mut values {
            text.0 = format!("{}%", (volume(&profile.settings, v.0) * 100.0).round() as i32);
        }
    }
    // saved once the button is let go, not on every step dragged over
    if mouse.just_released(MouseButton::Left) && *unsaved {
        *unsaved = false;
        profile.save();
    }
}

fn enter_village(mut commands: Commands, stage: Single<Entity, With<Stage>>, fonts: Res<Fonts>, l10n: Res<L10n>, profile: Res<crate::profile::Profile>, art: Res<Art>, assets: Res<AssetServer>) {
    use crate::profile::{Profile, BUILDINGS};
    commands.spawn((
        DespawnOnExit(AppState::Village),
        Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() },
        // the stockade is rebuilt in three stages as the palisade rises: ruin, half raised, whole
        {
            let stage = match profile.building("palisade") {
                0..=1 => 0,
                2..=3 => 1,
                _ => 2,
            };
            let picture = [format!("bg/village_{stage}"), "bg/village_0".to_string(), "bg/menu".to_string()].into_iter().find(|s| art.image(&assets, s).is_some()).unwrap_or_default();
            art.node(&assets, &picture, theme::SOOT)
        },
        Pickable::IGNORE,
        ChildOf(*stage),
    ));
    // the list keeps to the left, so the stockade it rebuilds stays in view
    commands
        .spawn((
            DespawnOnExit(AppState::Village),
            Node {
                position_type: PositionType::Absolute,
                left: px(60),
                width: px(880),
                height: px(STAGE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(22),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(*stage),
        ))
        .with_children(|p| {
        p.spawn(theme::text(l10n.t("village-title"), &fonts.title, 64.0, theme::BONE));
        p.spawn(theme::text(l10n.n("village-oberegi", profile.oberegi as i32), &fonts.num, 34.0, theme::CANDLE));
        for (id, max) in BUILDINGS {
            let level = profile.building(id);
            let price = profile.next_price(id);
            let can = price.is_some_and(|c| profile.oberegi >= c);
            let state = match price {
                Some(c) => l10n.tf("village-row", &[("level", (level as i32).into()), ("max", (*max as i32).into()), ("price", (c as i32).into())]),
                None => l10n.t("village-done"),
            };
            let label = format!("{}  ·  {}\n{}", l10n.t(&format!("building-{id}")), state, l10n.t(&format!("building-{id}-desc")));
            let base = if can { theme::PEAT } else { theme::SOOT };
            let mut row = p.spawn((
                Button,
                Hovered::default(),
                Btn(base),
                Node { width: px(860), padding: UiRect::axes(px(18), px(8)), border: UiRect::all(px(2)), ..default() },
                BackgroundColor(base.with_alpha(0.92)),
                BorderColor::all(if can { theme::CANDLE } else { Color::srgb(0.32, 0.26, 0.20) }),
                children![theme::text(label, &fonts.body, 19.0, if price.is_none() { theme::ROTLIGHT } else if can { theme::BONE } else { theme::DIM })],
            ));
            if can {
                let id = id.to_string();
                row.observe(move |_: On<Activate>, mut profile: ResMut<Profile>, mut next: ResMut<NextState<AppState>>| {
                    if profile.build(&id) {
                        // redraw with the new level and purse
                        next.set(AppState::Village);
                    }
                });
            }
        }
        if profile.building("graveyard") > 0 {
            p.spawn((
                menu_button(l10n.t("chronicle-open"), &fonts),
                observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Chronicle)),
            ));
        }
        p.spawn((
            menu_button(l10n.t("coop-back"), &fonts),
            observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu)),
        ));
    });
}

/// The graveyard's chronicle: every enemy in the game, named once met, and the last runs.
fn enter_chronicle(
    mut commands: Commands,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    profile: Res<crate::profile::Profile>,
    db: Res<Db>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    let met = db.0.enemies.keys().filter(|id| profile.bestiary.contains_key(*id)).count();
    commands.spawn((DespawnOnExit(AppState::Chronicle), backdrop(&art, &assets, "bg/chronicle", 0.55), ChildOf(*stage)));
    commands.spawn(screen(AppState::Chronicle, *stage)).with_children(|p| {
        p.spawn(theme::text(l10n.t("chronicle-title"), &fonts.title, 56.0, theme::BONE));
        p.spawn(theme::text(
            l10n.tf("chronicle-met", &[("n", (met as i32).into()), ("all", (db.0.enemies.len() as i32).into())]),
            &fonts.body,
            22.0,
            theme::CANDLE,
        ));
        // every enemy by its picture: the ones never met only as a black shape
        p.spawn((
            Node { width: px(1780), flex_wrap: FlexWrap::Wrap, column_gap: px(6), row_gap: px(6), justify_content: JustifyContent::Center, ..default() },
            Pickable::IGNORE,
        ))
        .with_children(|grid| {
            for id in db.0.enemies.keys() {
                let seen = profile.bestiary.get(id);
                let (mut face, _) = art.node(&assets, &format!("enemies/{id}"), theme::PEAT);
                face.color = if seen.is_some() { Color::WHITE } else { Color::srgba(0.0, 0.0, 0.0, 0.85) };
                let label = match seen {
                    Some(n) => format!("{}  ×{n}", l10n.t(&format!("enemy-{id}"))),
                    None => "? ? ?".to_string(),
                };
                grid.spawn((
                    theme::panel_edged(
                        Node { width: px(120), height: px(108), border: UiRect::all(px(2)), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, padding: UiRect::all(px(3)), ..default() },
                        if seen.is_some() { Color::srgb(0.42, 0.33, 0.22) } else { Color::srgb(0.2, 0.17, 0.14) },
                    ),
                    Pickable::IGNORE,
                ))
                .with_children(|cell| {
                    cell.spawn((face, Node { width: px(76), height: px(76), ..default() }, Pickable::IGNORE));
                    cell.spawn((theme::text(label, &fonts.body, 13.0, if seen.is_some() { theme::BONE } else { theme::DIM }), TextLayout::justify(Justify::Center)));
                });
            }
        });
        p.spawn(theme::text(
            l10n.tf("chronicle-runs", &[("runs", (profile.runs as i32).into()), ("wins", (profile.wins as i32).into())]),
            &fonts.body,
            22.0,
            theme::CANDLE,
        ));
        p.spawn(Node { width: px(1640), flex_wrap: FlexWrap::Wrap, column_gap: px(20), row_gap: px(4), justify_content: JustifyContent::Center, ..default() }).with_children(|grid| {
            // newest first; six fit on the page under the faces
            for r in profile.history.iter().rev().take(6) {
                let key = if r.won { "chronicle-won" } else { "chronicle-fell" };
                let line = l10n.tf(key, &[("hero", l10n.t(&format!("hero-{}", r.hero)).into()), ("act", (r.act as i32).into()), ("floors", (r.floors as i32).into()), ("circle", (r.circle as i32).into())]);
                grid.spawn((theme::text(line, &fonts.body, 18.0, if r.won { theme::ROTLIGHT } else { theme::BONE }), Node { width: px(530), ..default() }));
            }
        });
        let reached = crate::story::ENDINGS.iter().filter(|id| profile.endings.contains_key(**id)).count();
        p.spawn(theme::text(
            l10n.tf("chronicle-endings", &[("n", (reached as i32).into()), ("all", (crate::story::ENDINGS.len() as i32).into())]),
            &fonts.body,
            22.0,
            theme::CANDLE,
        ));
        // the glued leaves of the tale's book: what of the storyteller has been read
        p.spawn(theme::text(
            l10n.tf("chronicle-secrets", &[("n", (profile.secrets.len() as i32).into()), ("all", (nav_core::SECRET_RELICS.len() as i32).into())]),
            &fonts.body,
            22.0,
            theme::CANDLE,
        ));
        p.spawn(Node { width: px(1640), flex_wrap: FlexWrap::Wrap, column_gap: px(20), row_gap: px(4), justify_content: JustifyContent::Center, ..default() }).with_children(|grid| {
            for id in crate::story::ENDINGS {
                let (label, color) = match profile.endings.get(id) {
                    Some(n) => (format!("{}  ·  ×{n}", l10n.t(&format!("story-name-{id}"))), if id.starts_with("win") { theme::ROTLIGHT } else { theme::BONE }),
                    None => (l10n.t("chronicle-ending-unknown"), theme::DIM),
                };
                grid.spawn((theme::text(label, &fonts.body, 18.0, color), Node { width: px(390), ..default() }));
            }
        });
        p.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(20),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|btns| {
            btns.spawn((
                menu_button(l10n.t("chronicle-analytics"), &fonts),
                observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Analytics)),
            ));
            btns.spawn((
                menu_button(l10n.t("coop-back"), &fonts),
                observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Village)),
            ));
        });
    });
}
