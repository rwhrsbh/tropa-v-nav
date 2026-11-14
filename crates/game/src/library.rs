//! The card library, opened from the main menu: every card of one hero at a time, laid out to
//! read. A card the hero's level has not opened yet lies face down, black, with a question mark
//! and the level that opens it.

use crate::art::Art;
use crate::l10n::L10n;
use crate::menu::{backdrop, menu_button, screen};
use crate::session::Db;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage, STAGE_W};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};
use nav_core::*;

/// The heroes in the order the lobby shows them.
const HEROES: [&str; 7] = ["bogatyr", "vorozheya", "volhv", "volkolak", "znaharka", "skomoroh", "emelya"];
/// How large a card is shown, how many to a row, and the gaps.
const K: f32 = 0.72;
const COLS: f32 = 8.0;
const GAP: f32 = 18.0;
const GRID_W: f32 = COLS * crate::battle::CARD_W * K + (COLS - 1.0) * GAP;

pub fn plugin(app: &mut App) {
    app.init_resource::<Shelf>().add_systems(OnEnter(AppState::Library), enter_library).add_systems(Update, roll.run_if(in_state(AppState::Library)));
}

/// Whose cards are on the shelf; kept from one visit to the next.
#[derive(Resource)]
struct Shelf(&'static str);

impl Default for Shelf {
    fn default() -> Self {
        Shelf(HEROES[0])
    }
}

/// What the wheel rolls.
#[derive(Component)]
struct Scroll;

/// Rarest last, and within a rarity the cards that open later after those open sooner.
fn order(def: &CardDef) -> (u8, u8) {
    let rank = match def.rarity {
        Rarity::Starter => 0,
        Rarity::Common => 1,
        Rarity::Uncommon => 2,
        Rarity::Rare => 3,
        Rarity::Special => 4,
    };
    (rank, run::card_level(def))
}

#[allow(clippy::too_many_arguments)]
fn enter_library(
    mut commands: Commands,
    stage: Single<Entity, With<Stage>>,
    shelf: Res<Shelf>,
    db: Res<Db>,
    profile: Res<crate::profile::Profile>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    use crate::battle::{CARD_H, CARD_W};
    let hero = shelf.0;
    let level = profile.hero_level(hero);
    let mut cards: Vec<&CardDef> = db.0.cards.values().filter(|d| d.hero == hero && d.kind != CardType::Curse).collect();
    let name = |d: &CardDef| l10n.t(&format!("card-{}", d.id));
    cards.sort_by(|a, b| order(a).cmp(&order(b)).then_with(|| name(a).cmp(&name(b))));
    let open = |d: &CardDef| run::card_level(d) as u32 <= level;
    let opened = cards.iter().filter(|d| open(d)).count();

    commands.spawn((DespawnOnExit(AppState::Library), backdrop(&art, &assets, "bg/menu", 0.35), ChildOf(*stage)));
    let root = commands.spawn(screen(AppState::Library, *stage)).id();
    commands.entity(root).insert(Node {
        position_type: PositionType::Absolute,
        width: px(STAGE_W),
        height: px(crate::STAGE_H),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        padding: UiRect::top(px(28)),
        row_gap: px(14),
        ..default()
    });
    commands.spawn((theme::text(l10n.t("library-title"), &fonts.title, 56.0, theme::BONE), ChildOf(root)));

    // one tab for each hero, the chosen one lit in its own colour
    let tabs = commands.spawn((Node { column_gap: px(10), ..default() }, Pickable::IGNORE, ChildOf(root))).id();
    for id in HEROES {
        let lit = id == hero;
        let hue = theme::hero_color(id);
        commands.spawn((
            Button,
            Hovered::default(),
            crate::battle::Btn(if lit { theme::PEAT } else { theme::SOOT }),
            Node { padding: UiRect::axes(px(20), px(8)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), ..default() },
            BackgroundColor(if lit { theme::PEAT } else { theme::SOOT }),
            BorderColor::all(if lit { hue } else { theme::DIM.with_alpha(0.5) }),
            children![theme::text(l10n.t(&format!("hero-{id}")), &fonts.title, 24.0, if lit { hue.lighter(0.15) } else { theme::BONE })],
            observe(move |_: On<Activate>, mut shelf: ResMut<Shelf>, mut next: ResMut<NextState<AppState>>| {
                shelf.0 = id;
                // the screen is laid out again for the hero chosen
                next.set(AppState::Library);
            }),
            ChildOf(tabs),
        ));
    }
    commands.spawn((
        theme::text(
            l10n.tf("library-count", &[("open", (opened as i32).into()), ("all", (cards.len() as i32).into()), ("level", (level as i32).into())]),
            &fonts.body,
            24.0,
            theme::CANDLE,
        ),
        ChildOf(root),
    ));

    // the cards, rolled by the wheel
    let scroller = commands
        .spawn((
            Scroll,
            Node { width: px(STAGE_W), flex_grow: 1.0, flex_basis: px(0), min_height: px(0), justify_content: JustifyContent::Center, align_items: AlignItems::FlexStart, overflow: Overflow::scroll_y(), ..default() },
            ScrollPosition::default(),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let grid = commands
        .spawn((
            Node { width: px(GRID_W), flex_wrap: FlexWrap::Wrap, column_gap: px(GAP), row_gap: px(GAP), padding: UiRect::new(px(0), px(0), px(24), px(40)), ..default() },
            Pickable::IGNORE,
            ChildOf(scroller),
        ))
        .id();
    for (i, def) in cards.iter().enumerate() {
        let slot = commands.spawn((Node { width: px(CARD_W * K), height: px(CARD_H * K), flex_shrink: 0.0, ..default() }, Pickable::IGNORE, ChildOf(grid))).id();
        if open(def) {
            crate::run_ui::shown_card(&mut commands, slot, i, &def.id, false, K, &db.0, &l10n, &fonts, &art, &assets);
        } else {
            // face down: black, a question mark, and the level that turns it over
            commands.spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    border: UiRect::all(px(3)),
                    border_radius: BorderRadius::all(px(10)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: px(8),
                    ..default()
                },
                BackgroundColor(Color::BLACK),
                BorderColor::all(theme::DIM.with_alpha(0.35)),
                Pickable::IGNORE,
                ChildOf(slot),
                children![
                    theme::text("?", &fonts.title, 110.0, theme::DIM.with_alpha(0.6)),
                    (theme::text(l10n.n("library-locked", run::card_level(def) as i32), &fonts.body, 20.0, theme::DIM), TextLayout::justify(Justify::Center)),
                ],
            ));
        }
    }

    let back = commands.spawn((menu_button(l10n.t("coop-back"), &fonts), observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu)), ChildOf(root))).id();
    commands.entity(back).insert(Node { position_type: PositionType::Absolute, right: px(40), top: px(32), width: px(240), padding: UiRect::axes(px(20), px(12)), border: UiRect::all(px(3)), justify_content: JustifyContent::Center, ..default() });
}

/// The wheel rolls the shelf.
fn roll(mut wheel: MessageReader<bevy::input::mouse::MouseWheel>, mut scrolls: Query<(&mut ScrollPosition, &ComputedNode, &Children), With<Scroll>>, nodes: Query<&ComputedNode>) {
    use bevy::input::mouse::MouseScrollUnit;
    let turned: f32 = wheel.read().map(|w| if w.unit == MouseScrollUnit::Line { w.y * 110.0 } else { w.y }).sum();
    if turned == 0.0 {
        return;
    }
    for (mut at, frame, children) in &mut scrolls {
        let seen = frame.size().y * frame.inverse_scale_factor();
        let all: f32 = children.iter().filter_map(|c| nodes.get(c).ok()).map(|n| n.size().y * n.inverse_scale_factor()).sum();
        at.0.y = (at.0.y - turned).clamp(0.0, (all - seen).max(0.0));
    }
}
