//! The hollow that plays for stakes: an oak with three drums turning in it. Each player lays a
//! stake of their own (a purse, a potion, a relic, an arm) and turns the drums as often as they
//! dare; the others see what was staked and how it fell.

use super::{party_panel, redraw, Canvas, CardHints, ScreenRoot, STAGE_H, STAGE_W};
use crate::art::Art;
use crate::battle::Btn;
use crate::l10n::L10n;
use crate::net::{Net, NetStatus};
use crate::session::{Db, RunOutbox};
use crate::theme::{self, Fonts};
use crate::AppState;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button};
use nav_core::combat::{SHIELD_ARM, WEAPON_ARM};
use nav_core::*;

/// One side of a drum as it is shown, and the window it turns behind.
const FACE: f32 = 190.0;
const WINDOW_H: f32 = 250.0;
const DRUM_GAP: f32 = 16.0;
/// Where the drums stand on the stage: in the oak's hollow as it is painted.
const DRUMS_LEFT: f32 = 318.0;
const DRUMS_TOP: f32 = 372.0;
const DRUMS_W: f32 = 3.0 * FACE + 2.0 * DRUM_GAP;
/// When each drum stops, seconds after the pull; a little later the hollow says what it took or gave.
const STOPS: [f32; 3] = [1.3, 1.9, 2.6];
const SHOW_AT: f32 = 3.0;
const EDGE: Color = Color::srgb(0.42, 0.33, 0.22);
const LOSS: Color = Color::srgb(0.85, 0.32, 0.25);

pub fn plugin(app: &mut App) {
    app.init_resource::<Pick>().init_resource::<Reels>().init_resource::<crate::between::BarHold>().add_systems(Update, (draw_hollow, turn_drums).chain().run_if(in_state(AppState::Omen)));
}

/// The stake the player has laid down and not yet played.
#[derive(Resource, Default)]
struct Pick(Option<Stake>);

/// The drums as this player sees them: turning from where they stood to where the last spin fell.
#[derive(Resource)]
struct Reels {
    /// The player's spin already set turning, so a redraw does not turn it again.
    seen: u32,
    from: [u8; 3],
    to: [u8; 3],
    t: f32,
    stopped: [bool; 3],
    told: bool,
}

impl Default for Reels {
    fn default() -> Self {
        Reels { seen: 0, from: [0, 3, 6], to: [0, 3, 6], t: SHOW_AT, stopped: [true; 3], told: true }
    }
}

impl Reels {
    fn turning(&self) -> bool {
        self.t < SHOW_AT
    }

    /// How far drum `k` has turned, in sides from its top.
    fn at(&self, k: usize) -> f32 {
        let u = (self.t / STOPS[k]).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - u).powi(3);
        let n = HOLLOW_FACES as i32;
        let steps = (2 + k as i32) * n + (self.to[k] as i32 - self.from[k] as i32).rem_euclid(n);
        (self.from[k] as f32 + steps as f32 * eased).rem_euclid(n as f32)
    }
}

/// The strip of sides behind a drum's window.
#[derive(Component)]
struct Drum(usize);

/// Shown only once the drums have stopped.
#[derive(Component)]
struct AfterSpin;

/// Shown only while they turn.
#[derive(Component)]
struct WhileTurning;

/// A thing won or lost, as a small picture: the relic or the flask itself, a coin with its sum,
/// the hero's own hand.
fn loot_icon(commands: &mut Commands, row: Entity, loot: &Loot, hero: &str, art: &Art, assets: &AssetServer, fonts: &Fonts) {
    let frame = Node { width: px(72), height: px(72), padding: UiRect::all(px(4)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
    let slot = commands.spawn((frame, BackgroundColor(theme::SOOT), BorderColor::all(EDGE), Pickable::IGNORE, ChildOf(row))).id();
    let whole = Node { width: percent(100), height: percent(100), ..default() };
    match loot {
        Loot::Relic(id) => {
            commands.spawn((art.node(assets, &format!("relics/{id}"), theme::SOOT), whole, Pickable::IGNORE, ChildOf(slot)));
        }
        Loot::Potion(id) => {
            commands.spawn((art.node(assets, &format!("potions/{id}"), theme::SOOT), whole, Pickable::IGNORE, ChildOf(slot)));
        }
        Loot::Arm(_) => {
            let hand: Handle<Image> = assets.load(crate::art::path(&format!("hands/{hero}_paper.png")));
            commands.spawn((ImageNode::new(hand), Node { width: percent(100), ..default() }, Pickable::IGNORE, ChildOf(slot)));
        }
        Loot::Gold(n) => {
            commands.entity(slot).insert((BackgroundColor(Color::srgb(0.45, 0.30, 0.10)), BorderColor::all(theme::CANDLE), Node { width: px(72), height: px(72), border: UiRect::all(px(3)), border_radius: BorderRadius::MAX, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }));
            commands.spawn((theme::text(n.to_string(), &fonts.num, 28.0, theme::CANDLE), TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK }, ChildOf(slot)));
        }
        Loot::MaxHp(n) => {
            commands.entity(slot).insert((BackgroundColor(theme::RUST_BLOOD), BorderColor::all(theme::FRESH_BLOOD), Node { width: px(72), height: px(72), border: UiRect::all(px(3)), border_radius: BorderRadius::MAX, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }));
            commands.spawn((theme::text(format!("-{n}"), &fonts.num, 28.0, theme::BONE), TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK }, ChildOf(slot)));
        }
    }
}

fn arm_word(arm: u8) -> &'static str {
    if arm == WEAPON_ARM { "weapon" } else { "shield" }
}

fn loot_text(l10n: &L10n, loot: &Loot) -> String {
    match loot {
        Loot::Gold(n) => l10n.n("hollow-loot-gold", *n),
        Loot::Relic(id) => l10n.t(&format!("relic-{id}")),
        Loot::Potion(id) => l10n.t(&format!("potion-{id}")),
        Loot::Arm(arm) => l10n.t(&format!("hollow-loot-arm-{}", arm_word(*arm))),
        Loot::MaxHp(n) => l10n.n("hollow-loot-maxhp", *n),
    }
}

/// What a stake not yet played is called.
fn stake_text(l10n: &L10n, pl: &Player, stake: Stake) -> String {
    match stake {
        Stake::Gold(n) => l10n.n("hollow-loot-gold", n),
        Stake::Relic(i) => pl.relics.get(i as usize).map(|id| l10n.t(&format!("relic-{id}"))).unwrap_or_default(),
        Stake::Potion(i) => pl.potions.get(i as usize).map(|id| l10n.t(&format!("potion-{id}"))).unwrap_or_default(),
        Stake::Arm(arm) if pl.lost_arms & arm != 0 => l10n.n("hollow-stake-seam", HOLLOW_SEAM_PCT),
        Stake::Arm(arm) => l10n.t(&format!("hollow-arm-{}", arm_word(arm))),
    }
}

/// What was staked in a spin already played: what was lost names itself, what was kept is still
/// where it was.
fn staked_text(l10n: &L10n, pl: &Player, spin: &Spin) -> String {
    if spin.for_seam {
        return l10n.n("hollow-stake-seam", HOLLOW_SEAM_PCT);
    }
    match (spin.stake, spin.lost.first()) {
        (Stake::Arm(arm), _) => l10n.t(&format!("hollow-arm-{}", arm_word(arm))),
        (_, Some(lost)) => loot_text(l10n, lost),
        (stake, None) => stake_text(l10n, pl, stake),
    }
}

/// What three alike would give for this stake, tier by tier.
fn prize_lines(l10n: &L10n, pl: &Player, stake: Stake) -> Vec<String> {
    match stake {
        Stake::Gold(n) => (1..=3).map(|tier| l10n.tf(&format!("hollow-prize-{tier}"), &[("what", l10n.n("hollow-prize-gold", n * (hollow_pays(tier) - 1)).into())])).collect(),
        Stake::Potion(_) => (1..=3).map(|tier| l10n.tf(&format!("hollow-prize-{tier}"), &[("what", l10n.t(&format!("hollow-prize-potion-{tier}")).into())])).collect(),
        Stake::Relic(_) => (1..=3).map(|tier| l10n.tf(&format!("hollow-prize-{tier}"), &[("what", l10n.t(&format!("hollow-prize-relic-{tier}")).into())])).collect(),
        Stake::Arm(arm) if pl.lost_arms & arm != 0 => vec![l10n.t("hollow-prize-seam"), l10n.n("hollow-seam-lost", HOLLOW_SEAM_PCT)],
        Stake::Arm(_) => (1..=3).map(|tier| l10n.tf(&format!("hollow-prize-{tier}"), &[("what", l10n.t(&format!("hollow-prize-arm-{tier}")).into())])).collect(),
    }
}

fn face_image(art: &Art, assets: &AssetServer, face: u8) -> (ImageNode, BackgroundColor) {
    art.node(assets, &format!("hollow/face_{face}"), Color::srgb(0.10 + 0.08 * face as f32, 0.30, 0.22))
}

#[allow(clippy::too_many_arguments)]
fn draw_hollow(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut pick: ResMut<Pick>,
    mut reels: ResMut<Reels>,
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Hollow { spins, done, last } = &run.stage else {
        if reels.seen != 0 || pick.0.is_some() {
            *reels = Reels::default();
            pick.0 = None;
        }
        return;
    };
    let m = me as usize;
    let pl = &run.players[m];
    // my drums are set turning by a spin of mine not yet shown
    if spins[m] != reels.seen {
        reels.seen = spins[m];
        if let Some(spin) = &last[m] {
            reels.from = reels.to;
            reels.to = spin.faces;
            reels.t = 0.0;
            reels.stopped = [false; 3];
            reels.told = false;
        }
        // a purse may be laid again as it was; anything else has changed hands or places
        if !matches!(pick.0, Some(Stake::Gold(_))) {
            pick.0 = None;
        }
        status.run_dirty = true;
    }
    if pick.0.is_some_and(|stake| !stake_open(&db.0, pl, stake)) {
        pick.0 = None;
    }
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Omen, *canvas, Color::srgb(0.05, 0.05, 0.06)) else { return };
    // the oak itself; until it is painted, the act's forest in shadow
    if art.image(&assets, "bg/hollow").is_some() {
        commands.spawn((crate::menu::backdrop(&art, &assets, "bg/hollow", 1.0), ChildOf(root)));
    } else {
        crate::between::act_backdrop(&mut commands, root, run, &art, &assets, Color::srgb(0.22, 0.22, 0.25));
    }
    party_panel(&mut commands, root, run, me, &fonts, &l10n);

    // ---- the drums ----
    let frame = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(DRUMS_LEFT - 20.0),
                top: px(DRUMS_TOP - 20.0),
                width: px(DRUMS_W + 40.0),
                height: px(WINDOW_H + 40.0),
                padding: UiRect::all(px(16)),
                border: UiRect::all(px(4)),
                border_radius: BorderRadius::all(px(18)),
                column_gap: px(DRUM_GAP),
                ..default()
            },
            BackgroundColor(Color::srgb(0.035, 0.028, 0.024).with_alpha(0.94)),
            BorderColor::all(Color::srgb(0.27, 0.19, 0.11)),
            BoxShadow::new(Color::BLACK.with_alpha(0.85), px(0), px(0), px(6), px(34)),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    for k in 0..3 {
        let window = commands
            .spawn((
                Node { width: px(FACE), height: px(WINDOW_H), overflow: Overflow::clip(), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(10)), ..default() },
                BackgroundColor(Color::srgb(0.085, 0.07, 0.058)),
                BorderColor::all(EDGE),
                Pickable::IGNORE,
                ChildOf(frame),
            ))
            .id();
        // the sides in their order, one more above and below so the drum never shows its end
        let strip = commands
            .spawn((
                Drum(k),
                Node { position_type: PositionType::Absolute, left: px(-2), top: px(-2), width: px(FACE), flex_direction: FlexDirection::Column, ..default() },
                UiTransform::from_translation(Val2::px(0.0, strip_y(reels.at(k)))),
                Pickable::IGNORE,
                ChildOf(window),
            ))
            .id();
        for i in -1..=HOLLOW_FACES as i32 + 1 {
            let face = i.rem_euclid(HOLLOW_FACES as i32) as u8;
            commands.spawn((face_image(&art, &assets, face), Node { width: px(FACE), height: px(FACE), flex_shrink: 0.0, ..default() }, Pickable::IGNORE, ChildOf(strip)));
        }
        // the drum is round: its sides fall into shadow above and below
        for (top, from, to) in [(0.0, 0.92, 0.0), (WINDOW_H - 64.0, 0.0, 0.92)] {
            commands.spawn((
                Node { position_type: PositionType::Absolute, left: px(-2), top: px(top - 2.0), width: px(FACE), height: px(64), ..default() },
                BackgroundGradient::from(LinearGradient::to_bottom(vec![ColorStop::new(Color::BLACK.with_alpha(from), percent(0)), ColorStop::new(Color::BLACK.with_alpha(to), percent(100))])),
                Pickable::IGNORE,
                ChildOf(window),
            ));
        }
    }

    // ---- what the last spin of mine came to ----
    let under = commands
        .spawn((
            AfterSpin,
            Node {
                position_type: PositionType::Absolute,
                left: px(DRUMS_LEFT - 120.0),
                top: px(DRUMS_TOP + WINDOW_H + 40.0),
                width: px(DRUMS_W + 240.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            if reels.turning() { Visibility::Hidden } else { Visibility::Inherited },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    if let Some(spin) = &last[m] {
        let under = commands
            .spawn((
                Node { padding: UiRect::axes(px(34), px(12)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(12)), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(4), ..default() },
                BackgroundColor(Color::srgb(0.035, 0.028, 0.024).with_alpha(0.9)),
                BorderColor::all(if spin.tier > 0 { theme::CANDLE } else { LOSS.with_alpha(0.7) }),
                Pickable::IGNORE,
                ChildOf(under),
            ))
            .id();
        let (head, color, things) = if spin.tier > 0 {
            (l10n.t(&format!("hollow-won-{}", spin.tier)), if spin.tier == 3 { theme::CANDLE } else { theme::ROTLIGHT }, &spin.won)
        } else {
            (l10n.t("hollow-lost"), LOSS, &spin.lost)
        };
        commands.spawn((theme::text(head, &fonts.title, 44.0, color), TextLayout::justify(Justify::Center), ChildOf(under)));
        let icons = commands.spawn((Node { column_gap: px(10), margin: UiRect::vertical(px(4)), ..default() }, Pickable::IGNORE, ChildOf(under))).id();
        for loot in things.iter() {
            loot_icon(&mut commands, icons, loot, &pl.hero, &art, &assets, &fonts);
        }
        let what = things.iter().map(|l| loot_text(&l10n, l)).collect::<Vec<_>>().join(", ");
        if !what.is_empty() {
            let line = l10n.tf(if spin.tier > 0 { "hollow-gives" } else { "hollow-takes" }, &[("what", what.into())]);
            commands.spawn((theme::text(line, &fonts.body, 26.0, theme::BONE), TextLayout::justify(Justify::Center), Node { max_width: px(DRUMS_W + 240.0), ..default() }, ChildOf(under)));
        }

    }

    // ---- what three alike are worth ----
    let table = commands
        .spawn((
            theme::panel(Node {
                position_type: PositionType::Absolute,
                left: px(DRUMS_LEFT - 20.0),
                top: px(STAGE_H - 150.0),
                width: px(DRUMS_W + 40.0),
                padding: UiRect::axes(px(16), px(10)),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(10)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            }),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    for (tier, faces) in [(1u8, 0..4u8), (2, 4..7), (3, 7..8)] {
        let group = commands.spawn((Node { align_items: AlignItems::Center, column_gap: px(4), ..default() }, Pickable::IGNORE, ChildOf(table))).id();
        for face in faces {
            commands.spawn((face_image(&art, &assets, face), Node { width: px(46), height: px(46), ..default() }, Pickable::IGNORE, ChildOf(group)));
        }
        commands.spawn((
            theme::text(format!("×{}", hollow_pays(tier)), &fonts.num, 30.0, if tier == 3 { theme::CANDLE } else { theme::BONE }),
            Node { margin: UiRect::left(px(6)), ..default() },
            ChildOf(group),
        ));
    }

    // ---- the others at the hollow: what they staked and how it fell ----
    if run.players.len() > 1 {
        let feed = commands
            .spawn((
                Node { position_type: PositionType::Absolute, left: px(16), top: px(crate::between::BAR_H + 150.0), width: px(250), flex_direction: FlexDirection::Column, row_gap: px(10), ..default() },
                Pickable::IGNORE,
                ChildOf(root),
            ))
            .id();
        for (q, other) in run.players.iter().enumerate().filter(|(q, _)| *q != m) {
            let card = commands
                .spawn((
                    theme::panel(Node { padding: UiRect::all(px(8)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }),
                    Pickable::IGNORE,
                    ChildOf(feed),
                ))
                .id();
            commands.spawn((theme::text(l10n.t(&format!("hero-{}", other.hero)), &fonts.title, 22.0, theme::hero_color(&other.hero)), ChildOf(card)));
            match &last[q] {
                Some(spin) => {
                    let row = commands.spawn((Node { column_gap: px(4), ..default() }, Pickable::IGNORE, ChildOf(card))).id();
                    for face in spin.faces {
                        commands.spawn((face_image(&art, &assets, face), Node { width: px(54), height: px(54), border: UiRect::all(px(1)), ..default() }, BorderColor::all(EDGE), Pickable::IGNORE, ChildOf(row)));
                    }
                    commands.spawn((theme::text(l10n.tf("hollow-other-stake", &[("what", staked_text(&l10n, other, spin).into())]), &fonts.body, 18.0, theme::BONE), Node { max_width: px(230), ..default() }, ChildOf(card)));
                    let things = if spin.tier > 0 { &spin.won } else { &spin.lost };
                    let what = things.iter().map(|l| loot_text(&l10n, l)).collect::<Vec<_>>().join(", ");
                    let line = l10n.tf(if spin.tier > 0 { "hollow-other-won" } else { "hollow-other-lost" }, &[("what", what.into())]);
                    commands.spawn((theme::text(line, &fonts.body, 18.0, if spin.tier > 0 { theme::ROTLIGHT } else { LOSS }), Node { max_width: px(230), ..default() }, ChildOf(card)));
                }
                None => {
                    commands.spawn((theme::text(l10n.t(if done[q] { "hollow-other-away" } else { "hollow-other-thinks" }), &fonts.body, 18.0, theme::DIM), ChildOf(card)));
                }
            }
            if done[q] && last[q].is_some() {
                commands.spawn((theme::text(l10n.t("hollow-other-away"), &fonts.body, 18.0, theme::DIM), ChildOf(card)));
            }
        }
    }

    // ---- my side: the stakes ----
    let column = commands
        .spawn((
            theme::panel(Node {
                position_type: PositionType::Absolute,
                left: px(1090),
                top: px(crate::between::BAR_H + 26.0),
                width: px(790),
                max_height: px(STAGE_H - crate::between::BAR_H - 52.0),
                padding: UiRect::axes(px(26), px(20)),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(12)),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                ..default()
            }),
            AfterSpin,
            if reels.turning() { Visibility::Hidden } else { Visibility::Inherited },
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        theme::panel(Node {
            position_type: PositionType::Absolute,
            left: px(1090),
            top: px(crate::between::BAR_H + 26.0),
            width: px(790),
            padding: UiRect::axes(px(26), px(20)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(12)),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            ..default()
        }),
        WhileTurning,
        if reels.turning() { Visibility::Inherited } else { Visibility::Hidden },
        Pickable::IGNORE,
        ChildOf(root),
        children![theme::text(l10n.t("omen-greedy_hollow"), &fonts.title, 50.0, theme::CANDLE), theme::text(l10n.t("hollow-turning"), &fonts.body, 24.0, theme::DIM)],
    ));
    commands.spawn((theme::text(l10n.t("omen-greedy_hollow"), &fonts.title, 50.0, theme::CANDLE), ChildOf(column)));
    if done[m] {
        commands.spawn((theme::text(l10n.t("hollow-waiting"), &fonts.body, 24.0, theme::DIM), Node { max_width: px(730), ..default() }, ChildOf(column)));
        return;
    }
    commands.spawn((theme::text(l10n.t("hollow-hint"), &fonts.body, 22.0, theme::BONE), Node { max_width: px(730), margin: UiRect::bottom(px(4)), ..default() }, ChildOf(column)));

    let head = |commands: &mut Commands, key: &str| {
        commands.spawn((theme::text(l10n.t(key), &fonts.title, 24.0, theme::DIM), Node { margin: UiRect::top(px(4)), ..default() }, ChildOf(column)));
    };
    // a button that lays a stake down: it is played by the pull, not by this
    let lay = |commands: &mut Commands, row: Entity, stake: Stake, node: Node| -> Entity {
        let open = stake_open(&db.0, pl, stake);
        let on = pick.0 == Some(stake);
        let base = if on { theme::SWAMP } else if open { theme::PEAT } else { theme::SOOT };
        let mut b = commands.spawn((node, BackgroundColor(base), BorderColor::all(if on { theme::CANDLE } else { EDGE }), ChildOf(row)));
        if open {
            b.insert((Button, Hovered::default(), Btn(base)));
            b.observe(move |_: On<Activate>, mut pick: ResMut<Pick>, mut status: ResMut<NetStatus>| {
                pick.0 = if pick.0 == Some(stake) { None } else { Some(stake) };
                status.run_dirty = true;
            });
        }
        b.id()
    };
    let row = |commands: &mut Commands| commands.spawn((Node { column_gap: px(10), row_gap: px(10), flex_wrap: FlexWrap::Wrap, ..default() }, ChildOf(column))).id();

    head(&mut commands, "hollow-stake-gold");
    let gold = row(&mut commands);
    for n in HOLLOW_GOLD_STAKES {
        let open = pl.gold >= n;
        let b = lay(&mut commands, gold, Stake::Gold(n), Node { width: px(150), height: px(58), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() });
        commands.spawn((theme::text(n.to_string(), &fonts.num, 32.0, if open { theme::CANDLE } else { theme::CANDLE.with_alpha(0.35) }), ChildOf(b)));
    }

    if !pl.potions.is_empty() {
        head(&mut commands, "hollow-stake-potions");
        let potions = row(&mut commands);
        for (i, id) in pl.potions.iter().enumerate() {
            let b = lay(&mut commands, potions, Stake::Potion(i as u8), Node { width: px(68), height: px(68), padding: UiRect::all(px(4)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), ..default() });
            commands.entity(b).insert(CardHints::below(vec![(l10n.t(&format!("potion-{id}")), l10n.t(&format!("potion-{id}-desc")))]));
            commands.spawn((art.node(&assets, &format!("potions/{id}"), theme::SOOT), Node { width: percent(100), height: percent(100), ..default() }, Pickable::IGNORE, ChildOf(b)));
        }
    }

    let stakeable: Vec<(usize, &Id)> = pl.relics.iter().enumerate().filter(|(i, _)| stake_open(&db.0, pl, Stake::Relic(*i as u8))).collect();
    if !stakeable.is_empty() {
        head(&mut commands, "hollow-stake-relics");
        let relics = row(&mut commands);
        for (i, id) in stakeable {
            let b = lay(&mut commands, relics, Stake::Relic(i as u8), Node { width: px(68), height: px(68), padding: UiRect::all(px(4)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), ..default() });
            commands.entity(b).insert(CardHints::below(vec![(l10n.t(&format!("relic-{id}")), l10n.t(&format!("relic-{id}-desc")))]));
            commands.spawn((art.node(&assets, &format!("relics/{id}"), theme::SOOT), Node { width: percent(100), height: percent(100), ..default() }, Pickable::IGNORE, ChildOf(b)));
        }
    }

    head(&mut commands, "hollow-stake-arms");
    let arms = row(&mut commands);
    for arm in [WEAPON_ARM, SHIELD_ARM] {
        let gone = pl.lost_arms & arm != 0;
        let open = stake_open(&db.0, pl, Stake::Arm(arm));
        let b = lay(&mut commands, arms, Stake::Arm(arm), Node { width: px(360), min_height: px(58), padding: UiRect::axes(px(14), px(8)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, ..default() });
        let ink = if open { theme::BONE } else { theme::BONE.with_alpha(0.4) };
        commands.spawn((theme::text(l10n.t(&format!("hollow-arm-{}{}", arm_word(arm), if gone { "-back" } else { "" })), &fonts.body, 24.0, ink), ChildOf(b)));
        let note = if gone { l10n.n("hollow-arm-back-holds", HOLLOW_SEAM_PCT) } else { l10n.t(&format!("hollow-arm-{}-holds", arm_word(arm))) };
        commands.spawn((theme::text(note, &fonts.body, 17.0, theme::DIM), ChildOf(b)));
    }

    // what is on the table, and what three alike would bring for it
    let table = commands
        .spawn((
            Node { margin: UiRect::top(px(8)), padding: UiRect::axes(px(16), px(10)), border: UiRect { left: px(6), ..UiRect::all(px(1)) }, flex_direction: FlexDirection::Column, row_gap: px(3), min_height: px(128), ..default() },
            BackgroundColor(theme::SOOT.with_alpha(0.8)),
            BorderColor { left: theme::CANDLE, ..BorderColor::all(EDGE) },
            Pickable::IGNORE,
            ChildOf(column),
        ))
        .id();
    match pick.0 {
        Some(stake) => {
            commands.spawn((theme::text(l10n.tf("hollow-on-table", &[("what", stake_text(&l10n, pl, stake).into())]), &fonts.body, 24.0, theme::CANDLE), ChildOf(table)));
            if matches!(stake, Stake::Arm(arm) if pl.lost_arms & arm == 0) {
                commands.spawn((theme::text(l10n.t("hollow-arm-warning"), &fonts.body, 20.0, LOSS), ChildOf(table)));
            }
            for line in prize_lines(&l10n, pl, stake) {
                commands.spawn((theme::text(line, &fonts.body, 20.0, theme::BONE), ChildOf(table)));
            }
        }
        None => {
            commands.spawn((theme::text(l10n.t("hollow-nothing-laid"), &fonts.body, 22.0, theme::DIM), ChildOf(table)));
        }
    }

    let buttons = commands.spawn((Node { margin: UiRect::top(px(8)), column_gap: px(16), ..default() }, ChildOf(column))).id();
    let ready = pick.0.is_some();
    let base = if ready { theme::RUST_BLOOD } else { theme::SOOT };
    let mut pull = commands.spawn((
        Node { width: px(420), height: px(72), border: UiRect::all(px(3)), border_radius: BorderRadius::all(px(10)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        BackgroundColor(base),
        BorderColor::all(if ready { theme::CANDLE } else { EDGE }),
        ChildOf(buttons),
        children![theme::text(l10n.t("hollow-spin"), &fonts.title, 36.0, if ready { theme::BONE } else { theme::BONE.with_alpha(0.35) })],
    ));
    if ready {
        pull.insert((Button, Hovered::default(), Btn(base)));
        pull.observe(|_: On<Activate>, pick: Res<Pick>, reels: Res<Reels>, mut out: ResMut<RunOutbox>| {
            // one pull at a time: the drums are let stop first
            if let (Some(stake), false) = (pick.0, reels.turning()) {
                out.0.push(RunAction::Spin(stake));
            }
        });
    }
    let mut leave = commands.spawn((
        Button,
        Hovered::default(),
        Btn(theme::PEAT),
        Node { width: px(300), height: px(72), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(10)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        BackgroundColor(theme::PEAT),
        BorderColor::all(EDGE),
        ChildOf(buttons),
        children![theme::text(l10n.t("hollow-leave"), &fonts.title, 30.0, theme::BONE)],
    ));
    leave.observe(|_: On<Activate>, reels: Res<Reels>, mut out: ResMut<RunOutbox>| {
        if !reels.turning() {
            out.0.push(RunAction::LeaveHollow);
        }
    });
    let _ = STAGE_W;
}

/// Where a drum's strip hangs when it has turned `at` sides: that side in the middle of the window.
fn strip_y(at: f32) -> f32 {
    (WINDOW_H - FACE) / 2.0 - (at + 1.0) * FACE
}

/// The drums turn, slow and stop one after another, each with a knock; then the hollow speaks.
#[allow(clippy::too_many_arguments)]
fn turn_drums(
    mut commands: Commands,
    time: Res<Time>,
    mut reels: ResMut<Reels>,
    net: Option<Res<Net>>,
    mut drums: Query<(&Drum, &mut UiTransform)>,
    mut after: Query<&mut Visibility, (With<AfterSpin>, Without<WhileTurning>)>,
    mut during: Query<&mut Visibility, (With<WhileTurning>, Without<AfterSpin>)>,
    mut hold: ResMut<crate::between::BarHold>,
    sfx: Res<crate::audio::Sfx>,
    mixer: Res<crate::audio::Mixer>,
) {
    if reels.turning() {
        reels.t += time.delta_secs();
        for k in 0..3 {
            if !reels.stopped[k] && reels.t >= STOPS[k] {
                reels.stopped[k] = true;
                sfx.play(&mut commands, if k == 2 { "lot_land" } else { "lot_tick" }, &mixer, 1.0);
            }
        }
    }
    for (drum, mut tf) in &mut drums {
        tf.translation = Val2::px(0.0, strip_y(reels.at(drum.0)));
    }
    let shown = !reels.turning();
    for mut vis in &mut after {
        vis.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }
    for mut vis in &mut during {
        vis.set_if_neq(if shown { Visibility::Hidden } else { Visibility::Inherited });
    }
    // nor does the purse along the top say anything before the drums do
    if shown && hold.0 != reels.seen {
        hold.0 = reels.seen;
    }
    // what the spin came to is heard as it is told
    if shown && !reels.told {
        reels.told = true;
        let spin = net.as_ref().and_then(|n| Some((n.node.state.as_ref()?, n.node.my_slot()?))).and_then(|(run, me)| match &run.stage {
            Stage::Hollow { last, .. } => last.get(me as usize).cloned().flatten(),
            _ => None,
        });
        if let Some(spin) = spin {
            let sound = if spin.tier > 0 {
                "gold"
            } else if spin.lost.iter().any(|l| matches!(l, Loot::Arm(_))) {
                "limb_off"
            } else {
                "status"
            };
            sfx.play(&mut commands, sound, &mixer, 1.0);
        }
    }
}
