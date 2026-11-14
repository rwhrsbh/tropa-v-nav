//! What stays at hand between fights, as in Slay the Spire: the bar along the top (the hero's
//! health and purse, the potion belt, the relics, the deck to look through) and the pedlar
//! standing in the clearing before the party, who opens his counter when clicked and pays to be
//! left alone when a potion is broken over his head.

use crate::art::Art;
use crate::audio::{Mixer, Sfx};
use crate::battle::Btn;
use crate::l10n::L10n;
use crate::net::Net;
use crate::run_ui::{shown_card, CardHints};
use crate::session::{Db, RunOutbox};
use crate::theme::{self, Fonts};
use crate::{AppState, Stage as Canvas, STAGE_H, STAGE_W};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};
use nav_core::*;

/// The screens between fights, where the bar is shown.
const BETWEEN: [AppState; 5] = [AppState::Map, AppState::Reward, AppState::Rest, AppState::Shop, AppState::Omen];
/// How tall the bar along the top is.
pub const BAR_H: f32 = 64.0;
/// Where the pedlar stands in the clearing, his feet and his size.
pub const PEDLAR_FOOT: Vec2 = Vec2::new(1450.0, 760.0 + CLEARING_DROP);
/// How much lower than in a fight the party stands in the pedlar's clearing.
const CLEARING_DROP: f32 = 135.0;
const PEDLAR_SIZE: Vec2 = Vec2::new(390.0, 520.0);

pub fn plugin(app: &mut App) {
    app.init_resource::<ShopOpen>().init_resource::<PedlarShake>().init_resource::<DeckSort>().add_systems(Update, (resort_deck, light_sort, scroll_deck));
    for state in BETWEEN {
        app.add_systems(OnExit(state), |mut commands: Commands, bars: Query<Entity, Or<(With<TopBar>, With<DeckView>)>>| {
            for e in &bars {
                commands.entity(e).despawn();
            }
        });
    }
    // a player's sheet can be opened in a fight too
    app.add_systems(OnExit(AppState::Battle), |mut commands: Commands, views: Query<Entity, With<DeckView>>| {
        for e in &views {
            commands.entity(e).despawn();
        }
    });
    app.add_systems(OnEnter(AppState::Shop), |mut open: ResMut<ShopOpen>| open.0 = false)
        .add_systems(Update, top_bar.run_if(|state: Res<State<AppState>>| BETWEEN.contains(state.get())))
        .add_systems(Update, (fly_potions, shake_pedlar).run_if(in_state(AppState::Shop)));
}

/// The pedlar's counter is open (else the party stands before him in the clearing).
#[derive(Resource, Default)]
pub struct ShopOpen(pub bool);

/// Seconds left of the pedlar's flinch after a potion broke over him.
#[derive(Resource, Default)]
pub struct PedlarShake(f32);

#[derive(Component)]
struct TopBar;

/// What the bar was last drawn from, to redraw it only when that changes.
#[derive(Component, PartialEq)]
struct BarSig(String);

#[derive(Component)]
struct PotionMenu;

#[derive(Component)]
pub struct DeckView;

/// Pressed, opens the sheet of the player in this seat (see [`open_player`]).
#[derive(Component)]
pub struct PlayerSheet(pub u8);

/// A relic or a flask on the bar, by its picture's stem ("relics/x", "potions/y"): where a
/// newly gained one flies to.
#[derive(Component)]
pub struct BarItem(pub String);

/// The pedlar's figure in the clearing.
#[derive(Component)]
pub struct Pedlar;

/// A flask in flight from the belt to the pedlar's head.
#[derive(Component)]
struct Flight {
    from: Vec2,
    to: Vec2,
    t: f32,
}

/// The strip along the top of the screen, the same in a fight as between fights.
pub(crate) fn bar_plate() -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: px(STAGE_W),
            height: px(BAR_H),
            padding: UiRect::horizontal(px(20)),
            border: UiRect::bottom(px(3)),
            align_items: AlignItems::Center,
            column_gap: px(26),
            ..default()
        },
        BackgroundColor(Color::srgb(0.11, 0.09, 0.08).with_alpha(0.94)),
        BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
    )
}

/// A figure on the bar, its mark beside it.
pub(crate) fn bar_stat() -> impl Bundle {
    (Node { align_items: AlignItems::Center, column_gap: px(8), ..default() }, Pickable::IGNORE)
}

fn stat_mark(mark: Color, rim: Color, round: BorderRadius) -> impl Bundle {
    (Node { width: px(24), height: px(24), border: UiRect::all(px(2)), border_radius: round, ..default() }, BackgroundColor(mark), BorderColor::all(rim), UiTransform { rotation: Rot2::degrees(45.0), ..default() }, Pickable::IGNORE)
}

/// The mark of health: a drop of blood.
pub(crate) fn blood_mark() -> impl Bundle {
    let drop = BorderRadius { top_left: percent(50).into(), top_right: px(2).into(), bottom_right: percent(50).into(), bottom_left: percent(50).into() };
    stat_mark(theme::FRESH_BLOOD, theme::RUST_BLOOD.darker(0.1), drop)
}

/// The mark of the purse: a coin.
pub(crate) fn coin_mark() -> impl Bundle {
    stat_mark(theme::CANDLE, Color::srgb(0.45, 0.30, 0.10), BorderRadius::MAX)
}

/// How many of this player's spins at the hollow have been shown to their end. Until the drums
/// of the latest have stopped, the bar along the top keeps what it shows: a purse must not tell
/// how they will stop before they have.
#[derive(Resource, Default)]
pub struct BarHold(pub u32);

#[allow(clippy::too_many_arguments)]
fn top_bar(
    mut commands: Commands,
    hold: Res<BarHold>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    canvas: Single<Entity, With<Canvas>>,
    bars: Query<(Entity, &BarSig), With<TopBar>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Some(pl) = run.players.get(me as usize) else { return };
    let pelted = matches!(&run.stage, Stage::Shop { stock, .. } if stock.get(me as usize).is_some_and(|s| s.pelted));
    let in_shop = matches!(run.stage, Stage::Shop { .. });
    let sig = BarSig(format!("{}|{}|{}|{:?}|{:?}|{}|{pelted}{in_shop}", pl.hp, pl.max_hp, pl.gold, pl.potions, pl.relics, pl.deck.len()));
    if let Ok((_, old)) = bars.single() {
        let turning = matches!(&run.stage, Stage::Hollow { spins, .. } if spins.get(me as usize) != Some(&hold.0));
        if *old == sig || turning {
            return;
        }
    }
    for (e, _) in &bars {
        commands.entity(e).despawn();
    }
    let bar = commands
        .spawn((
            TopBar,
            sig,
            bar_plate(),
            GlobalZIndex(50),
            Pickable::IGNORE,
            ChildOf(*canvas),
        ))
        .id();
    let hero = l10n.t(&format!("hero-{}", pl.hero));
    commands.spawn((theme::text(hero, &fonts.title, 30.0, theme::hero_color(&pl.hero).lighter(0.1)), ChildOf(bar)));
    // health and purse, each with its mark: a drop of blood, a coin
    let health = commands.spawn((bar_stat(), ChildOf(bar))).id();
    commands.spawn((blood_mark(), ChildOf(health)));
    commands.spawn((theme::text(format!("{}/{}", pl.hp, pl.max_hp), &fonts.num, 28.0, theme::FRESH_BLOOD.lighter(0.2)), TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK }, ChildOf(health)));
    let purse = commands.spawn((bar_stat(), ChildOf(bar))).id();
    commands.spawn((coin_mark(), ChildOf(purse)));
    commands.spawn((theme::text(pl.gold.to_string(), &fonts.num, 28.0, theme::CANDLE), TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK }, ChildOf(purse)));
    // the potion belt: a flask is drunk, thrown or poured out from a little menu under it
    let belt = commands.spawn((Node { column_gap: px(10), margin: UiRect::left(px(20)), ..default() }, Pickable::IGNORE, ChildOf(bar))).id();
    for slot in 0..potion_slots(&pl.relics) {
        let id = pl.potions.get(slot).cloned();
        let mut e = commands.spawn((
            Node { width: px(50), height: px(50), padding: UiRect::all(px(3)), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, ..default() },
            BackgroundColor(theme::SOOT),
            BorderColor::all(if id.is_some() { theme::ROTLIGHT } else { theme::DIM.with_alpha(0.4) }),
            ChildOf(belt),
        ));
        let Some(id) = id else {
            e.insert(Pickable::IGNORE);
            continue;
        };
        let name = l10n.t(&format!("potion-{id}"));
        e.insert((BarItem(format!("potions/{id}")), Button, Hovered::default(), Btn(theme::SOOT), CardHints::new(vec![(name.clone(), l10n.t(&format!("potion-{id}-desc")))])));
        e.with_children(|f| {
            match art.image(&assets, &format!("potions/{id}")) {
                Some(h) => f.spawn((Node { width: percent(100), height: percent(100), ..default() }, ImageNode::new(h), Pickable::IGNORE)),
                None => f.spawn(theme::text(name.chars().take(2).collect::<String>(), &fonts.title, 20.0, theme::ROTLIGHT)),
            };
        });
        let heals = db.0.potions.get(&id).is_some_and(|d| !out_of_fight_heals(&d.effects).is_empty());
        let can_throw = in_shop && !pelted;
        e.observe(move |ev: On<Activate>, mut commands: Commands, menus: Query<Entity, With<PotionMenu>>, fonts: Res<Fonts>, l10n: Res<L10n>| {
            let open = !menus.is_empty();
            for m in &menus {
                commands.entity(m).despawn();
            }
            if open {
                return;
            }
            let flask = ev.event_target();
            let menu = commands
                .spawn((
                    PotionMenu,
                    theme::panel(Node {
                        position_type: PositionType::Absolute,
                        left: px(-10),
                        top: px(58),
                        padding: UiRect::all(px(8)),
                        border: UiRect::all(px(2)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(6),
                        ..default()
                    }),
                    GlobalZIndex(300),
                    ChildOf(flask),
                ))
                .id();
            let choice = |commands: &mut Commands, label: String, color: Color, action: RunAction, thrown: bool| {
                commands.spawn((
                    Button,
                    Hovered::default(),
                    Btn(theme::PEAT),
                    Node { width: px(250), padding: UiRect::axes(px(14), px(8)), border: UiRect::all(px(2)), ..default() },
                    BackgroundColor(theme::PEAT),
                    BorderColor::all(color),
                    children![theme::text(label, &fonts.title, 22.0, color)],
                    observe(move |_: On<Activate>, mut commands: Commands, mut out: ResMut<RunOutbox>, menus: Query<Entity, With<PotionMenu>>, flasks: Query<&UiGlobalTransform>, canvas: Single<Entity, With<Canvas>>, scale: Res<UiScale>, potions: Query<&Children>, images: Query<&ImageNode>| {
                        out.0.push(action.clone());
                        for m in &menus {
                            commands.entity(m).despawn();
                        }
                        if thrown {
                            // the flask leaves the belt for the pedlar's head
                            let from = flasks.get(flask).map(|t| t.translation / scale.0).unwrap_or(Vec2::new(600.0, 32.0));
                            let picture = potions.get(flask).ok().and_then(|c| c.iter().find_map(|k| images.get(k).ok())).map(|i| i.image.clone());
                            let mut f = commands.spawn((
                                Flight { from, to: PEDLAR_FOOT - Vec2::new(0.0, PEDLAR_SIZE.y * 0.78), t: 0.0 },
                                Node { position_type: PositionType::Absolute, left: px(from.x - 30.0), top: px(from.y - 30.0), width: px(60), height: px(60), ..default() },
                                UiTransform::IDENTITY,
                                GlobalZIndex(120),
                                Pickable::IGNORE,
                                DespawnOnExit(AppState::Shop),
                                ChildOf(*canvas),
                            ));
                            match picture {
                                Some(h) => f.insert(ImageNode::new(h)),
                                None => f.insert(BackgroundColor(theme::ROTLIGHT)),
                            };
                        }
                    }),
                    ChildOf(menu),
                ));
            };
            if heals {
                choice(&mut commands, l10n.t("potion-drink"), theme::ROTLIGHT, RunAction::DrinkPotion(slot as u8), false);
            }
            if can_throw {
                choice(&mut commands, l10n.tf("potion-pelt", &[("gold", PELT_GOLD.into())]), theme::CANDLE, RunAction::PeltPedlar(slot as u8), true);
            }
            choice(&mut commands, l10n.t("potion-drop"), theme::FRESH_BLOOD.lighter(0.1), RunAction::DropPotion(slot as u8), false);
        });
    }
    // relics, each telling what it does on hover
    // they wrap to a second row and grow smaller rather than run under the deck
    let side = match pl.relics.len() {
        0..=16 => 48.0,
        17..=44 => 28.0,
        _ => 20.0,
    };
    let relics = commands.spawn((Node { column_gap: px(4), row_gap: px(2), flex_wrap: FlexWrap::Wrap, align_content: AlignContent::Center, margin: UiRect::left(px(20)), flex_grow: 1.0, flex_shrink: 1.0, flex_basis: px(0), ..default() }, Pickable::IGNORE, ChildOf(bar))).id();
    for id in &pl.relics {
        commands.spawn((
            Button,
            Hovered::default(),
            CardHints::new(vec![(l10n.t(&format!("relic-{id}")), l10n.t(&format!("relic-{id}-desc")))]),
            BarItem(format!("relics/{id}")),
            Node { width: px(side), height: px(side), ..default() },
            art.node(&assets, &format!("relics/{id}"), theme::CANDLE),
            ChildOf(relics),
        ));
    }
    // the deck, to be looked through
    let deck = pl.deck.len();
    commands.spawn((
        Button,
        PlayerSheet(me),
        Hovered::default(),
        Btn(theme::PEAT),
        Node { padding: UiRect::axes(px(18), px(8)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), align_items: AlignItems::Center, column_gap: px(10), ..default() },
        BackgroundColor(theme::PEAT),
        BorderColor::all(theme::BONE.with_alpha(0.6)),
        children![
            (Node { width: px(26), height: px(36), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(4)), ..default() }, BackgroundColor(theme::RUST_BLOOD), BorderColor::all(theme::CANDLE), Pickable::IGNORE),
            theme::text(l10n.tf("bar-deck", &[("n", (deck as i32).into())]), &fonts.title, 24.0, theme::BONE),
        ],
        observe(open_player),
        ChildOf(bar),
    ));
}

/// A hero's standing figure with its head left off: its cut-out parts, stacked. Falls back on
/// nothing if they were never cut.
#[allow(clippy::too_many_arguments)]
pub fn figure_with_no_head(commands: &mut Commands, root: Entity, stem: &str, lost_arms: u8, place: Node, color: Color, flip: bool, art: &Art, assets: &AssetServer) {
    // painted as it is, when it was
    let mark = crate::battle::short_of(true, lost_arms).unwrap_or("_headless");
    if let Some(h) = art.image(assets, &format!("{stem}{mark}")) {
        commands.spawn((place, ImageNode { color, flip_x: flip, ..ImageNode::new(h) }, Pickable::IGNORE, ChildOf(root)));
        return;
    }
    if lost_arms != 0 {
        // the arms are off too: the parts below leave them out, not the whole headless picture
        for (part, arm) in [("_body", 0), ("_shield", nav_core::combat::SHIELD_ARM), ("_arm", nav_core::combat::WEAPON_ARM)] {
            if lost_arms & arm == 0 {
                if let Some(h) = art.image(assets, &format!("{stem}{part}")) {
                    commands.spawn((place.clone(), ImageNode { color, flip_x: flip, ..ImageNode::new(h) }, Pickable::IGNORE, ChildOf(root)));
                }
            }
        }
        return;
    }
    if let Some(h) = art.image(assets, &format!("{stem}_headless")) {
        commands.spawn((place, ImageNode { color, flip_x: flip, ..ImageNode::new(h) }, Pickable::IGNORE, ChildOf(root)));
        return;
    }
    for part in ["_body", "_shield", "_arm"] {
        if let Some(h) = art.image(assets, &format!("{stem}{part}")) {
            commands.spawn((place.clone(), ImageNode { color, flip_x: flip, ..ImageNode::new(h) }, Pickable::IGNORE, ChildOf(root)));
        }
    }
}

/// A player's sheet laid out over the screen until it is closed: what they carry and their deck,
/// whose cards turn over under the right button as anywhere.
#[allow(clippy::too_many_arguments)]
pub fn open_player(
    ev: On<Activate>,
    rows: Query<&PlayerSheet>,
    mut commands: Commands,
    open: Query<Entity, With<DeckView>>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    sort: Res<DeckSort>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    if !open.is_empty() {
        for e in &open {
            commands.entity(e).despawn();
        }
        return;
    }
    let Some(net) = net else { return };
    let Some(run) = net.node.state.as_ref() else { return };
    let Ok(seat) = rows.get(ev.event_target()) else { return };
    let Some(pl) = run.players.get(seat.0 as usize) else { return };
    let hue = theme::hero_color(&pl.hero);
    let view = commands
        .spawn((
            DeckView,
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(BAR_H),
                width: px(STAGE_W),
                height: px(STAGE_H - BAR_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::vertical(px(DECK_EDGE)),
                ..default()
            },
            // what was on the screen stays in sight, dimmed: the sheet is a window laid over it
            BackgroundColor(Color::srgb(0.02, 0.02, 0.025).with_alpha(0.86)),
            // over the hand and whatever flies in a fight, under the card hints
            GlobalZIndex(65),
            ChildOf(*canvas),
        ))
        .id();
    commands.entity(view).observe(|ev: On<Activate>, mut commands: Commands| commands.entity(ev.event_target()).despawn());
    // the window itself, under everything on the sheet: a framed board, its head strip darker
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px((STAGE_W - DECK_WINDOW_W) / 2.0),
            top: px(DECK_EDGE),
            width: px(DECK_WINDOW_W),
            height: px(STAGE_H - BAR_H - 2.0 * DECK_EDGE),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(12)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::srgb(0.115, 0.098, 0.088)),
        BorderColor::all(SORT_EDGE),
        BoxShadow::new(Color::BLACK.with_alpha(0.8), px(0), px(8), px(0), px(30)),
        Pickable::IGNORE,
        ChildOf(view),
        children![(
            Node { width: percent(100), height: px(DECK_HEAD_H), border: UiRect::bottom(px(2)), ..default() },
            BackgroundColor(theme::SOOT),
            BorderColor::all(SORT_EDGE),
            Pickable::IGNORE,
        )],
    ));
    // another player's sheet has a head: whose it is and how they are, and what they carry. One's
    // own has none: all of that is on the bar along the top already
    if net.node.my_slot() != Some(seat.0) {
    let head = commands
        .spawn((
            theme::panel_edged(
                Node { width: px(DECK_W), padding: UiRect::axes(px(24), px(14)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), align_items: AlignItems::Center, column_gap: px(28), margin: UiRect::top(px(14)), flex_shrink: 0.0, ..default() },
                hue.with_alpha(0.7),
            ),
            Pickable::IGNORE,
            ChildOf(view),
        ))
        .id();
    commands.spawn((
        Node { flex_direction: FlexDirection::Column, row_gap: px(2), flex_shrink: 0.0, ..default() },
        Pickable::IGNORE,
        ChildOf(head),
        children![
            theme::text(l10n.t(&format!("hero-{}", pl.hero)), &fonts.title, 38.0, hue.lighter(0.1)),
            theme::text(crate::run_ui::party_line(pl, &l10n), &fonts.body, 20.0, theme::BONE),
        ],
    ));
    // what they carry, each telling what it does on hover
    let carried = commands.spawn((Node { flex_grow: 1.0, flex_basis: px(0), flex_wrap: FlexWrap::Wrap, align_content: AlignContent::Center, column_gap: px(6), row_gap: px(6), ..default() }, Pickable::IGNORE, ChildOf(head))).id();
    for (kind, id) in pl.relics.iter().map(|id| ("relic", id)).chain(pl.potions.iter().map(|id| ("potion", id))) {
        commands.spawn((
            Button,
            Hovered::default(),
            CardHints::below(vec![(l10n.t(&format!("{kind}-{id}")), l10n.t(&format!("{kind}-{id}-desc")))]),
            Node { width: px(48), height: px(48), ..default() },
            art.node(&assets, &format!("{kind}s/{id}"), theme::CANDLE),
            ChildOf(carried),
        ));
    }
    }
    // how the cards are laid out: one way at a time, and the same way pressed again turns it round
    let bar = commands
        .spawn((Node { width: px(DECK_W), height: px(DECK_HEAD_H), flex_shrink: 0.0, align_items: AlignItems::Center, column_gap: px(12), ..default() }, Pickable::IGNORE, ChildOf(view)))
        .id();
    commands.spawn((theme::text(l10n.tf("bar-deck", &[("n", (pl.deck.len() as i32).into())]), &fonts.title, 26.0, theme::BONE), Node { margin: UiRect::right(px(24)), flex_shrink: 0.0, ..default() }, ChildOf(bar)));
    commands.spawn((theme::text(l10n.t("deck-sort"), &fonts.body, 20.0, theme::DIM), Node { flex_shrink: 0.0, ..default() }, ChildOf(bar)));
    for by in [DeckOrder::Name, DeckOrder::Rarity, DeckOrder::Cost, DeckOrder::Upgraded] {
        commands.spawn((
            SortButton(by),
            Button,
            Hovered::default(),
            Btn(theme::PEAT),
            Node { padding: UiRect::axes(px(18), px(7)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(18)), column_gap: px(8), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() },
            BackgroundColor(theme::PEAT),
            BorderColor::all(SORT_EDGE),
            children![
                theme::text(l10n.t(by.key()), &fonts.title, 20.0, theme::BONE),
                // a little chevron, drawn: the fonts have no arrows
                (
                    SortArrow(by),
                    Node { width: px(10), height: px(10), border: UiRect { right: px(3), bottom: px(3), ..default() }, margin: UiRect::horizontal(px(4)), ..default() },
                    BorderColor::all(theme::CANDLE),
                    UiTransform::IDENTITY,
                    Visibility::Hidden,
                    Pickable::IGNORE,
                ),
            ],
            observe(move |_: On<Activate>, mut sort: ResMut<DeckSort>| {
                if sort.by == by {
                    sort.flipped = !sort.flipped;
                } else {
                    *sort = DeckSort { by, flipped: false };
                }
            }),
            ChildOf(bar),
        ));
    }
    commands.spawn((theme::text(l10n.t("deck-hint"), &fonts.body, 18.0, theme::DIM), Node { margin: UiRect::left(auto()), flex_shrink: 0.0, ..default() }, ChildOf(bar)));
    // the way out, at the end of the same line
    commands.spawn((
        Button,
        Hovered::default(),
        Btn(theme::PEAT),
        Node { padding: UiRect::axes(px(20), px(7)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), margin: UiRect::left(px(8)), flex_shrink: 0.0, ..default() },
        BackgroundColor(theme::PEAT),
        BorderColor::all(theme::BONE.with_alpha(0.6)),
        children![theme::text(l10n.t("deck-close"), &fonts.title, 20.0, theme::BONE)],
        observe(|_: On<Activate>, mut commands: Commands, open: Query<Entity, With<DeckView>>| {
            for e in &open {
                commands.entity(e).despawn();
            }
        }),
        ChildOf(bar),
    ));
    // the cards: an even grid, rolled by the wheel when it is taller than the screen. As wide as
    // the screen, so what a card tells beside itself is not cut off at the grid's edge
    let scroller = commands
        .spawn((
            DeckScroll,
            Node { width: px(STAGE_W), flex_grow: 1.0, flex_basis: px(0), min_height: px(0), justify_content: JustifyContent::Center, align_items: AlignItems::FlexStart, overflow: Overflow::scroll_y(), ..default() },
            ScrollPosition::default(),
            Pickable::IGNORE,
            ChildOf(view),
        ))
        .id();
    let grid = commands
        .spawn((
            DeckGrid(seat.0),
            Node { width: px(DECK_W), flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::FlexStart, column_gap: px(DECK_GAP), row_gap: px(DECK_GAP), padding: UiRect::new(px(0), px(0), px(DECK_GRID_TOP), px(40)), ..default() },
            Pickable::IGNORE,
            ChildOf(scroller),
        ))
        .id();
    lay_deck(&mut commands, grid, &pl.deck, &sort, &db.0, &l10n, &fonts, &art, &assets);
    // down the window's right edge, a slider for a deck of more rows than fit: it shows how far
    // down the cards are rolled, and is dragged to roll them
    commands.spawn((
        DeckSlider,
        Button,
        Hovered::default(),
        Node {
            position_type: PositionType::Absolute,
            left: px((STAGE_W + DECK_WINDOW_W) / 2.0 - 30.0),
            top: px(DECK_EDGE + DECK_HEAD_H + 16.0),
            width: px(14),
            height: px(STAGE_H - BAR_H - 2.0 * DECK_EDGE - DECK_HEAD_H - 32.0),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(7)),
            ..default()
        },
        BackgroundColor(theme::SOOT),
        BorderColor::all(SORT_EDGE),
        Visibility::Hidden,
        ChildOf(view),
        children![(
            DeckThumb,
            Node { position_type: PositionType::Absolute, left: px(1), width: px(10), top: percent(0), height: percent(100), border_radius: BorderRadius::all(px(5)), ..default() },
            BackgroundColor(theme::CANDLE.with_alpha(0.85)),
            Pickable::IGNORE,
        )],
    ));
}

/// How wide the sheet's head and its grid of cards are: `DECK_COLS` cards and the gaps between.
const DECK_COLS: f32 = 7.0;
const DECK_GAP: f32 = 22.0;
/// How large a card is shown on the sheet.
const DECK_K: f32 = 0.82;
const DECK_W: f32 = DECK_COLS * crate::battle::CARD_W * DECK_K + (DECK_COLS - 1.0) * DECK_GAP;
const SORT_EDGE: Color = Color::srgb(0.42, 0.33, 0.22);
/// The sheet's window: how far it stands from the bar above and the screen's foot, how wide it
/// is (the cards, and room either side for the slider), and how tall its head strip is.
const DECK_EDGE: f32 = 14.0;
const DECK_WINDOW_W: f32 = DECK_W + 112.0;
const DECK_HEAD_H: f32 = 76.0;
/// Room above the first row of cards: a card under the cursor grows, and its top is not to be cut.
const DECK_GRID_TOP: f32 = 30.0;

/// What the cards of a sheet are laid out by.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum DeckOrder {
    #[default]
    Name,
    Rarity,
    Cost,
    Upgraded,
}

impl DeckOrder {
    fn key(self) -> &'static str {
        match self {
            DeckOrder::Name => "deck-sort-name",
            DeckOrder::Rarity => "deck-sort-rarity",
            DeckOrder::Cost => "deck-sort-cost",
            DeckOrder::Upgraded => "deck-sort-upgraded",
        }
    }
}

/// The order chosen on a player's sheet; it is kept from one look to the next.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct DeckSort {
    pub by: DeckOrder,
    /// Turned round: the rarest last, the dearest first, the upgraded last.
    pub flipped: bool,
}

/// The grid of a sheet's cards, and whose (the seat).
#[derive(Component)]
struct DeckGrid(u8);
/// What the wheel rolls on a sheet.
#[derive(Component)]
struct DeckScroll;
/// The slider down the side of a sheet whose cards do not all fit, and the piece of it that moves.
#[derive(Component)]
struct DeckSlider;
#[derive(Component)]
struct DeckThumb;
#[derive(Component)]
struct SortButton(DeckOrder);
/// The little arrow on a sort button, showing which way the order runs.
#[derive(Component)]
struct SortArrow(DeckOrder);

/// A deck in the order chosen: each card with its place in the deck. Cards that tie stand by
/// name, the plain one before the upgraded.
pub fn sorted_deck<'a>(deck: &'a [(Id, bool)], sort: DeckSort, db: &Content, name: &dyn Fn(&str) -> String) -> Vec<(usize, &'a (Id, bool))> {
    let rank = |(id, up): &(Id, bool)| -> i32 {
        let def = db.cards.get(id);
        match sort.by {
            DeckOrder::Name => 0,
            // the rarest first; what is no reward at all (curses, tokens) last
            DeckOrder::Rarity => match def.map(|d| d.rarity) {
                Some(Rarity::Rare) => 0,
                Some(Rarity::Uncommon) => 1,
                Some(Rarity::Common) => 2,
                Some(Rarity::Starter) => 3,
                _ => 4,
            },
            // the cheapest first, as it plays now; a card that costs X after every number
            DeckOrder::Cost => def.map(|d| d.view(*up, Form::Human)).map_or(i32::MAX, |v| if v.x_cost { i32::MAX - 1 } else { v.cost }),
            DeckOrder::Upgraded => i32::from(!*up),
        }
    };
    let mut cards: Vec<(i32, String, usize, &(Id, bool))> = deck.iter().enumerate().map(|(i, c)| (rank(c), name(&c.0), i, c)).collect();
    cards.sort_by(|a, b| {
        let first = if sort.flipped { b.0.cmp(&a.0) } else { a.0.cmp(&b.0) };
        first.then_with(|| a.1.cmp(&b.1)).then_with(|| a.3 .1.cmp(&b.3 .1)).then_with(|| a.2.cmp(&b.2))
    });
    cards.into_iter().map(|(_, _, i, c)| (i, c)).collect()
}

#[allow(clippy::too_many_arguments)]
fn lay_deck(commands: &mut Commands, grid: Entity, deck: &[(Id, bool)], sort: &DeckSort, db: &Content, l10n: &L10n, fonts: &Fonts, art: &Art, assets: &AssetServer) {
    use crate::battle::{CARD_H, CARD_W};
    for (i, (id, up)) in sorted_deck(deck, *sort, db, &|id| l10n.t(&format!("card-{id}"))) {
        let slot = commands.spawn((Node { width: px(CARD_W * DECK_K), height: px(CARD_H * DECK_K), flex_shrink: 0.0, ..default() }, Pickable::IGNORE, ChildOf(grid))).id();
        shown_card(commands, slot, i, id, *up, DECK_K, db, l10n, fonts, art, assets);
    }
}

/// A new order chosen: the cards of the open sheet are laid out again, from the top.
#[allow(clippy::too_many_arguments)]
fn resort_deck(
    mut commands: Commands,
    sort: Res<DeckSort>,
    grids: Query<(Entity, &DeckGrid)>,
    mut scrolls: Query<&mut ScrollPosition, With<DeckScroll>>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    if !sort.is_changed() || sort.is_added() {
        return;
    }
    let Some(run) = net.as_ref().and_then(|n| n.node.state.as_ref()) else { return };
    for (grid, seat) in &grids {
        let Some(pl) = run.players.get(seat.0 as usize) else { continue };
        commands.entity(grid).despawn_children();
        lay_deck(&mut commands, grid, &pl.deck, &sort, &db.0, &l10n, &fonts, &art, &assets);
    }
    for mut at in &mut scrolls {
        at.0 = Vec2::ZERO;
    }
}

/// The order in use shows lit on its button, with an arrow for the way it runs.
fn light_sort(sort: Res<DeckSort>, mut buttons: Query<(&SortButton, &mut BorderColor)>, mut arrows: Query<(&SortArrow, &mut Visibility, &mut UiTransform)>) {
    for (b, mut edge) in &mut buttons {
        let want = BorderColor::all(if b.0 == sort.by { theme::CANDLE } else { SORT_EDGE });
        if *edge != want {
            *edge = want;
        }
    }
    for (a, mut seen, mut tf) in &mut arrows {
        seen.set_if_neq(if a.0 == sort.by { Visibility::Inherited } else { Visibility::Hidden });
        // the corner of a square, turned to point down, or up when the order is turned round
        let (turn, shift) = if sort.flipped { (-135.0, 3.0) } else { (45.0, -3.0) };
        let want = UiTransform { rotation: Rot2::degrees(turn), translation: Val2::px(0.0, shift), ..UiTransform::IDENTITY };
        if *tf != want {
            *tf = want;
        }
    }
}

/// The wheel rolls the cards of an open sheet, and so does its slider, dragged or pressed
/// anywhere along its length. The slider is there only while the cards do not all fit, and its
/// moving piece is as long as the share of them in sight.
#[allow(clippy::too_many_arguments)]
fn scroll_deck(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    pointers: Query<(&bevy::picking::pointer::PointerId, &bevy::picking::pointer::PointerLocation)>,
    mut scrolls: Query<(&mut ScrollPosition, &ComputedNode), With<DeckScroll>>,
    grids: Query<&ComputedNode, With<DeckGrid>>,
    mut sliders: Query<(&ComputedNode, &UiGlobalTransform, &Hovered, &mut Visibility), With<DeckSlider>>,
    mut thumbs: Query<&mut Node, With<DeckThumb>>,
    mut held: Local<bool>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    let turned: f32 = wheel.read().map(|w| if w.unit == MouseScrollUnit::Line { w.y * 110.0 } else { w.y }).sum();
    let (Ok((mut at, frame)), Ok(grid)) = (scrolls.single_mut(), grids.single()) else {
        *held = false;
        return;
    };
    // in the stage's own pixels, as the scroll position is
    let seen = frame.size().y * frame.inverse_scale_factor();
    let all = grid.size().y * grid.inverse_scale_factor();
    let most = (all - seen).max(0.0);
    let share = if all > 0.0 { (seen / all).clamp(0.08, 1.0) } else { 1.0 };
    let mut to = at.0.y - turned;
    if let Ok((track, place, over, mut shown)) = sliders.single_mut() {
        shown.set_if_neq(if most > 1.0 { Visibility::Inherited } else { Visibility::Hidden });
        if mouse.just_pressed(MouseButton::Left) && over.get() && most > 1.0 {
            *held = true;
        }
        if !mouse.pressed(MouseButton::Left) {
            *held = false;
        }
        // held, the moving piece follows the cursor: its middle goes where the cursor is
        // (the picking pointer, as hovering uses, in the window's physical pixels like the node's place)
        let cursor = pointers.iter().find(|(id, _)| id.is_mouse()).and_then(|(_, l)| l.location.as_ref()).map(|l| l.position * window.scale_factor());
        if let (true, Some(cursor)) = (*held, cursor) {
            let length = track.size().y;
            let top = place.translation.y - length / 2.0;
            let piece = length * share;
            if length > piece {
                to = ((cursor.y - top - piece / 2.0) / (length - piece)).clamp(0.0, 1.0) * most;
            }
        }
    }
    let to = to.clamp(0.0, most);
    if (at.0.y - to).abs() > 0.01 {
        at.0.y = to;
    }
    for mut thumb in &mut thumbs {
        let (height, top) = (percent(share * 100.0), percent(if most > 0.0 { to / most * (1.0 - share) * 100.0 } else { 0.0 }));
        if thumb.height != height || thumb.top != top {
            thumb.height = height;
            thumb.top = top;
        }
    }
}

fn fly_potions(
    mut commands: Commands,
    time: Res<Time>,
    mut flights: Query<(Entity, &mut Flight, &mut Node, &mut UiTransform)>,
    mut shake: ResMut<PedlarShake>,
    sfx: Res<Sfx>,
    mixer: Res<Mixer>,
) {
    for (e, mut f, mut node, mut tf) in &mut flights {
        f.t += time.delta_secs() / 0.55;
        let p = f.t.min(1.0);
        // a high toss: along the line, lifted in the middle, spinning
        let at = f.from.lerp(f.to, p) - Vec2::new(0.0, 260.0 * (p * std::f32::consts::PI).sin());
        node.left = px(at.x - 30.0);
        node.top = px(at.y - 30.0);
        tf.rotation = Rot2::radians(p * 9.0);
        if f.t >= 1.0 {
            commands.entity(e).despawn();
            shake.0 = 0.6;
            sfx.play(&mut commands, "hit", &mixer, 1.0);
            sfx.play(&mut commands, "potion", &mixer, 0.8);
            sfx.play(&mut commands, "gold", &mixer, 1.0);
        }
    }
}

/// The pedlar flinches when a flask breaks over him, and brightens under the hand.
fn shake_pedlar(time: Res<Time>, mut shake: ResMut<PedlarShake>, mut pedlar: Query<(&mut UiTransform, &mut ImageNode, &Hovered), With<Pedlar>>) {
    shake.0 = (shake.0 - time.delta_secs()).max(0.0);
    let s = shake.0;
    let k = s / 0.6;
    for (mut tf, mut image, hovered) in &mut pedlar {
        tf.translation = Val2::px((s * 60.0).sin() * 14.0 * k, 0.0);
        tf.rotation = Rot2::radians((s * 40.0).sin() * 0.05 * k);
        tf.scale = Vec2::splat(if hovered.get() { 1.03 } else { 1.0 });
        let lit = if hovered.get() { Color::srgb(1.0, 0.95, 0.8) } else { Color::WHITE };
        image.color = lit.mix(&Color::srgb(1.0, 0.55, 0.45), k);
    }
}

/// The clearing where the party meets the pedlar: the act's forest, the heroes on the left as
/// in a fight (but with nothing to fight), and the pedlar on the right, who opens his counter
/// when clicked.
#[allow(clippy::too_many_arguments)]
pub fn pedlar_clearing(commands: &mut Commands, root: Entity, run: &Run, db: &Content, fonts: &Fonts, l10n: &L10n, art: &Art, assets: &AssetServer) {
    let act = run.act.clamp(1, 3);
    let backdrop = [format!("bg/shop_act{act}"), format!("bg/act{act}_battle"), "bg/act1_battle".to_string()].into_iter().find(|s| art.image(assets, s).is_some()).unwrap_or_default();
    let lift = crate::battle::ground_lift(&backdrop);
    // with no hand of cards below, everyone stands CLEARING_DROP lower than in a fight, and the
    // picture is raised only as much as still puts its ground under their feet, then grown
    // evenly so it reaches the bottom of the screen
    let ground = (crate::battle::hero_foot(0).y + lift) / STAGE_H;
    let raise = ((lift - CLEARING_DROP) / (1.0 - ground)).max(0.0);
    let grow = (STAGE_H + raise) / STAGE_H;
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(-STAGE_W * (grow - 1.0) / 2.0),
            top: px(-raise),
            width: px(STAGE_W * grow),
            height: px(STAGE_H + raise),
            ..default()
        },
        art.node(assets, &backdrop, theme::SOOT),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    let size = crate::battle::HERO_SIZE;
    // the back row first, so the front stands before it
    let mut order: Vec<usize> = (0..run.players.len()).collect();
    let n = run.players.len();
    order.sort_by(|a, b| crate::battle::hero_foot(*a as u8).y.total_cmp(&crate::battle::hero_foot(*b as u8).y));
    for i in order {
        let pl = &run.players[i];
        let foot = crate::battle::hero_rank(i as u8, n) + Vec2::new(0.0, CLEARING_DROP);
        // who has lost their head is carried along and laid down here too
        if pl.beheaded {
            if let Some(h) = art.image(assets, &format!("heroes/{}_dead_headless", pl.hero)) {
                commands.spawn((
                    Node { position_type: PositionType::Absolute, left: px(foot.x - size.y * 0.75), top: px(foot.y - size.y), width: px(size.y * 1.5), height: px(size.y), ..default() },
                    ImageNode::new(h),
                    Pickable::IGNORE,
                    ChildOf(root),
                ));
            }
            continue;
        }
        // the stove is broad: it is drawn as large as in a fight, not squeezed into a man's place
        let size = crate::battle::hero_draw(&format!("heroes/{}", pl.hero), size) * crate::battle::hp_stature(db, &pl.hero, pl.max_hp);
        if pl.walks_headless {
            let place = Node { position_type: PositionType::Absolute, left: px(foot.x - size.x / 2.0), top: px(foot.y - size.y), width: px(size.x), height: px(size.y), ..default() };
            figure_with_no_head(commands, root, &format!("heroes/{}", pl.hero), pl.lost_arms, place, Color::WHITE, false, art, assets);
            continue;
        }
        let Some(h) = art.image(assets, &format!("heroes/{}", pl.hero)) else { continue };
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: px(foot.x - size.x / 2.0), top: px(foot.y - size.y), width: px(size.x), height: px(size.y), ..default() },
            ImageNode { color: if pl.hp > 0 { Color::WHITE } else { Color::srgb(0.4, 0.4, 0.45) }, ..ImageNode::new(h) },
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    let mut pedlar = commands.spawn((
        Pedlar,
        Button,
        Hovered::default(),
        Node {
            position_type: PositionType::Absolute,
            left: px(PEDLAR_FOOT.x - PEDLAR_SIZE.x / 2.0),
            top: px(PEDLAR_FOOT.y - PEDLAR_SIZE.y),
            width: px(PEDLAR_SIZE.x),
            height: px(PEDLAR_SIZE.y),
            ..default()
        },
        UiTransform::IDENTITY,
        observe(|_: On<Activate>, mut open: ResMut<ShopOpen>, mut status: ResMut<crate::net::NetStatus>| {
            open.0 = true;
            status.run_dirty = true;
        }),
        ChildOf(root),
    ));
    match art.image(assets, "npc/pedlar") {
        Some(h) => pedlar.insert(ImageNode::new(h)),
        None => pedlar.insert((ImageNode::solid_color(theme::PEAT), BorderColor::all(theme::CANDLE))),
    };
    commands.spawn((
        theme::panel(Node {
            position_type: PositionType::Absolute,
            left: px(PEDLAR_FOOT.x - 200.0),
            // over his head: below his feet the Leave button stands
            top: px(PEDLAR_FOOT.y - PEDLAR_SIZE.y - 70.0),
            width: px(400),
            padding: UiRect::all(px(10)),
            border: UiRect::all(px(2)),
            justify_content: JustifyContent::Center,
            ..default()
        }),
        Pickable::IGNORE,
        ChildOf(root),
        children![(theme::text(l10n.t("shop-pedlar-hint"), &fonts.title, 24.0, theme::CANDLE), TextLayout::justify(Justify::Center))],
    ));
}

/// The act's forest behind a screen between fights, darkened (and warmed, at the fire) by `tint`
/// so what is laid over it stays readable.
pub fn act_backdrop(commands: &mut Commands, root: Entity, run: &Run, art: &Art, assets: &AssetServer, tint: Color) {
    let act = run.act.clamp(1, 3);
    // every fight is met in its foe's own haunt, so the act's other places are seen between them:
    // a different one as the road goes on, the same for everyone
    let painted: Vec<String> = ["", "_b", "_c", "_d", "_e"].iter().map(|v| format!("bg/act{act}_battle{v}")).filter(|s| art.image(assets, s).is_some()).collect();
    let here = painted.get(run.floors_cleared as usize % painted.len().max(1)).cloned();
    let Some(stem) = here.into_iter().chain(["bg/act1_battle".to_string()]).find(|s| art.image(assets, s).is_some()) else { return };
    let (mut image, _) = art.node(assets, &stem, theme::SOOT);
    image.color = tint;
    commands.spawn((image, Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() }, Pickable::IGNORE, ChildOf(root)));
}
