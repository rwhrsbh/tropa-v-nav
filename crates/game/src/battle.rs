//! Battle screen (design doc, section 9): heroes left, enemies right, hand at
//! the bottom, party panel top-left, Ready seal bottom-right. Reads `Session`,
//! writes only to `Outbox`.

use std::collections::HashSet;
use crate::art::Art;
use crate::l10n::L10n;
use crate::session::*;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage, STAGE_H, STAGE_W};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::{observe, Activate, Button};
use nav_core::*;
use nav_core::Event;

pub const CARD_W: f32 = 220.0;
pub const CARD_H: f32 = 310.0;
/// Original resting height; the optional raised layout lifts it by 90 stage pixels.
const HAND_TOP: f32 = 905.0;
/// Where the top of a card in focus comes to, wherever it lay in the fan: the whole of it on the
/// stage, its text with it, even grown and from the fan's low ends.
const HAND_RISEN: f32 = 745.0;
pub(crate) const HERO_SIZE: Vec2 = Vec2::new(255.0, 340.0);
const ENEMY_SIZE: Vec2 = Vec2::new(230.0, 345.0);
/// Room above a unit's sprite for its intent.
const HEAD_ROOM: f32 = 62.0;
/// Side of the picture that says what an enemy is about to do.
const INTENT_ICON: f32 = 60.0;
const UNIT_W: f32 = 280.0;
/// How long a fallen unit takes to collapse and fade, in seconds.
const DYING: f32 = 1.1;

mod aim;
mod fit_text;
mod fx;
mod gore;
mod held;
mod impact;
pub(crate) mod piles;
mod rig;
mod stands;
mod stature;
mod status_row;

pub fn plugin(app: &mut App) {
    app.add_plugins((aim::plugin, fit_text::plugin, fx::plugin, gore::plugin, held::plugin, impact::plugin, piles::plugin, rig::plugin, status_row::plugin));
    app.insert_resource(AnimSpeed(1.0))
        .init_resource::<Outbox>()
        .init_resource::<RunOutbox>()
        .init_resource::<Cues>()
        .init_resource::<HoverTop>()
        .init_resource::<HandReach>()
        .init_resource::<Shake>()
        .init_resource::<TimerText>()
        .insert_resource(Comfort { less_shake: false, ink: false })
        .init_resource::<TooltipText>()
        .add_systems(OnEnter(AppState::Battle), enter)
        .add_systems(OnExit(AppState::Battle), exit)
        .add_systems(
            Update,
            (
                playback,
                (sync_units, gore::lay_out_the_headless, sync_hand, sync_potions, cue_effects).chain(),
                no_eyes,
                (pick_hover, layout_hand, layout_units, swap_form_art, animate_sprites, update_labels, update_intent_icons, update_bars, update_tooltip),
                (floaters, fly_ghosts, dying, style_buttons, impact::shake_screen, turn_timer, send_pings, show_pings, light_bonuses),
            )
                .chain()
                .run_if(in_state(AppState::Battle))
                .run_if(resource_exists::<Session>),
        )
        .add_observer(on_drag_start)
        .add_observer(on_card_click)
        .add_observer(on_drag)
        .add_observer(on_drag_end)
        .add_observer(on_drop);
}

// ---- components ----

#[derive(Component)]
struct BattleRoot;
#[derive(Component)]
struct UnitLayer;
/// The dim plate behind an enemy's intent, so it reads over any backdrop; shown with the picture.
#[derive(Component)]
struct IntentBack(u32);

/// The picture of an enemy's intent; the number and target beside it are `Label::Intent`.
#[derive(Component)]
struct IntentIcon(u32);
#[derive(Component)]
struct HandLayer;
#[derive(Component)]
struct PotionBelt;
/// One of the player's relics in the row under the party list; hovering it says what it does.
#[derive(Component)]
struct RelicChip {
    id: String,
    at: Vec2,
}
/// A potion on the belt: click to drink, drag onto a unit to throw.
#[derive(Component)]
struct PotionChip {
    slot: u8,
    id: String,
}
#[derive(Component)]
struct FloatLayer;
#[derive(Component)]
struct Tooltip;
#[derive(Component)]
struct Dying(f32);
/// Marks a styled button; the colour is its resting background.
#[derive(Component)]
pub struct Btn(pub Color);

#[derive(Component)]
struct UnitWidget {
    unit: UnitRef,
    pos: Vec2,
}

#[derive(Component)]
struct UnitSprite {
    unit: UnitRef,
    /// Resting colour: white for real art, the hero colour for a missing sprite.
    base: Color,
    /// Art currently shown, so a change of form swaps the picture once.
    stem: String,
    t: f32,
    anim: Option<(Anim, f32)>,
    /// Drawn height, so a stretch can be taken from the feet and not from the middle.
    height: f32,
    /// How much of a dying unit is still there: 1 alive, 0 fallen.
    alive: f32,
    /// Drawn size of the picture.
    draw: Vec2,
    /// Killed and giving way to its picture lying dead: fades out (see `gore::SWAP_AT`).
    vanish: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Anim {
    Lunge,
    Hit,
    /// A skill, power or enemy ability: a short gathering gesture.
    Cast,
    /// Gained block: a cold flash.
    Guarded,
    /// Healed or raised: a warm swell.
    Healed,
    /// A buff took hold.
    Buffed,
    /// A curse or other debuff took hold: a dark shiver.
    Cursed,
}

impl Anim {
    /// A weightier animation is never cut short by a slighter one that starts the same instant.
    fn weight(self) -> u8 {
        match self {
            Anim::Hit => 5,
            Anim::Lunge => 4,
            Anim::Cast => 3,
            _ => 1,
        }
    }
}

/// A small card (or potion) flying from whoever used it to what it was used on,
/// so that every player sees every play, not only their own.
#[derive(Component)]
struct Ghost {
    from: Vec2,
    to: Vec2,
    t: f32,
}

#[derive(Component)]
struct HpFill(UnitRef);
/// The pale strip of health just lost, left standing a moment behind the fill and then
/// draining after it, so a blow is read by how much it took.
#[derive(Component)]
struct HpTrail {
    unit: UnitRef,
    /// Its width now, and the fill's last time, as fractions of the bar.
    at: f32,
    last: f32,
    hold: f32,
}
/// A health bar's frame, edged in frost while its unit has block.
#[derive(Component)]
struct HpBar(UnitRef);
#[derive(Component)]
struct BlockBadge(UnitRef);

/// Every piece of text that is refreshed from the session each frame.
#[derive(Component, Clone, Copy, PartialEq)]
enum Label {
    Hp(UnitRef),
    Block(UnitRef),
    Intent(u32),
    Team(u8),
    Energy,
    Draw,
    Discard,
    Round,
    Gold,
    Ready,
    Error,
    Tooltip,
    Timer,
}

#[derive(Component)]
struct CardWidget {
    uid: u32,
    form: Form,
    /// Drawn face down: the hero is blinded this turn.
    hidden: bool,
    pos: Vec2,
    drag: Vec2,
    dragging: bool,
    /// Lifted out of the hand and waiting above it for a target (see `aim`).
    aiming: bool,
    /// Upgraded, and made free: either changing in hand reprints the card where it is.
    look: (bool, bool),
    /// Seconds it waits at the draw pile before it comes to hand, so a draw comes one by one.
    wait: f32,
    /// Where it was last drawn on the stage, to leave the hand from.
    shown: Vec2,
    /// A power, which stays in play rather than going to a pile.
    power: bool,
}

#[derive(Component)]
struct Floater {
    ttl: f32,
    life: f32,
}

/// Screen shake: how shaken the view is, 0..1 (see `impact`), and the clock of its noise.
#[derive(Resource, Default)]
struct Shake {
    trauma: f32,
    t: f32,
}

/// Comfort settings that never touch game state.
#[derive(Resource)]
pub struct Comfort {
    pub less_shake: bool,
    /// Blood shown as black ink.
    pub ink: bool,
}

impl Comfort {
    /// The colour of a wound: fresh blood, or ink for those who want less gore.
    pub fn blood(&self) -> Color {
        if self.ink { Color::srgb(0.03, 0.03, 0.05) } else { theme::FRESH_BLOOD }
    }
}

/// The one hovered card that is on top, since fanned cards overlap.
#[derive(Resource, Default)]
struct HoverTop(Option<u32>);

/// The cursor's x over the hand, in stage pixels, while the player is reaching into it.
#[derive(Resource, Default)]
struct HandReach(Option<f32>);

// ---- layout ----

/// How far a backdrop is lifted, in stage pixels, so that its ground meets the units' feet.
pub(crate) fn ground_lift(backdrop: &str) -> f32 {
    match backdrop {
        "bg/act2_battle" => 155.0,
        "bg/act3_battle" => 140.0,
        _ => 150.0,
    }
}

/// Where the belt lies on the bar: past the longest name, health and purse.
const BELT_LEFT: f32 = 520.0;
/// Where the relics run on the bar: past the belt, short of the round.
const RELIC_LEFT: f32 = 700.0;
const RELIC_RIGHT: f32 = 1720.0;

/// The size of a relic's chip: full size while one row holds them all, then small enough for two.
fn relic_side(count: usize) -> f32 {
    let room = RELIC_RIGHT - RELIC_LEFT;
    if count as f32 * 46.0 <= room {
        42.0
    } else {
        // two rows, at least as many across as fit; past that they shrink further
        (room / (count as f32 / 2.0).ceil() - 4.0).clamp(10.0, 26.0)
    }
}

/// The place of the i-th of `count` relics in the strip.
fn relic_at(i: usize, count: usize) -> Vec2 {
    let side = relic_side(count);
    let across = (((RELIC_RIGHT - RELIC_LEFT) / (side + 4.0)) as usize).max(1);
    // one row sits in the middle of the bar, more start at its top
    let top = if count <= across { (crate::between::BAR_H - side) / 2.0 } else { 4.0 };
    Vec2::new(RELIC_LEFT + (i % across) as f32 * (side + 4.0), top + (i / across) as f32 * (side + 4.0))
}

pub(crate) fn hero_foot(slot: u8) -> Vec2 {
    // staggered front and back rows, far enough apart that every HP bar stays readable
    match slot {
        0 => Vec2::new(670.0, 700.0),
        1 => Vec2::new(480.0, 630.0),
        2 => Vec2::new(300.0, 700.0),
        _ => Vec2::new(120.0, 630.0),
    }
}

fn enemy_foot(i: usize, n: usize) -> Vec2 {
    let span = 780.0;
    let step = span / n.max(1) as f32;
    let x = 1060.0 + step * (i as f32 + 0.5);
    // a crowd staggers into two rows so sprites don't hide each other
    let y = if n > 3 && i % 2 == 1 { 610.0 } else { 700.0 };
    Vec2::new(x, y)
}

/// Feet position and sprite size of a unit on the stage.
fn unit_place(shown: &Combat, unit: UnitRef) -> Option<(Vec2, Vec2)> {
    match unit {
        UnitRef::Hero(i) => shown.hero(i).map(|_| (stands::hero(i).unwrap_or_else(|| hero_foot(i)), HERO_SIZE)),
        UnitRef::Enemy(uid) => {
            let i = shown.enemies.iter().position(|e| e.uid == uid)?;
            Some((stands::foe(uid, || shown.enemies.iter().map(|e| (e.uid, e.alive)).collect()).unwrap_or_else(|| enemy_foot(i, shown.enemies.len())), ENEMY_SIZE))
        }
    }
}

/// How large a unit's picture is drawn, for one whose picture is broader than its slot.
pub(super) fn hero_draw(stem: &str, size: Vec2) -> Vec2 {
    // the stove is broad: drawn as large as an elite, it would stand on its neighbours
    if stem == "heroes/emelya" {
        Vec2::splat(size.y * 1.08)
    } else {
        size
    }
}

/// The dark before the eyes of one who walks with no head.
#[derive(Component)]
struct NoEyes;

/// This player is up and fighting with no head: a body raised by water sees nothing but outlines.
/// One who lies dead only watches, and one sewn up again sees as ever.
fn sees_nothing(session: &Session) -> bool {
    let h = session.my_hero();
    h.walks_headless && !h.downed && !h.beheaded
}

/// With no eyes the world is black: the ground and what lies on it go, the foes' marks go, and the
/// cards in hand are empty white frames. The figures draw their own outlines (see `animate_sprites`).
fn no_eyes(
    session: Res<Session>,
    mut veil: Query<&mut Visibility, With<NoEyes>>,
    mut grounds: Query<&mut Visibility, (Or<(With<gore::StainLayer>, With<gore::CorpseLayer>)>, Without<NoEyes>)>,
    units: Query<(&UnitWidget, &Children)>,
    sprites: Query<(), With<UnitSprite>>,
    cards: Query<(&CardWidget, &Children, Entity)>,
    mut marks: Query<&mut Visibility, (Without<NoEyes>, Without<gore::StainLayer>, Without<gore::CorpseLayer>)>,
    mut edges: Query<&mut BorderColor>,
    mut was: Local<bool>,
) {
    let dark = sees_nothing(&session);
    if !dark && !*was {
        return;
    }
    *was = dark;
    let want = |seen: bool| if seen { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut veil {
        v.set_if_neq(want(dark));
    }
    for mut v in &mut grounds {
        v.set_if_neq(want(!dark));
    }
    for (unit, children) in &units {
        if !matches!(unit.unit, UnitRef::Enemy(_)) {
            continue;
        }
        for c in children.iter().filter(|c| !sprites.contains(*c)) {
            if let Ok(mut v) = marks.get_mut(c) {
                v.set_if_neq(want(!dark));
            }
        }
    }
    if dark {
        for (card, children, entity) in &cards {
            if !card.hidden {
                continue;
            }
            if let Ok(mut edge) = edges.get_mut(entity) {
                let white = BorderColor::all(Color::WHITE);
                if *edge != white {
                    *edge = white;
                }
            }
            for c in children.iter() {
                if let Ok(mut v) = marks.get_mut(c) {
                    v.set_if_neq(Visibility::Hidden);
                }
            }
        }
    }
}

fn widget_origin(foot: Vec2, size: Vec2) -> Vec2 {
    Vec2::new(foot.x - UNIT_W / 2.0, foot.y - size.y - HEAD_ROOM)
}

fn hand_spacing(n: usize) -> f32 {
    // narrow enough that the fan's ends stay clear of the energy orb and the piles
    (860.0 / n.max(1) as f32).min(CARD_W * 0.78)
}

fn hand_top(raised: bool) -> f32 {
    HAND_TOP - if raised { 90.0 } else { 0.0 }
}

fn hand_slot(i: usize, n: usize, raised: bool) -> Vec2 {
    let spacing = hand_spacing(n);
    let total = spacing * (n.max(1) - 1) as f32;
    let x = STAGE_W / 2.0 - total / 2.0 + spacing * i as f32 - CARD_W / 2.0;
    // a shallow arc: the ends of the fan sit a little lower
    let mid = (n as f32 - 1.0) / 2.0;
    let off = (i as f32 - mid).abs();
    Vec2::new(x, hand_top(raised) + off * off * 1.5)
}

// ---- screen setup ----

fn enter(
    mut commands: Commands,
    session: Option<Res<Session>>,
    net: Option<Res<crate::net::Net>>,
    profile: Res<crate::profile::Profile>,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    // the session is built from the run's state before this screen opens
    let Some(session) = session else {
        warn!("battle opened with nothing to fight");
        return;
    };
    let layer = || Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() };
    let run = net.as_ref().and_then(|n| n.node.state.as_ref());
    let act = run.map(|r| r.act.min(3)).unwrap_or(1);
    // where this fight is: a boss has an arena of its own, an elite a lair, and ordinary fights
    // walk through the act's places in turn, so that two in a row never look alike.
    // It is read from the run alone, so every player sees the same place
    let place: Vec<String> = {
        let real_act = run.map(|r| r.act).unwrap_or(1);
        let step = run.map(|r| r.floors_cleared as usize).unwrap_or(0);
        let mut want = Vec::new();
        // a foe with a haunt of its own is met there: the drowned by the mill pond, the Leshy
        // in the heart of the forest. The first such foe in the fight decides
        for e in &session.truth.enemies {
            want.push(format!("bg/at_{}", e.def));
        }
        match run.and_then(|r| r.fighting) {
            Some(nav_core::NodeKind::Boss) if real_act >= 4 => want.push("bg/koschei".to_string()),
            Some(nav_core::NodeKind::Boss) => want.push(format!("bg/boss_act{act}")),
            Some(nav_core::NodeKind::Elite) => want.push(format!("bg/elite_act{act}")),
            _ => {}
        }
        let painted: Vec<String> = ["", "_b", "_c", "_d", "_e"].iter().map(|v| format!("bg/act{act}_battle{v}")).filter(|s| art.image(&assets, s).is_some()).collect();
        if !painted.is_empty() {
            want.push(painted[step % painted.len()].clone());
        }
        want.push(format!("bg/act{act}_battle"));
        want.push("bg/act1_battle".to_string());
        want
    };

    let root = commands
        .spawn((BattleRoot, layer(), UiTransform::IDENTITY, Pickable::IGNORE, ChildOf(*stage)))
        .with_children(|p| {
            // until a place is painted, the act's first backdrop stands in, and the forest for that
            let backdrop = place.iter().find(|s| art.image(&assets, s).is_some()).cloned().unwrap_or_default();
            // every backdrop is painted with its open ground at about three quarters of its height;
            // units stand higher than that to leave room for the hand, so the picture is lifted
            // until that ground lies under their feet, and the strip it leaves is the hand's dark apron
            let lift = ground_lift(&backdrop);
            stands::enter(&art, &backdrop, lift);
            p.spawn((
                Node { position_type: PositionType::Absolute, left: px(0), top: px(-lift), width: px(STAGE_W), height: px(STAGE_H), ..default() },
                art.node(&assets, &backdrop, theme::SOOT),
                Pickable::IGNORE,
            ));
            for (i, alpha) in [0.18, 0.4, 0.62, 0.82, 1.0].into_iter().enumerate() {
                let top = STAGE_H - lift - 120.0 + 30.0 * i as f32;
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: px(0), top: px(top), width: px(STAGE_W), height: px(if i == 4 { lift + 1.0 } else { 30.0 }), ..default() },
                    BackgroundColor(theme::SOOT.with_alpha(alpha)),
                    Pickable::IGNORE,
                ));
            }
            // blood soaks into the ground under everyone
            p.spawn((NoEyes, Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: px(STAGE_W), height: px(STAGE_H), ..default() }, BackgroundColor(Color::BLACK), Visibility::Hidden, Pickable::IGNORE));
            p.spawn((gore::StainLayer, layer(), Pickable::IGNORE));
            p.spawn((gore::CorpseLayer, layer(), Pickable::IGNORE));
            p.spawn((UnitLayer, layer(), Pickable::IGNORE));

            // the bar along the top, as between fights: who you are, health, purse; the belt and
            // the relics lie over it, the round at its right end
            let mine = session.my_hero();
            p.spawn((crate::between::bar_plate(), Pickable::IGNORE)).with_children(|bar| {
                bar.spawn(theme::text(l10n.t(&format!("hero-{}", mine.def)), &fonts.title, 30.0, theme::hero_color(&mine.def).lighter(0.1)));
                bar.spawn(crate::between::bar_stat()).with_children(|s| {
                    s.spawn(crate::between::blood_mark());
                    s.spawn((Label::Hp(UnitRef::Hero(mine.slot)), theme::text("", &fonts.num, 28.0, theme::FRESH_BLOOD.lighter(0.2)), TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK }));
                });
                bar.spawn(crate::between::bar_stat()).with_children(|s| {
                    s.spawn(crate::between::coin_mark());
                    s.spawn((Label::Gold, theme::text("", &fonts.num, 28.0, theme::CANDLE), TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK }));
                });
            });

            // party panel under the bar, top-left: alone, the bar says it all
            let party = session.truth.heroes.len() > 1;
            p.spawn((
                theme::panel(Node {
                    position_type: PositionType::Absolute,
                    left: px(16),
                    top: px(crate::between::BAR_H + 10.0),
                    display: if party { Display::Flex } else { Display::None },
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(2)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    ..default()
                }),
                Pickable::IGNORE,
            ))
            .with_children(|team| {
                for h in &session.truth.heroes {
                    // pressed, opens what that player carries
                    team.spawn((
                        Button,
                        Hovered::default(),
                        Btn(theme::PEAT),
                        crate::between::PlayerSheet(h.slot),
                        BackgroundColor(theme::PEAT),
                        observe(crate::between::open_player),
                        Node { column_gap: px(10), align_items: AlignItems::Center, padding: UiRect::axes(px(6), px(3)), ..default() },
                        children![
                            (
                                Node { width: px(34), height: px(34), border: UiRect::all(px(2)), ..default() },
                                art.node(&assets, &format!("portraits/{}", h.def), theme::hero_color(&h.def)),
                                BorderColor::all(theme::hero_color(&h.def)),
                                Pickable::IGNORE,
                            ),
                            (Label::Team(h.slot), theme::text("", &fonts.body, 20.0, theme::BONE)),
                        ],
                    ));
                }
            });

            // what the player carries, along the bar: a picture where one is painted, else the
            // first letters of its name
            let count = mine.relics.len();
            let side = relic_side(count);
            for (i, id) in mine.relics.iter().enumerate() {
                let at = relic_at(i, count);
                let picture = art.image(&assets, &format!("relics/{id}"));
                let mut chip = p.spawn((
                    RelicChip { id: id.clone(), at },
                    Hovered::default(),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(at.x),
                        top: px(at.y),
                        width: px(side),
                        height: px(side),
                        border: UiRect::all(px(2)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(theme::SOOT.with_alpha(0.88)),
                    BorderColor::all(theme::CANDLE.with_alpha(0.7)),
                ));
                match picture {
                    Some(h) => {
                        chip.with_child((ImageNode::new(h), Node { width: percent(100), height: percent(100), ..default() }, Pickable::IGNORE));
                    }
                    None => {
                        let short: String = l10n.t(&format!("relic-{id}")).chars().filter(|c| c.is_alphabetic()).take(2).collect();
                        chip.with_child(theme::text(short, &fonts.title, 17.0, theme::BONE));
                    }
                }
            }

            p.spawn((
                Label::Timer,
                theme::text("", &fonts.num, 30.0, theme::CANDLE),
                Node { position_type: PositionType::Absolute, right: px(24), top: px(crate::between::BAR_H + 8.0), ..default() },
            ));
            // the very first fight explains itself, briefly
            if profile.runs == 0 {
                p.spawn((
                    theme::panel(Node {
                        position_type: PositionType::Absolute,
                        left: px(560),
                        top: px(crate::between::BAR_H + 10.0),
                        width: px(800),
                        padding: UiRect::all(px(14)),
                        border: UiRect::all(px(2)),
                        ..default()
                    }),
                    Pickable::IGNORE,
                    children![theme::text(l10n.t("battle-tutorial"), &fonts.body, 20.0, theme::BONE)],
                ));
            }
            p.spawn((
                Label::Round,
                theme::text("", &fonts.title, 26.0, theme::BONE),
                Node { position_type: PositionType::Absolute, right: px(24), top: px(16), ..default() },
            ));

            // energy: burning splinters left of the hand
            p.spawn((
                theme::panel_edged(Node {
                    position_type: PositionType::Absolute,
                    left: px(300),
                    top: px(900),
                    width: px(120),
                    height: px(120),
                    border: UiRect::all(px(3)),
                    border_radius: BorderRadius::MAX,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                }, theme::CANDLE),
                Pickable::IGNORE,
                children![
                    (Label::Energy, theme::text("", &fonts.num, 46.0, theme::CANDLE)),
                    theme::text(l10n.t("battle-energy"), &fonts.body, 16.0, theme::DIM),
                ],
            ));

            // the piles: pressed, each spreads its cards out (see `piles`)
            p.spawn(piles::pile_plate(140.0, piles::PileKind::Draw, Label::Draw, &l10n, &fonts));
            p.spawn(piles::pile_plate(1780.0, piles::PileKind::Discard, Label::Discard, &l10n, &fonts));

            // the Ready seal
            p.spawn((
                Button,
                Hovered::default(),
                Btn(theme::RUST_BLOOD),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(1640),
                    top: px(812),
                    width: px(240),
                    height: px(96),
                    border: UiRect::all(px(3)),
                    border_radius: BorderRadius::all(px(48)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(theme::RUST_BLOOD),
                BorderColor::all(theme::CANDLE),
                children![(Label::Ready, theme::text("", &fonts.title, 26.0, theme::BONE), TextLayout::justify(Justify::Center))],
                observe(|_: On<Activate>, session: Res<Session>, mut outbox: ResMut<Outbox>| {
                    let h = session.my_hero();
                    if !h.downed && session.truth.phase == Phase::Heroes {
                        outbox.0.push(CombatAction::Ready(!h.ready));
                    }
                }),
            ));

            p.spawn((
                PotionBelt,
                // on the bar, as between fights, and the relics after it
                Node { position_type: PositionType::Absolute, left: px(BELT_LEFT), top: px(7), column_gap: px(10), ..default() },
                Pickable::IGNORE,
            ));
            // the hand over every unit, bar and intent: a card held is never under anything
            p.spawn((HandLayer, layer(), Pickable::IGNORE, GlobalZIndex(30)));
            p.spawn((FloatLayer, layer(), Pickable::IGNORE, GlobalZIndex(50)));
            // the aiming arrow, over everything
            p.spawn((layer(), Pickable::IGNORE)).with_children(|a| aim::spawn_arrow(a, &mut images));

            p.spawn((
                Label::Error,
                theme::text("", &fonts.body, 26.0, theme::FRESH_BLOOD),
                Node { position_type: PositionType::Absolute, top: px(700), width: px(STAGE_W), justify_content: JustifyContent::Center, ..default() },
                TextLayout::justify(Justify::Center),
                GlobalZIndex(60),
            ));

            p.spawn((
                Tooltip,
                theme::panel(Node {
                    position_type: PositionType::Absolute,
                    width: px(330),
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(2)),
                    ..default()
                }),
                Visibility::Hidden,
                GlobalZIndex(70),
                Pickable::IGNORE,
                children![(Label::Tooltip, theme::text("", &fonts.body, 18.0, theme::BONE))],
            ));
        })
        .id();
    let _ = root;
}

fn exit(mut commands: Commands, roots: Query<Entity, With<BattleRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<Session>();
}

// ---- units ----

fn sync_units(
    mut commands: Commands,
    session: Res<Session>,
    layer: Single<Entity, With<UnitLayer>>,
    widgets: Query<(Entity, &UnitWidget, Has<Dying>)>,
    corpses: Query<(Entity, &gore::Corpse)>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut rigs: ResMut<Assets<rig::RigMaterial>>,
) {
    let shown = &session.shown;
    let have = |u: UnitRef| widgets.iter().find(|(_, w, _)| w.unit == u);
    let mut wanted: Vec<(UnitRef, String, String, Color)> = Vec::new();
    for h in &shown.heroes {
        // the head gone, he falls as a foe falls, and is not seen again
        if let (true, Some((entity, _, false))) = (h.beheaded, have(UnitRef::Hero(h.slot))) {
            commands.entity(entity).insert(Dying(DYING));
        }
        if have(UnitRef::Hero(h.slot)).is_none() && !h.beheaded {
            wanted.push((UnitRef::Hero(h.slot), format!("heroes/{}", h.def), l10n.t(&format!("hero-{}", h.def)), theme::hero_color(&h.def)));
        }
    }
    for e in &shown.enemies {
        let unit = UnitRef::Enemy(e.uid);
        match have(unit) {
            None if e.alive => {
                // raised from the dead: it gets up, and its body no longer lies there
                for (body, corpse) in &corpses {
                    if corpse.unit == unit {
                        corpse.clear_away(&mut commands, body);
                    }
                }
                wanted.push((unit, format!("enemies/{}", e.def), l10n.t(&format!("enemy-{}", e.def)), theme::SWAMP));
            }
            // one that ran off is simply gone from the field, with no body left behind
            Some((entity, _, _)) if !e.alive && e.fled => {
                commands.entity(entity).despawn();
            }
            Some((entity, _, false)) if !e.alive => {
                commands.entity(entity).insert(Dying(DYING));
            }
            _ => {}
        }
    }
    for (unit, stem, name, color) in wanted {
        let Some((foot, size)) = unit_place(shown, unit) else { continue };
        let origin = widget_origin(foot, size);
        let is_hero = matches!(unit, UnitRef::Hero(_));
        // elites and bosses are painted square and stand larger than their slot: the picture
        // spills over the sides and the top, the layout box stays put
        let wide = (!is_hero || stem == "heroes/emelya") && art.aspect(&stem).is_some_and(|a| a > 0.9);
        // a foe of known stature is drawn as tall as it stands, a raven small, a giant huge
        let tall = match unit {
            UnitRef::Enemy(uid) => shown.enemy(uid).and_then(|e| stature::height_of(&e.def)),
            UnitRef::Hero(_) => None,
        };
        let draw = match tall {
            Some(h) => Vec2::new(h * art.aspect(&stem).unwrap_or(size.x / size.y), h),
            // the stove is broad: drawn as large as an elite, it would stand on its neighbours
            None if stem == "heroes/emelya" => Vec2::splat(size.y * 1.08),
            None if wide => Vec2::splat(size.y * 1.25),
            None => size,
        };
        commands
            .spawn((
                UnitWidget { unit, pos: origin },
                Hovered::default(),
                Pickable::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(origin.x),
                    top: px(origin.y),
                    width: px(UNIT_W),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },
                ZIndex(foot.y as i32),
                ChildOf(*layer),
            ))
            .with_children(|u| {
                u.spawn((
                    // over a picture that spills above its slot, the intent rises with it, so it
                    // stays over the head and not on the face
                    Node { height: px(HEAD_ROOM), align_items: AlignItems::Center, top: px(-(draw.y - size.y)), ..default() },
                    // text is never hidden by a sprite, this unit's or a neighbour's
                    GlobalZIndex(20),
                    Pickable::IGNORE,
                ))
                .with_children(|head| {
                    // heroes show nothing overhead; the slot keeps both sides aligned
                    if let UnitRef::Enemy(uid) = unit {
                        head.spawn((
                            IntentBack(uid),
                            Node { align_items: AlignItems::Center, padding: UiRect::axes(px(8), px(3)), border_radius: BorderRadius::all(px(12)), ..default() },
                            BackgroundColor(theme::SOOT.with_alpha(0.68)),
                            Visibility::Hidden,
                        ))
                        .with_children(|plate| {
                            plate.spawn((
                                IntentIcon(uid),
                                ImageNode::default(),
                                Node { width: px(INTENT_ICON), height: px(INTENT_ICON), margin: UiRect::right(px(4)), ..default() },
                                Visibility::Hidden,
                            ));
                            plate.spawn((Label::Intent(uid), theme::text("", &fonts.num, 26.0, theme::BONE), TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK.with_alpha(0.9) }));
                        });
                    }
                });
                // the feet are painted a little above the bottom edge of the picture
                // a soft contact shadow, feathered out to nothing at its edge
                u.spawn((
                    ImageNode { color: Color::WHITE.with_alpha(0.75), ..ImageNode::new(assets.load(crate::art::path("fx/shadow.png"))) },
                    Node {
                        position_type: PositionType::Absolute,
                        left: px((UNIT_W - size.x * 0.95) / 2.0),
                        top: px(HEAD_ROOM + size.y * 0.957 - 20.0),
                        width: px(size.x * 0.95),
                        height: px(40),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
                // drawn through the rig, so the figure bends as if it had a body
                let picture = art.image(&assets, &stem);
                let base = if picture.is_some() { Color::WHITE } else { color };
                // a figure cut into parts is drawn as its body and the parts over it
                let (skin, parts) = match rig::parts_of(&art, &assets, &stem) {
                    Some((body, parts)) => (
                        rig::skin(&mut rigs, Some(body), base, draw, is_hero, None),
                        parts.into_iter().map(|(cut, picture)| rig::skin(&mut rigs, Some(picture), base, draw, is_hero, Some(cut))).collect(),
                    ),
                    None => (rig::skin(&mut rigs, picture, base, draw, is_hero, None), Vec::new()),
                };
                u.spawn((
                    UnitSprite { unit, base, stem: stem.clone(), t: if is_hero { 0.0 } else { 1.3 }, anim: None, height: draw.y, alive: 1.0, draw, vanish: false },
                    {
                        Node {
                            width: px(draw.x),
                            height: px(draw.y),
                            margin: UiRect { left: px((size.x - draw.x) / 2.0), right: px((size.x - draw.x) / 2.0), top: px(size.y - draw.y), bottom: px(0) },
                            flex_shrink: 0.0,
                            ..default()
                        }
                    },
                    UiTransform::IDENTITY,
                    Pickable::IGNORE,
                    children![skin],
                ))
                .with_children(|s| {
                    for part in parts {
                        s.spawn(part);
                    }
                });
                u.spawn((
                    Node {
                        width: px(180),
                        height: px(22),
                        margin: UiRect::top(px(8)),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(7)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    HpBar(unit),
                    BackgroundColor(Color::srgb(0.05, 0.04, 0.04)),
                    BorderColor::all(Color::srgb(0.32, 0.26, 0.20)),
                    BoxShadow::new(Color::BLACK.with_alpha(0.7), px(0), px(2), px(0), px(6)),
                    GlobalZIndex(20),
                    Pickable::IGNORE,
                    children![
                        (
                            HpTrail { unit, at: 1.0, last: 1.0, hold: 0.0 },
                            Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: percent(100), height: percent(100), border_radius: BorderRadius::all(px(5)), ..default() },
                            BackgroundColor(Color::srgb(0.93, 0.78, 0.52)),
                            Pickable::IGNORE,
                        ),
                        (
                            HpFill(unit),
                            Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: percent(100), height: percent(100), border_radius: BorderRadius::all(px(5)), ..default() },
                            BackgroundColor(theme::FRESH_BLOOD),
                            Pickable::IGNORE,
                        ),
                        (Label::Hp(unit), theme::text("", &fonts.num, 17.0, theme::BONE)),
                        (
                            BlockBadge(unit),
                            // half over its own bar and over a neighbour's: bars stand side by side
                            GlobalZIndex(21),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(-22),
                                top: px(-8),
                                width: px(40),
                                height: px(34),
                                border: UiRect::all(px(2)),
                                border_radius: BorderRadius::all(px(6)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(theme::NAV_BLUE),
                            BorderColor::all(theme::FROST),
                            Visibility::Hidden,
                            Pickable::IGNORE,
                            children![(Label::Block(unit), theme::text("", &fonts.num, 20.0, theme::BONE))],
                        ),
                    ],
                ));
                // what the card or potion being aimed would do to this unit
                u.spawn((
                    aim::Forecast(unit),
                    theme::text("", &fonts.num, 26.0, theme::BONE),
                    TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK.with_alpha(0.9) },
                    Node { position_type: PositionType::Absolute, top: px(HEAD_ROOM + size.y - 40.0), ..default() },
                    GlobalZIndex(60),
                    Visibility::Hidden,
                ));
                u.spawn((theme::text(name, &fonts.body, 17.0, theme::BONE), Node { margin: UiRect::top(px(3)), ..default() }));
                // what lies on the unit, as pictures with numbers (see `status_row`)
                u.spawn(status_row::status_row(unit));
            });
    }
}

fn layout_units(session: Res<Session>, time: Res<Time>, mut widgets: Query<(&mut UnitWidget, &mut Node, &mut ZIndex)>) {
    let k = (time.delta_secs() * 10.0).min(1.0);
    for (mut w, mut node, mut z) in &mut widgets {
        let Some((foot, size)) = unit_place(&session.shown, w.unit) else { continue };
        let target = widget_origin(foot, size);
        if w.pos.distance(target) > 0.5 {
            w.pos = w.pos.lerp(target, k);
            node.left = px(w.pos.x);
            node.top = px(w.pos.y);
            z.0 = foot.y as i32;
        }
    }
}

/// The werewolf has a second body: when the form changes, so does the sprite.
/// Pictures a hero may change into mid-fight (a blow, the Edge, the other form), loaded as the
/// fight begins: one loaded only when first wanted would come a frame or more too late.
#[derive(Default)]
struct Preloaded {
    handles: Vec<Handle<Image>>,
    /// Pictures already asked for, by unit: heroes once, a foe summoned mid-fight when it comes.
    seen: HashSet<String>,
}

/// Poses a unit is drawn in for a moment, over its own picture.
const POSES: [&str; 9] = ["_strike", "_cast", "_hurt", "_guard", "_downed", "_headless", "_noweapon", "_noshield", "_noarms"];

/// The painted picture of a figure short of a part. Where none was painted for it, the figure
/// stands in its cut-out parts.
pub(crate) fn short_of(no_head: bool, lost_arms: u8) -> Option<&'static str> {
    use nav_core::combat::{SHIELD_ARM, WEAPON_ARM};
    match (no_head, lost_arms) {
        (true, 0) => Some("_headless"),
        (false, a) if a == WEAPON_ARM => Some("_noweapon"),
        (false, a) if a == SHIELD_ARM => Some("_noshield"),
        (false, a) if a == WEAPON_ARM | SHIELD_ARM => Some("_noarms"),
        (true, a) if a == WEAPON_ARM => Some("_headless_noweapon"),
        (true, a) if a == SHIELD_ARM => Some("_headless_noshield"),
        (true, a) if a == WEAPON_ARM | SHIELD_ARM => Some("_headless_noarms"),
        _ => None,
    }
}

/// The unit's own picture under whatever pose it is drawn in for the moment.
pub(crate) fn pose_base(mut stem: &str) -> &str {
    // a maimed hero's pose carries two marks: `_noarms_strike`
    while let Some(less) = POSES.iter().find_map(|p| stem.strip_suffix(p)) {
        stem = less;
    }
    stem
}

fn swap_form_art(
    session: Res<Session>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut preloaded: Local<Preloaded>,
    mut sprites: Query<(&mut UnitSprite, &Children)>,
    skins: Query<(&MaterialNode<rig::RigMaterial>, &rig::RigSkin, &mut Visibility)>,
    mut rigs: ResMut<Assets<rig::RigMaterial>>,
) {
    let mut skins = skins;
    // every picture a unit may change into is loaded before it is needed, so a first blow
    // is not drawn in the resting pose
    let stems = session.shown.heroes.iter().flat_map(|h| [format!("heroes/{}", h.def), format!("heroes/{}_beast", h.def)]).chain(session.shown.enemies.iter().map(|e| format!("enemies/{}", e.def)));
    for stem in stems {
        if preloaded.seen.insert(stem.clone()) {
            for mark in ["", "_headless", "_noweapon", "_noshield", "_noarms", "_headless_noweapon", "_headless_noshield", "_headless_noarms"] {
                for pose in ["", "_strike", "_cast", "_hurt", "_guard", "_downed"] {
                    if let Some(handle) = art.image(&assets, &format!("{stem}{mark}{pose}")) {
                        preloaded.handles.push(handle);
                    }
                }
            }
        }
    }
    for (mut s, children) in &mut sprites {
        let mut whole = true;
        let (mut want, downed) = match s.unit {
            UnitRef::Hero(i) => {
                let Some(h) = session.shown.hero(i) else { continue };
                // a lost limb is on no finished picture: he stands in his cut-out parts
                whole = h.lost_limbs == 0 && !h.walks_headless;
                (if h.form == Form::Beast { format!("heroes/{}_beast", h.def) } else { format!("heroes/{}", h.def) }, h.downed && !h.beheaded)
            }
            UnitRef::Enemy(uid) => {
                // a foe falling keeps the picture it fell in
                let Some(e) = session.shown.enemy(uid).filter(|e| e.alive && !s.vanish) else { continue };
                (format!("enemies/{}", e.def), false)
            }
        };
        // through the heart of a lunge the unit is drawn mid-blow, through a skill or power
        // calling on it, and as a blow lands reeling from it, when those pictures exist
        let pose = match s.anim {
            Some((Anim::Lunge, t)) if (0.12..0.38).contains(&t) => Some("_strike"),
            Some((Anim::Cast, t)) if (0.06..0.40).contains(&t) => Some("_cast"),
            Some((Anim::Hit, t)) if t < 0.26 => Some("_hurt"),
            Some((Anim::Guarded, t)) if t < 0.36 => Some("_guard"),
            _ => None,
        };
        // who is short of a part wears their own picture of it, when one was painted
        let short = if let UnitRef::Hero(i) = s.unit { session.shown.hero(i).and_then(|h| short_of(h.walks_headless, h.lost_arms)) } else { None };
        let mut painted = whole;
        if let Some(mark) = short {
            if art.image(&assets, &format!("{want}{mark}")).is_some() {
                want = format!("{want}{mark}");
                painted = true;
            }
        }
        // on the Edge a hero is drawn down on one knee, wounded, when that picture exists: the
        // maimed too, in a picture of their own short of what they lost
        if downed && painted && art.image(&assets, &format!("{want}_downed")).is_some() {
            want = format!("{want}_downed");
        }
        if let Some(pose) = pose.filter(|_| !downed && painted) {
            if art.image(&assets, &format!("{want}{pose}")).is_some() {
                want = format!("{want}{pose}");
            }
        }
        if s.stem == want {
            continue;
        }
        // no beast art for this hero: keep what is shown rather than blank the sprite
        if let Some(handle) = art.image(&assets, &want) {
            // a form cut into parts shows its body and parts; one that is not, its whole
            // picture, the parts' skins hidden
            let parts = rig::parts_of(&art, &assets, &want);
            let mut iter = skins.iter_many_mut(children);
            while let Some(Ok((skin, part, mut vis))) = iter.fetch_next() {
                match (&parts, part.part) {
                    (Some((body, _)), None) => rig::repaint(&mut rigs, skin, body.clone()),
                    (Some((_, parts)), Some(p)) => match parts.iter().find(|(cut, _)| cut.0 == p) {
                        Some((_, picture)) => {
                            rig::repaint(&mut rigs, skin, picture.clone());
                            *vis = Visibility::Inherited;
                        }
                        None => *vis = Visibility::Hidden,
                    },
                    (None, None) => rig::repaint(&mut rigs, skin, handle.clone()),
                    (None, Some(_)) => *vis = Visibility::Hidden,
                }
            }
        }
        s.stem = want;
    }
}

fn animate_sprites(
    session: Res<Session>,
    time: Res<Time>,
    speed: Res<AnimSpeed>,
    comfort: Res<Comfort>,
    mut sprites: Query<(&mut UnitSprite, &mut UiTransform, &Children)>,
    skins: Query<(&MaterialNode<rig::RigMaterial>, &rig::RigSkin)>,
    mut rigs: ResMut<Assets<rig::RigMaterial>>,
) {
    let dt = time.delta_secs() * speed.0;
    for (mut s, mut tf, children) in &mut sprites {
        s.t += dt;
        // idle: slow breathing, feet planted (the rig sways and ripples the figure itself)
        let breath = (s.t * 2.5).sin();
        let mut scale = Vec2::new(1.0 - breath * 0.004, 1.0 + breath * 0.008);
        let mut offset = Vec2::ZERO;
        let hero = matches!(s.unit, UnitRef::Hero(_));
        let toward = if hero { 1.0 } else { -1.0 };
        let mut tint = s.base;
        let mut pose = rig::Pose::idle(s.base, hero);
        pose.outline = sees_nothing(&session);
        // a body short of its parts cannot hold itself up: it lurches where it stands
        let unsteady = matches!(s.unit, UnitRef::Hero(i) if session.shown.hero(i).is_some_and(|h| h.walks_headless || h.lost_limbs > 0));
        if unsteady {
            pose.sway *= 3.0;
            pose.lean += 0.035 * (s.t * 1.1).sin() + 0.015 * (s.t * 2.7).sin();
        }
        // the weapon arm hangs and stirs a little
        // the parts stir at rest, each in its own time: the weapon hangs heavy, the head looks
        // about, the shield arm tires and is lifted again
        pose.swing = 0.035 * (s.t * 1.3).sin();
        pose.nod = 0.025 * (s.t * 0.7 + 1.0).sin() + 0.012 * (s.t * 1.9).sin();
        pose.guard = 0.02 * (s.t * 0.9 + 2.0).sin();

        let cut = rig::is_cut(pose_base(&s.stem));
        if let Some((anim, t)) = &mut s.anim {
            *t += dt;
            let dur = match anim {
                // a weapon swung from on high takes its time
                Anim::Lunge if cut => 0.5,
                Anim::Lunge | Anim::Hit => 0.30,
                Anim::Cast => 0.45,
                Anim::Guarded | Anim::Cursed => 0.40,
                Anim::Healed | Anim::Buffed => 0.55,
            };
            let done = *t >= dur;
            let p = (*t / dur).clamp(0.0, 1.0);
            let swell = (p * std::f32::consts::PI).sin();
            match anim {
                Anim::Cast => {
                    // gather, then release toward the other side
                    offset.y -= 14.0 * swell;
                    offset.x += toward * 12.0 * swell;
                    scale *= 1.0 + 0.05 * swell;
                    // drawn back, then thrown forward
                    pose.lean = if p < 0.4 { -0.05 * (p / 0.4) } else { 0.09 * ((p - 0.4) / 0.6 * std::f32::consts::PI).sin() };
                    pose.swing = -0.35 * swell;
                    // the head thrown back to call it down, the shield drawn aside
                    pose.nod = -0.18 * swell;
                    pose.guard = 0.12 * swell;
                    pose.wave = 0.012 * swell;
                    pose.flash = 0.08 * swell;
                    pose.flash_color = Color::srgb(0.8, 0.85, 1.0);
                }
                Anim::Guarded => {
                    scale *= Vec2::new(1.0 + 0.04 * swell, 1.0 - 0.02 * swell);
                    pose.squash = 1.0 - 0.04 * swell;
                    pose.lean = -0.025 * swell;
                    // the shield brought up before the body, the head ducked behind it
                    pose.guard = -0.35 * swell;
                    pose.nod = 0.12 * swell;
                    tint = s.base.mix(&Color::srgb(0.45, 0.65, 1.0), 0.3 * swell);
                }
                Anim::Healed => {
                    offset.y -= 8.0 * swell;
                    pose.squash = 1.0 + 0.035 * swell;
                    pose.flash = 0.09 * swell;
                    pose.flash_color = Color::srgb(0.7, 1.0, 0.6);
                    scale *= 1.0 + 0.04 * swell;
                    tint = s.base.mix(&Color::srgb(0.65, 1.0, 0.55), 0.3 * swell);
                }
                Anim::Buffed => {
                    scale *= 1.0 + 0.07 * swell;
                    pose.squash = 1.0 + 0.05 * swell;
                    pose.flash = 0.1 * swell;
                    pose.flash_color = Color::srgb(1.0, 0.85, 0.45);
                    tint = s.base.mix(&Color::srgb(1.0, 0.8, 0.4), 0.22 * swell);
                }
                Anim::Cursed => {
                    offset.x += 6.0 * (p * 38.0).sin() * (1.0 - p);
                    // a shudder runs up the body
                    pose.wave = 0.03 * (1.0 - p);
                    pose.squash = 1.0 - 0.05 * swell;
                    tint = s.base.mix(&Color::srgb(0.25, 0.4, 0.15), 0.6 * swell);
                }
                Anim::Lunge => {
                    offset.x += toward * 70.0 * (p * std::f32::consts::PI).sin();
                    // a crouch to wind up, then the whole body thrown into the blow
                    pose.squash = if p < 0.25 { 1.0 - 0.06 * (p / 0.25) } else { 0.94 + 0.1 * ((p - 0.25) / 0.75 * std::f32::consts::PI).sin() };
                    pose.lean = if p < 0.25 { -0.03 * (p / 0.25) } else { 0.16 * ((p - 0.25) / 0.75 * std::f32::consts::PI).sin() };
                    // the weapon raised high while crouching, then brought down through the blow
                    pose.swing = if p < 0.35 {
                        -1.9 * (p / 0.35).powf(0.6)
                    } else if p < 0.55 {
                        -1.9 + 2.4 * ((p - 0.35) / 0.2).powi(2)
                    } else {
                        0.5 * (1.0 - (p - 0.55) / 0.45)
                    };
                    // the head goes down into the blow, the shield tucked in
                    pose.nod = if p < 0.35 { -0.08 * p / 0.35 } else { 0.16 * ((p - 0.35) / 0.65 * std::f32::consts::PI).sin() };
                    pose.guard = 0.1 * swell;
                }
                Anim::Hit => {
                    offset.x -= toward * 22.0 * (1.0 - p) * (p * 40.0).cos().abs();
                    scale.x *= 1.0 - 0.04 * (1.0 - p);
                    // bent back by the blow, the head going furthest, and springing upright
                    pose.lean = -0.13 * (1.0 - p) * (1.0 + 0.4 * (p * 18.0).cos());
                    // the arm flung out by the blow
                    pose.swing = -0.3 * (1.0 - p) * (p * 14.0).cos();
                    // the head snapped back and the shield knocked aside
                    pose.nod = -0.32 * (1.0 - p) * (1.0 + 0.3 * (p * 16.0).cos());
                    pose.guard = 0.22 * (1.0 - p) * (p * 11.0).cos();
                    pose.squash = 1.0 - 0.05 * (1.0 - p);
                    // a white-hot instant, then the blow's colour fading off
                    // (the flash mixes in linear light, where a little goes a long way; the
                    // colour is a tint, so the painting still shows through it)
                    if p < 0.2 {
                        pose.flash = 0.45 * (1.0 - p / 0.2);
                        pose.flash_color = Color::WHITE;
                    }
                    if p < 0.6 {
                        let k = 1.0 - p / 0.6;
                        let blow = if comfort.ink { Color::srgb(0.35, 0.35, 0.4) } else { Color::srgb(1.0, 0.35, 0.3) };
                        tint = s.base.mix(&blow, 0.8 * k);
                    }
                }
            }
            if done {
                s.anim = None;
            }
        }
        if let UnitRef::Hero(i) = s.unit {
            if session.shown.hero(i).is_some_and(|h| h.downed) {
                // on the Edge: slumped and dark; a picture already kneeling is only dimmed
                let kneeling = s.stem.ends_with("_downed");
                tint = s.base.mix(&Color::srgb(0.05, 0.05, 0.08), if kneeling { 0.35 } else { 0.72 });
                if !kneeling {
                    offset.y += 16.0;
                    scale *= Vec2::new(1.0, 0.86);
                }
                pose.lean = -0.06;
                pose.swing = 0.3;
                pose.nod = 0.3;
                pose.guard = 0.25;
                pose.sway = 0.002;
                pose.breath = 0.006;
            }
        }
        if let UnitRef::Enemy(uid) = s.unit {
            if session.shown.enemy(uid).is_some_and(|e| e.flags.contains_key(nav_core::state::BONES)) {
                // fallen apart, a heap about to get up: low, dark and still
                tint = s.base.mix(&Color::srgb(0.08, 0.07, 0.06), 0.6);
                offset.y += s.height * 0.3;
                scale *= Vec2::new(1.08, 0.55);
                pose.nod = 0.5;
                pose.swing = 0.6;
                pose.sway = 0.0;
                pose.breath = 0.0;
            }
        }
        if s.alive < 1.0 {
            // dying: the knees give first and the body sinks on them, the head drops, the arms
            // go slack and swing; then it goes over (see `gore`) and lies
            let k = 1.0 - s.alive;
            let knees = (k / 0.45).min(1.0);
            pose.squash = 1.0 - 0.16 * knees * knees;
            pose.lean = -0.05 * knees - 0.1 * k * k;
            pose.nod = 0.35 * knees - 0.5 * (k - 0.45).max(0.0);
            pose.swing = 0.5 * knees + 0.25 * (k * 11.0).sin() * (1.0 - k);
            pose.guard = 0.4 * knees;
            pose.swing = 0.7 * k;
            pose.nod = -0.45 * k;
            pose.guard = 0.5 * k;
            pose.sway = 0.0;
            pose.wave = 0.01 * (1.0 - k);
            offset.x -= toward * 30.0 * k;
            tint = tint.mix(&Color::srgb(0.42, 0.42, 0.42), 0.55 * k);
            if s.vanish {
                // the time since the blow, from how much is left
                let t = k * DYING;
                pose.fade = 1.0 - ((t - gore::SWAP_AT) / gore::SWAP_FOR).clamp(0.0, 1.0);
            }
        }
        // a transform stretches about the middle: lift by as much as the stretch lowers the feet,
        // so they stay on the ground through every breath
        offset.y -= (scale.y - 1.0) * s.height / 2.0;
        tf.translation = Val2::px(offset.x, offset.y);
        tf.scale = scale;
        pose.tint = tint;
        for (skin, part) in skins.iter_many(children).flatten() {
            rig::pose(&mut rigs, skin, part, s.t, pose);
        }
    }
}

/// A unit killed: its parts struck off and its body toppled (see `gore`); the body is no longer
/// a unit (it cannot be aimed at, and its bars go) but lies where it fell until the fight ends.
#[allow(clippy::too_many_arguments)]
fn dying(
    mut commands: Commands,
    time: Res<Time>,
    session: Res<Session>,
    comfort: Res<Comfort>,
    images: Res<Assets<Image>>,
    rigs: Res<Assets<rig::RigMaterial>>,
    float_layer: Single<Entity, With<FloatLayer>>,
    layers: (Single<Entity, With<gore::CorpseLayer>>, Single<Entity, With<gore::StainLayer>>),
    blood_art: Res<gore::BloodArt>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut q: Query<(Entity, &mut Dying, &Children, Option<&UnitWidget>)>,
    mut sprites: Query<(&mut UnitSprite, &Children)>,
    skins: Query<(Entity, &rig::RigSkin, &MaterialNode<rig::RigMaterial>)>,
    mut others: Query<&mut Visibility, Without<rig::RigSkin>>,
    blows: Res<gore::Blows>,
) {
    for (entity, mut d, children, widget) in &mut q {
        if let Some(w) = widget {
            let (unit, origin) = (w.unit, w.pos);
            commands.entity(entity).remove::<(UnitWidget, Hovered)>().insert(Pickable::IGNORE);
            let mut parts = Vec::new();
            let mut draw = ENEMY_SIZE;
            let mut stem = String::new();
            for c in children.iter() {
                match sprites.get(c) {
                    Ok((s, skin_children)) => {
                        draw = s.draw;
                        // a foe struck down mid-pose falls as itself
                        stem = pose_base(&s.stem).to_string();
                        let cuts = rig::cuts_of(&stem);
                        for (skin, part, material) in skins.iter_many(skin_children).flatten() {
                            let (Some(part), Some(m)) = (part.part, rigs.get(&material.0)) else { continue };
                            if let Some(joint) = cuts.iter().find(|(p, ..)| *p == part).map(|(_, j, _)| *j) {
                                parts.push((part, joint, m.picture.clone(), skin));
                            }
                        }
                    }
                    // the bars, the name and the shadow go with the life
                    Err(_) => {
                        if let Ok(mut v) = others.get_mut(c) {
                            *v = Visibility::Hidden;
                        }
                    }
                }
            }
            if let Some((foot, _)) = unit_place(&session.shown, unit) {
                let blood = gore::blood_of(&comfort, &session.shown, unit);
                // drawn from what every player's game knows alike (who died, by how hard a
                // blow), so the same parts fly the same way on every screen
                let (weight, lost) = blows.0.get(&unit).copied().unwrap_or((0.0, 0));
                let seed = match unit {
                    UnitRef::Enemy(u) => u as u32,
                    UnitRef::Hero(i) => 1000 + i as u32,
                }
                .wrapping_mul(7919)
                .wrapping_add(lost * 104_729);
                let headless = art.image(&assets, &format!("{stem}_dead_headless"));
                // who had no head to lose falls as they are, and nothing flies off
                let no_head = matches!(unit, UnitRef::Hero(i) if session.shown.hero(i).is_some_and(|h| h.walks_headless));
                let lying = if no_head { headless.clone() } else { art.image(&assets, &format!("{stem}_dead")) };
                if no_head {
                    parts.retain(|p: &(rig::Part, Vec2, Handle<Image>, Entity)| p.0 != rig::Part::Head);
                }
                let swaps = gore::strike_down(&mut commands, entity, unit, origin, foot, draw, matches!(unit, UnitRef::Hero(_)), blood, parts, &images, *float_layer, *layers.0, *layers.1, &blood_art, lying, headless, art.image(&assets, &stem), seed, weight);
                for c in children.iter() {
                    if let Ok((mut s, _)) = sprites.get_mut(c) {
                        s.vanish = swaps;
                    }
                }
            }
        }
        d.0 -= time.delta_secs();
        // the rig lets the body give (see `animate_sprites`)
        for c in children.iter() {
            if let Ok((mut s, _)) = sprites.get_mut(c) {
                s.alive = (d.0 / DYING).clamp(0.0, 1.0);
            }
        }
        if d.0 <= 0.0 {
            commands.entity(entity).remove::<Dying>();
        }
    }
}

fn update_bars(
    session: Res<Session>,
    time: Res<Time>,
    mut fills: Query<(&HpFill, &mut Node), Without<HpTrail>>,
    mut trails: Query<(&mut HpTrail, &mut Node), Without<HpFill>>,
    mut frames: Query<(&HpBar, &mut BorderColor)>,
    mut badges: Query<(&BlockBadge, &mut Visibility)>,
) {
    let stats = |u: UnitRef| match u {
        UnitRef::Hero(i) => session.shown.hero(i).map(|h| (h.hp, h.max_hp, h.block)),
        UnitRef::Enemy(i) => session.shown.enemy(i).map(|e| (e.hp, e.max_hp, e.block)),
    };
    for (fill, mut node) in &mut fills {
        if let Some((hp, max, _)) = stats(fill.0) {
            let w = percent(100.0 * hp.max(0) as f32 / max.max(1) as f32);
            if node.width != w {
                node.width = w;
            }
        }
    }
    let dt = time.delta_secs();
    for (mut trail, mut node) in &mut trails {
        let Some((hp, max, _)) = stats(trail.unit) else { continue };
        let now = hp.max(0) as f32 / max.max(1) as f32;
        if now < trail.last - 1e-4 {
            // a fresh blow: the strip stands a moment before it drains
            trail.hold = 0.45;
        }
        trail.last = now;
        if now >= trail.at {
            trail.at = now;
        } else if trail.hold > 0.0 {
            trail.hold -= dt;
        } else {
            trail.at = (trail.at - dt * (0.15 + (trail.at - now) * 2.5)).max(now);
        }
        let w = percent(100.0 * trail.at);
        if node.width != w {
            node.width = w;
        }
    }
    for (bar, mut edge) in &mut frames {
        let want = if stats(bar.0).is_some_and(|s| s.2 > 0) { BorderColor::all(theme::FROST) } else { BorderColor::all(Color::srgb(0.32, 0.26, 0.20)) };
        if *edge != want {
            *edge = want;
        }
    }
    for (badge, mut vis) in &mut badges {
        let want = if stats(badge.0).is_some_and(|s| s.2 > 0) { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

// ---- text ----


/// Shows each enemy's intent as its picture (`assets/intents/<kind>`), hidden while it has none.
/// Under the Sirin's song every intent is the unknown one.
fn update_intent_icons(
    session: Res<Session>,
    db: Res<Db>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut pictures: Local<std::collections::HashMap<String, Option<Handle<Image>>>>,
    mut icons: Query<(&IntentIcon, &mut ImageNode, &mut Visibility), Without<IntentBack>>,
    mut plates: Query<(&IntentBack, &mut Visibility), Without<IntentIcon>>,
) {
    let shown = &session.shown;
    let sung = session.my_hero().flags.contains_key("no_intents");
    // with no eyes there is no telling what a foe means to do
    if sees_nothing(&session) {
        for (_, _, mut vis) in &mut icons {
            vis.set_if_neq(Visibility::Hidden);
        }
        for (_, mut vis) in &mut plates {
            vis.set_if_neq(Visibility::Hidden);
        }
        return;
    }
    for (icon, mut image, mut vis) in &mut icons {
        let mv = shown
            .enemy(icon.0)
            .and_then(|e| Some((e, e.intent.as_ref()?)))
            .and_then(|(e, i)| db.0.enemies.get(&e.def)?.moves.get(i.mv));
        // every move wears its own emblem; one kept secret, or with none, only its kind's
        let picture = mv.and_then(|mv| {
            let kind = if sung { IntentKind::Unknown } else { mv.intent };
            let mut look = |stem: String| pictures.entry(stem.clone()).or_insert_with(|| art.image(&assets, &stem)).clone();
            let own = (kind != IntentKind::Unknown).then(|| look(format!("intents/m_{}", mv.id))).flatten();
            own.or_else(|| look(format!("intents/{kind:?}")))
        });
        match picture {
            Some(h) => {
                if image.image != h {
                    image.image = h;
                }
                vis.set_if_neq(Visibility::Inherited);
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
    for (plate, mut vis) in &mut plates {
        let on = icons.iter().any(|(icon, _, v)| icon.0 == plate.0 && *v != Visibility::Hidden);
        vis.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
    }
}

#[allow(clippy::too_many_arguments)]
fn update_labels(session: Res<Session>, net: Option<Res<crate::net::Net>>, db: Res<Db>, l10n: Res<L10n>, tip: Res<TooltipText>, clock: Res<TimerText>, mut labels: Query<(&Label, &mut Text)>) {
    let (shown, truth) = (&session.shown, &session.truth);
    for (label, mut text) in &mut labels {
        let s = match *label {
            Label::Hp(UnitRef::Hero(i)) => shown.hero(i).map(|h| format!("{}/{}", h.hp, h.max_hp)),
            Label::Hp(UnitRef::Enemy(i)) => shown.enemy(i).map(|e| format!("{}/{}", e.hp, e.max_hp)),
            Label::Block(UnitRef::Hero(i)) => shown.hero(i).map(|h| h.block.to_string()),
            Label::Block(UnitRef::Enemy(i)) => shown.enemy(i).map(|e| e.block.to_string()),
            Label::Intent(uid) => shown.enemy(uid).map(|e| {
                let Some(intent) = &e.intent else { return String::new() };
                // the Sirin's song: nobody can tell what is coming
                if session.my_hero().flags.contains_key("no_intents") {
                    return String::new();
                }
                // the icon says what kind of move it is; the text only adds how hard and at whom
                let mut s = match bot::intent_damage(&db.0, shown, e) {
                    Some((d, 1)) => d.to_string(),
                    Some((d, k)) => format!("{d}×{k}"),
                    None => String::new(),
                };
                // in a party the target matters; alone it is always you
                if let (Some(t), true) = (intent.target, shown.is_coop()) {
                    if let Some(h) = shown.hero(t) {
                        s.push_str(&format!(" → {}", l10n.t(&format!("hero-{}", h.def))));
                    }
                }
                s
            }),
            Label::Team(i) => shown.hero(i).zip(truth.hero(i)).map(|(h, real)| {
                let mut s = format!("{}  {}/{}", l10n.t(&format!("hero-{}", h.def)), h.hp, h.max_hp);
                if h.beheaded {
                    s.push_str(&format!("  ·  {}", l10n.t("battle-beheaded")));
                } else if h.downed {
                    s.push_str(&format!("  ·  {}", l10n.t("battle-downed")));
                } else if h.walks_headless {
                    s.push_str(&format!("  ·  {}  ·  {} {}", l10n.t("battle-walks-headless"), l10n.t("battle-energy"), real.energy));
                    if real.ready {
                        s.push_str(&format!("  ·  {}", l10n.t("battle-ready")));
                    }
                } else {
                    s.push_str(&format!("  ·  {} {}", l10n.t("battle-energy"), real.energy));
                    if real.ready {
                        s.push_str(&format!("  ·  {}", l10n.t("battle-ready")));
                    }
                }
                s
            }),
            Label::Energy => Some(format!("{}", session.my_hero().energy)),
            // the piles' counts, on the discs of their stacks
            Label::Draw => Some(session.my_hero().draw.len().to_string()),
            Label::Discard => Some(session.my_hero().discard.len().to_string()),
            Label::Round => Some(l10n.n("battle-round", shown.round as i32)),
            // what a thief took counts at once, though the run settles it only when the fight ends
            Label::Gold => net
                .as_ref()
                .and_then(|n| n.node.state.as_ref().zip(n.node.my_slot()))
                .and_then(|(run, me)| run.players.get(me as usize))
                .map(|p| (p.gold - session.my_hero().gold_lost).max(0).to_string()),
            // whoever has lost their head only watches, and the seal says why it does nothing
            Label::Ready if session.my_hero().beheaded => Some(l10n.t("battle-beheaded")),
            Label::Ready => Some(if session.my_hero().ready {
                if truth.is_coop() {
                    l10n.t("battle-waiting")
                } else {
                    l10n.t("battle-unready")
                }
            } else {
                l10n.t("battle-ready")
            }),
            Label::Error => Some(session.error.as_ref().map(|e| e.0.clone()).unwrap_or_default()),
            Label::Tooltip => Some(tip.0.clone()),
            Label::Timer => Some(clock.0.clone()),
        };
        if let Some(s) = s {
            if text.0 != s {
                text.0 = s;
            }
        }
    }
}

// ---- hand ----

pub fn card_frame(def: &CardDef) -> Color {
    match (def.kind, def.rarity) {
        (CardType::Curse, _) => Color::srgb(0.16, 0.09, 0.17),
        (CardType::Status, _) => Color::srgb(0.17, 0.20, 0.18),
        (_, Rarity::Uncommon) => Color::srgb(0.30, 0.31, 0.34),
        (_, Rarity::Rare) => Color::srgb(0.46, 0.33, 0.12),
        _ => Color::srgb(0.24, 0.20, 0.15),
    }
}

/// The ink of a card's rarity, for the ones that have one worth telling.
fn rarity_ink(def: &CardDef) -> Option<Color> {
    if matches!(def.kind, CardType::Curse | CardType::Status) {
        return None;
    }
    match def.rarity {
        Rarity::Common => Some(Color::srgb(0.66, 0.48, 0.30)),
        Rarity::Uncommon => Some(Color::srgb(0.72, 0.78, 0.86)),
        Rarity::Rare => Some(Color::srgb(0.98, 0.78, 0.30)),
        _ => None,
    }
}

/// Spawns a card face under `parent`: free-floating at `pos`, or in the parent's flow when `pos` is `None`.
/// `extra` carries whatever makes the card behave (drag in the hand, a button in a reward).
/// The cost of a card made free, and its glow.
const FREE_INK: Color = Color::srgb(0.52, 0.95, 0.42);

pub fn spawn_card<B: Bundle>(
    commands: &mut Commands,
    parent: Entity,
    card: &Card,
    form: Form,
    pos: Option<Vec2>,
    hidden: bool,
    extra: B,
    db: &Content,
    l10n: &L10n,
    fonts: &Fonts,
    art: &Art,
    assets: &AssetServer,
) -> Option<Entity> {
    let def = db.cards.get(&card.def)?;
    let edge = if def.hero.is_empty() { theme::DIM } else { theme::hero_color(&def.hero) };
    let id = commands
        .spawn((
            extra,
            Hovered::default(),
            Node {
                position_type: if pos.is_some() { PositionType::Absolute } else { PositionType::Relative },
                left: pos.map(|p| px(p.x)).unwrap_or(Val::Auto),
                top: pos.map(|p| px(p.y)).unwrap_or(Val::Auto),
                flex_shrink: 0.0,
                width: px(CARD_W),
                height: px(CARD_H),
                border: UiRect::all(px(4)),
                border_radius: BorderRadius::all(px(12)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(px(6)),
                ..default()
            },
            BackgroundColor(card_frame(def)),
            BorderColor::all(edge),
            UiTransform::IDENTITY,
            ZIndex(0),
            ChildOf(parent),
        ))
        .with_children(|c| card_face(c, card, def, form, hidden, l10n, fonts, art, assets))
        .id();
    Some(id)
}

/// What is drawn on a card, its frame aside.
#[allow(clippy::too_many_arguments)]
pub fn card_face(c: &mut ChildSpawnerCommands, card: &Card, def: &CardDef, form: Form, hidden: bool, l10n: &L10n, fonts: &Fonts, art: &Art, assets: &AssetServer) {
    let view = def.view(card.upgraded, form);
    let cost = if hidden || view.has(Keyword::Unplayable) {
        String::new()
    } else if view.x_cost {
        "X".into()
    } else if card.free {
        "0".into()
    } else {
        view.cost.to_string()
    };
    // a card made free this fight wears its new cost in green, glowing, as the Spire shows it
    let (cost_ink, cost_glow) = if card.free && !view.x_cost { (FREE_INK, Some(FREE_INK)) } else { (theme::CANDLE, None) };
    {
        {
            // a blinded hero sees only the backs: which card is which must be remembered
            if hidden {
                c.spawn((theme::text("?", &fonts.title, 120.0, theme::DIM), Node { margin: UiRect::top(px(70)), ..default() }));
                return;
            }
            let name = l10n.card_name(&card.def, card.upgraded);
            // long names shrink to stay on one line: the title face runs up to 0.8 em a letter
            // (its capitals are broad), in a strip clear of the cost badge on the left
            let size = (160.0 / (name.chars().count().max(1) as f32 * 0.8)).clamp(8.0, 19.0);
            c.spawn((
                theme::text(name.clone(), &fonts.title, size, if card.upgraded { theme::ROTLIGHT } else { theme::BONE }),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
                Node { width: px(170), height: px(28), margin: UiRect::left(px(28)), align_items: AlignItems::Center, ..default() },
            ));
            // rarity in the picture's frame and the line under it: bronze, silver, gold
            let gem = rarity_ink(def);
            c.spawn((
                Node { width: px(196), height: px(147), border: UiRect::all(px(if gem.is_some() { 3 } else { 2 })), ..default() },
                art.node(assets, &format!("cards/{}", card.def), Color::srgb(0.10, 0.09, 0.08)),
                BorderColor::all(gem.unwrap_or(theme::SOOT)),
                Pickable::IGNORE,
            ));
            let kind = l10n.t(&format!("type-{:?}", view.kind));
            let line = match gem {
                Some(_) => format!("{kind} · {}", l10n.t(&format!("rarity-{:?}", def.rarity))),
                None => kind,
            };
            c.spawn((theme::text(line, &fonts.body, 14.0, gem.unwrap_or(theme::DIM)), Node { margin: UiRect::vertical(px(2)), ..default() }));
            // a long text shrinks to stay on the card: about 110 letters fit at full size, and
            // what this guess misses is measured and brought in by `fit_text`
            let mut text = l10n.card_text(&card.def, def, &view);
            // a werewolf card tells its other side too, so the turn holds no surprise
            if def.hero == "volkolak" && def.two_sided(card.upgraded) {
                let other = if form == Form::Beast { Form::Human } else { Form::Beast };
                let side = l10n.card_text(&card.def, def, &def.view(card.upgraded, other));
                text = format!("{text}
{}: {side}", l10n.t(&format!("side-{other:?}")));
            }
            let size = (18.0 * (110.0 / text.chars().count().max(1) as f32).sqrt()).clamp(12.0, 18.0);
            c.spawn((
                theme::text(text, &fonts.body, size, theme::BONE),
                TextLayout::justify(Justify::Center),
                Node { width: px(196), ..default() },
                fit_text::FitText,
            ));
            if !cost.is_empty() {
                c.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(-16),
                        top: px(-16),
                        width: px(48),
                        height: px(48),
                        border: UiRect::all(px(3)),
                        border_radius: BorderRadius::MAX,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(theme::SOOT),
                    BorderColor::all(cost_ink),
                    cost_glow.map(|g| BoxShadow::new(g.with_alpha(0.85), px(0), px(0), px(4), px(14))).unwrap_or_default(),
                    Pickable::IGNORE,
                    children![(theme::text(cost, &fonts.num, 28.0, cost_ink), TextShadow { offset: Vec2::ZERO, color: cost_glow.unwrap_or(Color::NONE) })],
                ));
            }
        }
    }
}

fn sync_hand(
    mut commands: Commands,
    session: Res<Session>,
    db: Res<Db>,
    layer: Single<Entity, With<HandLayer>>,
    cards: Query<(Entity, &CardWidget)>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    let h = session.my_hero();
    // with no head the hand is felt, not read
    let blind = h.flags.contains_key("blind") || sees_nothing(&session);
    let mut reprint = Vec::new();
    let mut leaving = 0;
    for (entity, w) in &cards {
        let held = h.hand.iter().find(|c| c.uid == w.uid);
        if held.is_none() && w.form == h.form {
            // a card gone from the hand is seen going where it went, the hand one after another
            let from = w.shown + Vec2::new(CARD_W, CARD_H) / 2.0;
            let (to, end) = piles::whither(h, w.uid, w.power, from);
            commands.entity(entity).remove::<CardWidget>();
            piles::send_off(&mut commands, entity, from, to, end, leaving as f32 * 0.06);
            leaving += 1;
        } else if held.is_none() || w.form != h.form || w.hidden != blind {
            // a form change flips every card, so the widget is rebuilt
            commands.entity(entity).despawn();
        } else if held.is_some_and(|c| (c.upgraded, c.free) != w.look) {
            commands.entity(entity).despawn();
            reprint.push((w.uid, w.pos));
        }
    }
    let mut drawn = 0;
    for card in &h.hand {
        let current = |w: &CardWidget| w.uid == card.uid && w.form == h.form && w.hidden == blind && w.look == (card.upgraded, card.free);
        if !cards.iter().any(|(_, w)| current(w)) {
            // new cards come in from the draw pile one by one; a reprinted one stays where it was
            let again = reprint.iter().find(|(uid, _)| *uid == card.uid).map(|(_, p)| *p);
            let from = again.unwrap_or(Vec2::new(30.0, 860.0));
            let wait = if again.is_some() { 0.0 } else { drawn as f32 * 0.11 };
            if again.is_none() {
                drawn += 1;
            }
            let widget = (
                CardWidget { uid: card.uid, form: h.form, hidden: blind, pos: from, drag: Vec2::ZERO, dragging: false, aiming: false, look: (card.upgraded, card.free), wait, shown: from, power: db.0.cards.get(&card.def).is_some_and(|d| d.kind == CardType::Power) },
                // let the pointer through, so a dragged card can be dropped on the unit under it
                Pickable { should_block_lower: false, is_hoverable: true },
            );
            spawn_card(&mut commands, *layer, card, h.form, Some(from), blind, widget, &db.0, &l10n, &fonts, &art, &assets);
        }
    }
}

/// A card of the hand whose bonus would come off if played now: ringed in candlelight.
#[derive(Component)]
struct BonusLit;

/// Rings the cards whose condition is met, and takes the ring off once it is not. The rules
/// decide, on a copy of the fight; worked out only when the fight has changed.
fn light_bonuses(mut commands: Commands, session: Res<Session>, db: Res<Db>, cards: Query<(Entity, &CardWidget, Has<BonusLit>)>, mut last: Local<Option<(u64, usize)>>) {
    let h = session.my_hero();
    // the hand and the log of plays are what a condition reads
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        session.truth.hash(&mut hasher);
        (hasher.finish(), cards.iter().count())
    };
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    let blind = sees_nothing(&session);
    for (e, w, lit) in &cards {
        let Some(card) = h.hand.iter().find(|c| c.uid == w.uid) else { continue };
        let ready = !blind
            && db.0.cards.get(&card.def).is_some_and(|def| nav_core::combat::bonus_ready(&db.0, &session.truth, session.me, &def.view(card.upgraded, h.form).effects));
        if ready && !lit {
            commands.entity(e).insert((BonusLit, Outline::new(px(4), px(1), theme::CANDLE)));
        } else if !ready && lit {
            commands.entity(e).remove::<(BonusLit, Outline)>();
        }
    }
}

/// An empty place on the belt.
#[derive(Component)]
struct EmptyFlask;

/// Keeps the potion belt in step with the hero's potions.
fn sync_potions(
    mut commands: Commands,
    session: Res<Session>,
    db: Res<Db>,
    belt: Single<Entity, With<PotionBelt>>,
    chips: Query<(Entity, &PotionChip)>,
    empty: Query<Entity, With<EmptyFlask>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    let potions = &session.my_hero().potions;
    let same = chips.iter().count() == potions.len() && chips.iter().all(|(_, c)| potions.get(c.slot as usize) == Some(&c.id));
    if same && !empty.is_empty() == (potions.len() < POTION_SLOTS) {
        return;
    }
    for (e, _) in &chips {
        commands.entity(e).despawn();
    }
    for e in &empty {
        commands.entity(e).despawn();
    }
    for (i, id) in potions.iter().enumerate() {
        let needs_target = db.0.potions.get(id).is_some_and(|d| matches!(d.target, CardTarget::Enemy | CardTarget::Downed));
        let slot = i as u8;
        let mut e = commands.spawn((
            PotionChip { slot, id: id.clone() },
            Button,
            Hovered::default(),
            Btn(theme::SOOT),
            Pickable { should_block_lower: false, is_hoverable: true },
            // a flask: its picture when painted (potions/<id>), else a disc; what it does on hover
            crate::run_ui::CardHints::new(vec![(l10n.t(&format!("potion-{id}")), l10n.t(&format!("potion-{id}-desc")))]),
            Node { width: px(50), height: px(50), padding: UiRect::all(px(3)), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, ..default() },
            BackgroundColor(theme::SOOT),
            BorderColor::all(theme::ROTLIGHT),
            BoxShadow::new(Color::BLACK.with_alpha(0.6), px(0), px(3), px(0), px(6)),
            UiTransform::IDENTITY,
            GlobalZIndex(40),
            ChildOf(*belt),
        ));
        e.with_children(|f| {
            match art.image(&assets, &format!("potions/{id}")) {
                Some(h) => f.spawn((Node { width: percent(100), height: percent(100), ..default() }, ImageNode::new(h), Pickable::IGNORE)),
                // until it has a picture, the first letters of its name tell one from another
                None => f.spawn((
                    Node { width: percent(100), height: percent(100), border_radius: BorderRadius::MAX, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
                    BackgroundColor(theme::ROTLIGHT.with_alpha(0.6)),
                    Pickable::IGNORE,
                    children![theme::text(l10n.t(&format!("potion-{id}")).chars().take(2).collect::<String>(), &fonts.title, 24.0, theme::SOOT)],
                )),
            };
        });
        // untargeted and ally draughts are drunk on a click; the rest must be thrown at something
        if !needs_target {
            e.observe(move |_: On<Activate>, mut outbox: ResMut<Outbox>| outbox.0.push(CombatAction::UsePotion { slot, target: None }));
        }
        // dragged, every potion is aimed with the arrow from its place on the belt, as cards are
        e.observe(move |ev: On<PointerDrag>, mut commands: Commands| {
            commands.entity(ev.event_target()).insert(aim::Thrown);
        });
        // one that lands on nobody in particular is drunk once the arrow has been drawn far enough
        e.observe(move |ev: On<PointerDragEnd>, mut commands: Commands, scale: Res<UiScale>, session: Option<Res<Session>>, db: Res<Db>, mut outbox: ResMut<Outbox>, chips: Query<&PotionChip>| {
            commands.entity(ev.event_target()).remove::<aim::Thrown>();
            let (Some(session), Ok(chip)) = (session, chips.get(ev.event_target())) else { return };
            let alone = session.truth.standing_heroes().count() == 1;
            let aimed = db.0.potions.get(&chip.id).is_some_and(|d| aim::needs_unit(d.target, alone));
            if !aimed && ev.distance.length() / scale.0 > aim::POTION_PULL {
                outbox.0.push(CombatAction::UsePotion { slot: chip.slot, target: None });
            }
        });
    }
    for _ in potions.len()..POTION_SLOTS {
        commands.spawn((
            EmptyFlask,
            Node { width: px(50), height: px(50), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, ..default() },
            BackgroundColor(theme::SOOT),
            BorderColor::all(theme::DIM.with_alpha(0.4)),
            Pickable::IGNORE,
            ChildOf(*belt),
        ));
    }
}

/// Where the mouse is on the stage. Taken from the picking pointer, so it is the same point
/// that hovering and dragging use.
fn stage_pointer(pointers: &Query<(&bevy::picking::pointer::PointerId, &bevy::picking::pointer::PointerLocation)>, window: &Window, scale: f32) -> Option<Vec2> {
    let at = pointers.iter().find(|(id, _)| id.is_mouse()).and_then(|(_, l)| l.location.as_ref()).map(|l| l.position)?;
    let origin = Vec2::new(window.width() - STAGE_W * scale, window.height() - STAGE_H * scale) / 2.0;
    Some((at - origin) / scale)
}

/// When the cursor reaches the hand, the nearest card is
/// the one in focus, and it stays so while the cursor travels up over the risen card.
fn pick_hover(
    session: Res<Session>,
    profile: Res<crate::profile::Profile>,
    cards: Query<&CardWidget>,
    pointers: Query<(&bevy::picking::pointer::PointerId, &bevy::picking::pointer::PointerLocation)>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Res<UiScale>,
    mut top: ResMut<HoverTop>,
    mut reach: ResMut<HandReach>,
    mut reaching: Local<bool>,
) {
    let hand = &session.my_hero().hand;
    let n = hand.len();
    let dragging = cards.iter().find(|w| w.dragging).map(|w| w.uid);
    let cursor = stage_pointer(&pointers, &window, scale.0);
    let span = hand_spacing(n) * (n.max(1) - 1) as f32 / 2.0 + CARD_W * 0.6;
    *reaching = match cursor {
        Some(c) if n > 0 && (c.x - STAGE_W / 2.0).abs() <= span => {
            // reaching in starts at the visible strip; once in, the whole risen card keeps it
            c.y >= hand_top(profile.settings.raised_hand) - 20.0 || (*reaching && c.y >= HAND_RISEN - 15.0)
        }
        _ => false,
    };
    let x = cursor.filter(|_| *reaching && dragging.is_none()).map(|c| c.x);
    if reach.0 != x {
        reach.0 = x;
    }
    let best = dragging.or_else(|| {
        let x = x?;
        (0..n).min_by(|a, b| {
            let d = |i: usize| (hand_slot(i, n, profile.settings.raised_hand).x + CARD_W / 2.0 - x).abs();
            d(*a).total_cmp(&d(*b))
        })
        .map(|i| hand[i].uid)
    });
    if top.0 != best {
        top.0 = best;
    }
}

fn layout_hand(
    session: Res<Session>,
    profile: Res<crate::profile::Profile>,
    db: Res<Db>,
    time: Res<Time>,
    top: Res<HoverTop>,
    reach: Res<HandReach>,
    mut cards: Query<(&mut CardWidget, &mut Node, &mut UiTransform, &mut ZIndex, &mut BackgroundColor)>,
) {
    let h = session.my_hero();
    let n = h.hand.len();
    let k = (time.delta_secs() * 14.0).min(1.0);
    for (mut w, mut node, mut tf, mut z, mut bg) in &mut cards {
        let Some(i) = h.hand.iter().position(|c| c.uid == w.uid) else { continue };
        if w.wait > 0.0 {
            // still on the draw pile, small, its turn to come not yet here
            w.wait -= time.delta_secs();
            tf.scale = Vec2::splat(0.2);
            node.left = px(w.pos.x);
            node.top = px(w.pos.y);
            continue;
        }
        let rest = hand_slot(i, n, profile.settings.raised_hand);
        let mut target = rest;
        let lifted = top.0 == Some(w.uid);
        // the nearer the cursor, the higher the card: full height under it, falling away
        // smoothly over the next two or three cards to either side
        let centre = rest.x + CARD_W / 2.0;
        let pull = if w.dragging {
            1.0
        } else {
            reach.0.map_or(0.0, |x| {
                let d = (x - centre).abs() / (hand_spacing(n) * 2.6);
                if d < 1.0 { (1.0 + (d * std::f32::consts::PI).cos()) / 2.0 } else { 0.0 }
            })
        };
        target.y += (HAND_RISEN - rest.y) * pull;
        // neighbours lean away a little, so the card in focus is not half covered
        if let (Some(x), false) = (reach.0, lifted) {
            target.x += (centre - x).signum() * 46.0 * (pull * (1.0 - pull) * 4.0).min(1.0);
        }
        if w.aiming {
            target = aim::AIM_SPOT;
        }
        w.pos = w.pos.lerp(if w.aiming { target - w.drag } else { target }, k);
        let p = w.pos + w.drag;
        w.shown = p;
        node.left = px(p.x);
        node.top = px(p.y);
        // how far it has actually risen drives size and tilt, so they move with it
        let risen = ((rest.y - w.pos.y) / (rest.y - HAND_RISEN)).clamp(0.0, 1.0);
        let scale = if w.aiming { 1.08 } else if w.dragging { 0.86 } else { 1.0 + 0.10 * risen };
        tf.scale = tf.scale.lerp(Vec2::splat(scale), k);
        let mid = (n as f32 - 1.0) / 2.0;
        tf.rotation = Rot2::radians((i as f32 - mid) * 0.035 * (1.0 - risen) * if w.dragging { 0.0 } else { 1.0 });
        let want_z = if lifted { 100 } else { i as i32 };
        if z.0 != want_z {
            z.0 = want_z;
        }
        // cards you can't afford sink into the dark
        let card = &h.hand[i];
        if let Some(def) = db.0.cards.get(&card.def) {
            let view = def.view(card.upgraded, h.form);
            let cost = if view.x_cost { 0 } else { view.cost };
            // held in the mire or asleep, coiled (no attacks), or this card frozen by Mara
            let held = h.flags.contains_key("mired") || h.flags.contains_key("asleep");
            let coiled = view.kind == CardType::Attack && h.flags.contains_key("coiled");
            let frozen = (i as i32) < h.flags.get("frozen").copied().unwrap_or(0);
            let no_arm = nav_core::combat::needs_lost_arm(&view, h.lost_arms, h.def.as_str(), h.form);
            let ok = !view.has(Keyword::Unplayable) && cost <= h.energy && !h.downed && !held && !coiled && !frozen && !no_arm;
            let frame = card_frame(def);
            let want = if sees_nothing(&session) { Color::BLACK } else if frozen { frame.mix(&theme::FROST, 0.55) } else if ok { frame } else { frame.mix(&theme::SOOT, 0.45) };
            if bg.0 != want {
                bg.0 = want;
            }
        }
    }
}

/// While a discard is being picked for, the card clicked in the hand is thrown.
fn on_card_click(ev: On<PointerClick>, top: Res<HoverTop>, cards: Query<&CardWidget>, session: Option<Res<Session>>, mut outbox: ResMut<Outbox>) {
    let Some(session) = session else { return };
    if session.my_hero().choosing.is_none() {
        return;
    }
    let Ok(w) = cards.get(ev.event_target()) else { return };
    if top.0.is_some_and(|t| t != w.uid) {
        return;
    }
    outbox.0.push(CombatAction::Discard { card: w.uid });
}

fn on_drag_start(ev: On<PointerDragStart>, top: Res<HoverTop>, mut cards: Query<&mut CardWidget>, session: Option<Res<Session>>) {
    // one card in the hand at a time: the one on top under the cursor. Overlapping cards each
    // hear the press at their edges, and both used to come away together
    if cards.iter().any(|w| w.dragging) {
        return;
    }
    let target = ev.event_target();
    let Ok(uid) = cards.get(target).map(|w| w.uid) else { return };
    if top.0.is_some_and(|t| t != uid) {
        return;
    }
    // a discard is being picked for: cards are clicked to be thrown, not dragged to be played
    if session.as_ref().is_some_and(|s| s.my_hero().choosing.is_some()) {
        return;
    }
    if let Ok(mut w) = cards.get_mut(target) {
        w.dragging = true;
    }
}

fn on_drag(ev: On<PointerDrag>, scale: Res<UiScale>, mut cards: Query<&mut CardWidget>) {
    if let Ok(mut w) = cards.get_mut(ev.event_target()) {
        if w.dragging {
            w.drag = ev.distance / scale.0;
        }
    }
}

fn on_drag_end(
    ev: On<PointerDragEnd>,
    scale: Res<UiScale>,
    mut cards: Query<&mut CardWidget>,
    session: Option<Res<Session>>,
    db: Res<Db>,
    mut outbox: ResMut<Outbox>,
) {
    let Ok(mut w) = cards.get_mut(ev.event_target()) else { return };
    // a card that never took the drag (another card had it) plays nothing
    if !w.dragging {
        return;
    }
    let lifted = ev.distance.y / scale.0 < -aim::LIFT_TO_AIM;
    w.dragging = false;
    w.aiming = false;
    // keep the card where it was dropped; the hand layout eases it home or it leaves the hand
    let dropped = w.drag;
    w.pos += dropped;
    w.drag = Vec2::ZERO;
    let Some(session) = session else { return };
    let h = session.my_hero();
    let Some(card) = h.hand.iter().find(|c| c.uid == w.uid) else { return };
    let Some(def) = db.0.cards.get(&card.def) else { return };
    let view = def.view(card.upgraded, h.form);
    // untargeted cards are played by flicking them up onto the field
    let alone = session.truth.standing_heroes().count() == 1;
    let untargeted = view.target == CardTarget::None || (view.target == CardTarget::Ally && alone);
    if lifted && untargeted {
        outbox.0.push(CombatAction::Play { card: w.uid, target: None });
    }
}

fn on_drop(
    ev: On<PointerDragDrop>,
    units: Query<&UnitWidget>,
    potions: Query<&PotionChip>,
    cards: Query<&CardWidget>,
    session: Option<Res<Session>>,
    db: Res<Db>,
    mut outbox: ResMut<Outbox>,
) {
    // a potion thrown at a unit; one that needs nobody is drunk when its drag ends instead
    if let (Ok(unit), Ok(chip)) = (units.get(ev.event_target()), potions.get(ev.dropped)) {
        let alone = session.as_ref().is_some_and(|s| s.truth.standing_heroes().count() == 1);
        if db.0.potions.get(&chip.id).is_some_and(|d| aim::needs_unit(d.target, alone)) {
            outbox.0.push(CombatAction::UsePotion { slot: chip.slot, target: Some(unit.unit) });
        }
        return;
    }
    let (Ok(unit), Ok(w), Some(session)) = (units.get(ev.event_target()), cards.get(ev.dropped), session) else { return };
    if !w.dragging {
        return;
    }
    let h = session.my_hero();
    let Some(card) = h.hand.iter().find(|c| c.uid == w.uid) else { return };
    let Some(def) = db.0.cards.get(&card.def) else { return };
    let view = def.view(card.upgraded, h.form);
    let downed = matches!(unit.unit, UnitRef::Hero(i) if session.truth.hero(i).is_some_and(|a| a.downed));
    if downed && view.target != CardTarget::Downed {
        // any card on a fallen ally is the Raise action, paid with that card
        if let UnitRef::Hero(ally) = unit.unit {
            outbox.0.push(CombatAction::Raise { ally, discard: w.uid });
        }
    } else if let (true, UnitRef::Hero(to)) = (view.has(Keyword::Passable), unit.unit) {
        // the needle goes from hand to hand
        if to != session.me {
            outbox.0.push(CombatAction::Pass { card: w.uid, to });
        }
    } else if view.target != CardTarget::None {
        outbox.0.push(CombatAction::Play { card: w.uid, target: Some(unit.unit) });
    }
}

// ---- tooltips ----

#[derive(Resource, Default)]
pub struct TooltipText(String);

fn update_tooltip(
    session: Res<Session>,
    db: Res<Db>,
    l10n: Res<L10n>,
    top: Res<HoverTop>,
    cards: Query<&CardWidget>,
    units: Query<(&UnitWidget, &Hovered)>,
    relics: Query<(&RelicChip, &Hovered)>,
    mut tip: ResMut<TooltipText>,
    mut panel: Single<(&mut Node, &mut Visibility), With<Tooltip>>,
) {
    let mut lines: Vec<(String, String)> = Vec::new();
    let mut at = Vec2::ZERO;
    let h = session.my_hero();
    if let Some(w) = top.0.and_then(|uid| cards.iter().find(|w| w.uid == uid && !w.dragging && !w.hidden)) {
        if let Some((card, def)) = h.hand.iter().find(|c| c.uid == w.uid).and_then(|c| db.0.cards.get(&c.def).map(|d| (c, d))) {
            lines = l10n.card_glossary(&def.view(card.upgraded, h.form));
            at = Vec2::new(w.pos.x + CARD_W * 1.12 + 14.0, HAND_RISEN + 5.0);
            if at.x + 340.0 > STAGE_W {
                at.x = w.pos.x - 350.0;
            }
        }
    } else if let Some((chip, _)) = relics.iter().find(|(_, hov)| hov.get()) {
        lines.push((l10n.t(&format!("relic-{}", chip.id)), l10n.t(&format!("relic-{}-desc", chip.id))));
        at = chip.at + Vec2::new(0.0, 50.0);
    } else if let Some((w, _)) = units.iter().find(|(_, hov)| hov.get()) {
        if let UnitRef::Enemy(uid) = w.unit {
            if let Some(e) = session.shown.enemy(uid) {
                let sung = session.my_hero().flags.contains_key("no_intents");
                if let Some(mv) = e.intent.as_ref().filter(|_| !sung).and_then(|i| db.0.enemies.get(&e.def)?.moves.get(i.mv)) {
                    let key = format!("move-{}", mv.id);
                    let desc = format!("{key}-desc");
                    // the kind first: it names the picture over the enemy's head
                    let kind = l10n.t(&format!("intent-{:?}", mv.intent));
                    lines.push((format!("{kind} · {}", l10n.t(&key)), if l10n.has(&desc) { l10n.t(&desc) } else { String::new() }));
                }
            }
        }
        // everything the row under the unit shows, by name, with what it does
        for c in status_row::chips(&l10n, &session, w.unit) {
            let name = match c.n {
                Some(n) => format!("{} {}", c.name, c.shown.unwrap_or_else(|| n.to_string())),
                None => c.name,
            };
            lines.push((name, c.help));
        }
        at = Vec2::new((w.pos.x + UNIT_W + 6.0).min(STAGE_W - 350.0), w.pos.y + 80.0);
    }
    let body = lines.iter().map(|(k, v)| if v.is_empty() { k.clone() } else { format!("{k} — {v}") }).collect::<Vec<_>>().join("\n\n");
    let (node, vis) = &mut *panel;
    let want = if body.is_empty() { Visibility::Hidden } else { Visibility::Inherited };
    if **vis != want {
        **vis = want;
    }
    if !body.is_empty() {
        node.left = px(at.x);
        node.top = px(at.y);
    }
    if tip.0 != body {
        tip.0 = body;
    }
}

// ---- juice ----

fn cue_effects(
    mut commands: Commands,
    cues: Res<Cues>,
    session: Res<Session>,
    layer: Single<Entity, With<FloatLayer>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    mut sprites: Query<&mut UnitSprite>,
    mut impacts: ResMut<impact::Impacts>,
    mut fx: ResMut<fx::Bursts>,
    comfort: Res<Comfort>,
    db: Res<Db>,
    mut blows: ResMut<gore::Blows>,
    mut severed: ResMut<gore::Severed>,
) {
    let float = |commands: &mut Commands, unit: UnitRef, text: String, color: Color, size: f32, dx: f32| {
        let Some((foot, sz)) = unit_place(&session.shown, unit) else { return };
        commands.spawn((
            Floater { ttl: 0.95, life: 0.95 },
            theme::text(text, &fonts.num, size, color),
            TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK.with_alpha(0.95) },
            // a dim plate behind it: a number must read over snow as over night
            BackgroundColor(theme::SOOT.with_alpha(0.62)),
            Node { position_type: PositionType::Absolute, left: px(foot.x - 20.0 + dx), top: px(foot.y - sz.y * 0.72), padding: UiRect::axes(px(8), px(1)), border_radius: BorderRadius::all(px(10)), ..default() },
            ChildOf(*layer),
        ));
    };
    let mut play = |unit: UnitRef, anim: Anim| {
        for mut s in &mut sprites {
            if s.unit == unit && s.anim.is_none_or(|(now, _)| anim.weight() >= now.weight()) {
                s.anim = Some((anim, 0.0));
            }
        }
    };
    // where a unit's body is, for things that fly between units
    let body = |unit: UnitRef| unit_place(&session.shown, unit).map(|(foot, sz)| Vec2::new(foot.x, foot.y - sz.y * 0.5));
    let ghost = |commands: &mut Commands, from: Vec2, to: Vec2, text: String, edge: Color| {
        commands.spawn((
            Ghost { from, to, t: 0.0 },
            Node {
                position_type: PositionType::Absolute,
                left: px(from.x - 70.0),
                top: px(from.y - 45.0),
                width: px(140),
                height: px(90),
                padding: UiRect::all(px(6)),
                border: UiRect::all(px(3)),
                border_radius: BorderRadius::all(px(8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme::PEAT),
            BorderColor::all(edge),
            UiTransform::IDENTITY,
            Pickable::IGNORE,
            children![(theme::text(text, &fonts.title, 15.0, theme::BONE), TextLayout::justify(Justify::Center))],
            ChildOf(*layer),
        ));
    };
    for e in &cues.0 {
        match e {
            Event::Damage { src, dst, blocked, lost, attack, .. } => {
                // the blow comes from the attacker's side: blood and sparks fly the other way
                let dir = if matches!(dst, UnitRef::Enemy(_)) { 1.0 } else { -1.0 };
                if *attack {
                    if let Some(s) = src {
                        play(*s, Anim::Lunge);
                        if let Some((foot, _)) = unit_place(&session.shown, *s) {
                            fx.0.push(fx::Burst::Dust { at: foot, dir });
                        }
                    }
                }
                if *blocked > 0 {
                    impacts.0.push(impact::Impact::Blocked { unit: *dst, blocked: *blocked, through: *lost });
                    if let Some(at) = body(*dst) {
                        fx.0.push(fx::Burst::Sparks { at: at - Vec2::new(dir * 60.0, 0.0), dir, weight: (*blocked as f32 / 20.0).min(1.0) });
                    }
                }
                if *lost > 0 {
                    // the screen shakes with the weight of the blow (see `impact`)
                    let max = match dst {
                        UnitRef::Hero(i) => session.shown.hero(*i).map(|h| h.max_hp),
                        UnitRef::Enemy(u) => session.shown.enemy(*u).map(|e| e.max_hp),
                    };
                    let share = max.map_or(0.0, |m| *lost as f32 / m.max(1) as f32);
                    impacts.0.push(impact::Impact::Hurt { unit: *dst, lost: *lost, share });
                    play(*dst, Anim::Hit);
                    if let Some(at) = body(*dst) {
                        let weight = (share * 3.0).max(*lost as f32 / 30.0).min(1.0);
                        let ground = unit_place(&session.shown, *dst).map_or(at.y + 200.0, |(foot, _)| foot.y);
                        let blood = gore::blood_of(&comfort, &session.shown, *dst);
                        // the last blow a unit took decides what a killing one strikes off
                        blows.0.insert(*dst, (weight, *lost as u32));
                        fx.0.push(fx::Burst::Wound { at: at - Vec2::new(0.0, 40.0), dir, weight, blood, ground });
                    }
                    float(&mut commands, *dst, format!("-{lost}"), comfort.blood(), 46.0 + (*lost as f32).min(30.0), 0.0);
                }
                if *blocked > 0 {
                    float(&mut commands, *dst, format!("-{blocked}"), theme::FROST, 34.0, -70.0);
                }
            }
            Event::Maimed { player, arm, .. } => {
                let unit = UnitRef::Hero(*player);
                severed.0.push((unit, if *arm == nav_core::combat::SHIELD_ARM { rig::Part::Shield } else { rig::Part::Arm }));
                float(&mut commands, unit, l10n.t("battle-maimed"), comfort.blood(), 40.0, 60.0);
            }
            Event::Beheaded { player } => {
                blows.0.insert(UnitRef::Hero(*player), (1.0, 999));
                impacts.0.push(impact::Impact::Quake(0.8));
            }
            Event::Missed { unit } => float(&mut commands, *unit, l10n.t("battle-missed"), theme::BONE, 34.0, -70.0),
            Event::GoldStolen { player, amount } => float(&mut commands, UnitRef::Hero(*player), l10n.n("battle-gold-stolen", *amount), theme::CANDLE, 40.0, -40.0),
            Event::EnemyFled { enemy } => float(&mut commands, UnitRef::Enemy(*enemy), l10n.t("battle-fled"), theme::BONE, 36.0, -40.0),
            Event::GoldScattered { enemy, amount } => float(&mut commands, UnitRef::Enemy(*enemy), l10n.n("battle-gold-scattered", *amount), theme::CANDLE, 36.0, -40.0),
            Event::GoldReturned { player, amount } => float(&mut commands, UnitRef::Hero(*player), l10n.n("battle-gold-returned", *amount), theme::CANDLE, 40.0, -40.0),
            Event::Block { unit, amount, .. } => {
                play(*unit, Anim::Guarded);
                float(&mut commands, *unit, format!("+{amount}"), theme::NAV_BLUE.lighter(0.25), 36.0, -70.0)
            }
            Event::Heal { unit, amount, .. } => {
                play(*unit, Anim::Healed);
                if let Some((foot, _)) = unit_place(&session.shown, *unit) {
                    fx.0.push(fx::Burst::Motes { at: foot, color: Color::srgb(0.6, 1.0, 0.5) });
                }
                float(&mut commands, *unit, format!("+{amount}"), theme::ROTLIGHT, 38.0, 40.0)
            }
            Event::Status { unit, status, delta, .. } if *delta > 0 => {
                play(*unit, if status.debuff() { Anim::Cursed } else { Anim::Buffed });
                if let Some((foot, _)) = unit_place(&session.shown, *unit) {
                    fx.0.push(if status.debuff() { fx::Burst::Smoke { at: foot, color: Color::srgb(0.2, 0.3, 0.12) } } else { fx::Burst::Motes { at: foot, color: Color::srgb(1.0, 0.82, 0.4) } });
                }
                let color = if status.debuff() { theme::ROTLIGHT } else { theme::CANDLE };
                float(&mut commands, *unit, format!("{} +{}", l10n.t(&format!("status-{status:?}")), l10n.status_amount(*status, *delta)), color, 24.0, 30.0)
            }
            Event::EnemySkipped { enemy, .. } => float(&mut commands, UnitRef::Enemy(*enemy), "…".into(), theme::BONE, 44.0, 0.0),
            // a card played is seen in the hand that held it (`held`)
            Event::CardPlayed { player, card, .. } => {
                let who = UnitRef::Hero(*player);
                let def = db.0.cards.get(card);
                let kind = def.map(|d| d.kind);
                // what raises a shield is seen raising it: the block's own guard pose follows
                let shields = def.is_some_and(|d| d.effects.iter().any(|e| matches!(e, nav_core::Effect::Block(..) | nav_core::Effect::Guard(..))));
                if kind != Some(CardType::Attack) && !shields {
                    play(who, Anim::Cast);
                }
            }
            Event::Passed { from, to, card } => {
                play(UnitRef::Hero(*from), Anim::Cast);
                play(UnitRef::Hero(*to), Anim::Buffed);
                if let (Some(a), Some(b)) = (body(UnitRef::Hero(*from)), body(UnitRef::Hero(*to))) {
                    ghost(&mut commands, a, b, l10n.card_name(card, false), theme::BONE);
                }
            }
            Event::PotionUsed { player, potion, target } => {
                let who = UnitRef::Hero(*player);
                play(who, Anim::Cast);
                if let Some(from) = body(who) {
                    let to = target.and_then(|t| body(t)).unwrap_or(Vec2::new(from.x, from.y - 150.0));
                    ghost(&mut commands, from, to, l10n.t(&format!("potion-{potion}")), theme::ROTLIGHT);
                }
            }
            // an enemy's ability is named over its head as it acts
            Event::EnemyMove { enemy, mv } => {
                let unit = UnitRef::Enemy(*enemy);
                let def = session.shown.enemy(*enemy).and_then(|e| db.0.enemies.get(&e.def));
                if let Some(m) = def.and_then(|d| d.moves.get(*mv)) {
                    if !matches!(m.intent, IntentKind::Attack | IntentKind::AttackAll) {
                        play(unit, Anim::Cast);
                    }
                    float(&mut commands, unit, l10n.t(&format!("move-{}", m.id)), theme::BONE, 26.0, -60.0);
                }
            }
            Event::RuneFired { player, rune } => {
                play(UnitRef::Hero(*player), Anim::Buffed);
                float(&mut commands, UnitRef::Hero(*player), l10n.t(&format!("rune-{rune:?}")), theme::FROST, 22.0, 50.0)
            }
            Event::RuneDischarged { player, rune } => {
                play(UnitRef::Hero(*player), Anim::Cast);
                float(&mut commands, UnitRef::Hero(*player), l10n.tf("rune-discharged", &[("r", l10n.t(&format!("rune-{rune:?}")).into())]), theme::CANDLE, 24.0, 90.0)
            }
            Event::HerbSpilled { player, herb } => {
                play(UnitRef::Hero(*player), Anim::Cast);
                float(&mut commands, UnitRef::Hero(*player), l10n.tf("herb-spilled", &[("h", l10n.t(&format!("herb-{herb}")).into())]), theme::ROTLIGHT, 24.0, 90.0)
            }
            Event::RuneCarved { player, .. } | Event::HerbAdded { player, .. } | Event::MaskChanged { player, .. } | Event::Shifted { player, .. } => play(UnitRef::Hero(*player), Anim::Buffed),
            Event::Brewed { player, recipe } => {
                play(UnitRef::Hero(*player), Anim::Cast);
                let key = format!("recipe-{recipe}");
                if l10n.has(&key) {
                    float(&mut commands, UnitRef::Hero(*player), l10n.t(&key), theme::ROTLIGHT, 24.0, 50.0);
                }
            }
            Event::Raised { player, .. } => play(UnitRef::Hero(*player), Anim::Healed),
            Event::Summoned { enemy, .. } => play(UnitRef::Enemy(*enemy), Anim::Buffed),
            Event::Phase2 { enemy } => {
                play(UnitRef::Enemy(*enemy), Anim::Buffed);
                impacts.0.push(impact::Impact::Quake(0.6));
            }
            _ => {}
        }
    }
}

/// Flies each ghost along a shallow arc and lets it fade into its target.
fn fly_ghosts(mut commands: Commands, time: Res<Time>, speed: Res<AnimSpeed>, mut q: Query<(Entity, &mut Ghost, &mut Node, &mut UiTransform, &mut BackgroundColor)>) {
    for (entity, mut g, mut node, mut tf, mut bg) in &mut q {
        g.t += time.delta_secs() * speed.0 / 0.42;
        if g.t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // ease out: quick off the hand, settling on arrival
        let p = 1.0 - (1.0 - g.t).powi(2);
        let at = g.from.lerp(g.to, p) + Vec2::new(0.0, -90.0 * (p * std::f32::consts::PI).sin());
        node.left = px(at.x - 70.0);
        node.top = px(at.y - 45.0);
        tf.scale = Vec2::splat(1.0 - 0.45 * p);
        tf.rotation = Rot2::radians(0.25 * (1.0 - p) * if g.to.x >= g.from.x { 1.0 } else { -1.0 });
        bg.0.set_alpha(1.0 - 0.6 * p * p);
    }
}

fn floaters(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Floater, &mut Node, &mut TextColor, Option<&mut BackgroundColor>)>) {
    for (entity, mut f, mut node, mut color, plate) in &mut q {
        let dt = time.delta_secs();
        f.ttl -= dt;
        if f.ttl <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // pop up fast, then hang and fade
        let p = 1.0 - f.ttl / f.life;
        let speed = 190.0 * (1.0 - p).powi(2) + 14.0;
        if let Val::Px(top) = node.top {
            node.top = px(top - speed * dt);
        }
        let fade = ((1.0 - p) * 2.2).min(1.0);
        color.0.set_alpha(fade);
        if let Some(mut plate) = plate {
            plate.0.set_alpha(0.62 * fade);
        }
    }
}

/// Text of the turn clock, empty when the timer is off.
#[derive(Resource, Default)]
struct TimerText(String);

/// The optional turn timer: when it runs out, this player is marked ready.
fn turn_timer(
    session: Res<Session>,
    net: Option<Res<crate::net::Net>>,
    time: Res<Time>,
    mut clock: ResMut<TimerText>,
    mut outbox: ResMut<Outbox>,
    mut state: Local<(u32, f32)>,
) {
    let limit = net.as_ref().and_then(|n| n.node.lobby.turn_timer);
    let Some(limit) = limit else {
        if !clock.0.is_empty() {
            clock.0.clear();
        }
        return;
    };
    // the clock restarts with every round
    if state.0 != session.truth.round {
        *state = (session.truth.round, 0.0);
    }
    let h = session.my_hero();
    if session.truth.phase != Phase::Heroes || h.ready || h.downed {
        clock.0.clear();
        return;
    }
    if session.idle() {
        state.1 += time.delta_secs();
    }
    let left = (limit as f32 - state.1).ceil().max(0.0) as i32;
    let text = left.to_string();
    if clock.0 != text {
        clock.0 = text;
    }
    if left == 0 && outbox.0.is_empty() {
        outbox.0.push(CombatAction::Ready(true));
        state.1 = -5.0;
    }
}

/// Quick signals without chat: hover a unit and press 1-5.
fn send_pings(keys: Res<ButtonInput<KeyCode>>, units: Query<(&UnitWidget, &Hovered)>, mut net: Option<ResMut<crate::net::Net>>) {
    let Some(net) = net.as_mut() else { return };
    let kind = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5].iter().position(|k| keys.just_pressed(*k));
    let (Some(kind), Some((w, _))) = (kind, units.iter().find(|(_, h)| h.get())) else { return };
    net.node.ping(kind as u8, w.unit);
}

fn show_pings(
    mut commands: Commands,
    mut status: ResMut<crate::net::NetStatus>,
    session: Res<Session>,
    layer: Single<Entity, With<FloatLayer>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
) {
    for (from, kind, at) in status.pings.drain(..) {
        let Some((foot, size)) = unit_place(&session.shown, at) else { continue };
        let color = session.shown.hero(from).map(|h| theme::hero_color(&h.def).lighter(0.25)).unwrap_or(theme::BONE);
        commands.spawn((
            Floater { ttl: 2.2, life: 2.2 },
            theme::text(l10n.t(&format!("ping-{kind}")), &fonts.title, 26.0, color),
            TextShadow::default(),
            Node { position_type: PositionType::Absolute, left: px(foot.x - 90.0), top: px(foot.y - size.y - 30.0), ..default() },
            ChildOf(*layer),
        ));
    }
}

pub fn style_buttons(mut buttons: Query<(&Btn, &Hovered, Has<Pressed>, &mut BackgroundColor), With<Button>>) {
    for (btn, hovered, pressed, mut bg) in &mut buttons {
        let want = match (hovered.get(), pressed) {
            (_, true) => btn.0.darker(0.06),
            (true, false) => btn.0.lighter(0.08),
            _ => btn.0,
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

#[cfg(test)]
mod relic_strip {
    use super::*;

    #[test]
    fn relics_stay_in_their_strip_however_many() {
        for count in [1, 8, 30, 31, 60, 100, 200] {
            let side = relic_side(count);
            for i in 0..count {
                let at = relic_at(i, count);
                assert!(at.x >= RELIC_LEFT && at.x + side <= RELIC_RIGHT + 1.0, "{count}: {at}");
                assert!(at.y + side <= crate::between::BAR_H, "{count}: {at}");
            }
        }
        assert_eq!(relic_side(10), 42.0, "a few relics keep their full size");
    }
}
