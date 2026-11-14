//! The draw and discard piles. Pressed, a pile spreads its cards out to be read: the draw pile
//! sorted by name, so its order stays secret, the discard as it lies, with whatever burned away
//! after it. A reshuffle flies the discard back into the deck card by card, and a card gained in
//! the fight shows itself before it flies to the pile it joins.

use super::*;
use crate::audio::{Mixer, Sfx};
use bevy::ui_widgets::{observe, Activate, Button};

/// Where each pile sits on the stage, at the middle of its plate.
const DRAW_AT: Vec2 = Vec2::new(140.0, 1000.0);
const DISCARD_AT: Vec2 = Vec2::new(1780.0, 1000.0);
const HAND_AT: Vec2 = Vec2::new(960.0, 960.0);
/// A card back, as it flies.
const BACK: Vec2 = Vec2::new(66.0, 94.0);
/// At most this many backs fly in a reshuffle, however big the discard was.
const SHUFFLE_FLIGHT: usize = 12;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (cue_flights, fly, pike_flight, close_on_escape).chain().after(cue_effects).run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>),
    );
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum PileKind {
    Draw,
    Discard,
}

/// The spread-out pile, over everything until it is clicked away.
#[derive(Component)]
pub(crate) struct PileView(PileKind);

/// Something flying between places on the stage: a card back in a reshuffle, a gained card.
#[derive(Component)]
struct Flight {
    from: Vec2,
    to: Vec2,
    /// Seconds before it sets off; it waits hidden, or shown where it starts with `show_first`.
    wait: f32,
    t: f32,
    dur: f32,
    arc: f32,
    size: Vec2,
    scale: (f32, f32),
    show_first: bool,
}

/// A card the pike took off the draw pile: it leaves the pile face down, turns over on its way
/// up, is held out a moment a little below the middle of the screen, and goes to the discard.
#[derive(Component)]
struct PikeShow {
    t: f32,
    /// Its face is on (it has turned over).
    face: bool,
    card: Card,
    form: Form,
}

/// The stages of a pike's flight, in seconds: up from the pile, held out, down to the discard.
const PIKE_UP: f32 = 0.38;
const PIKE_HOLD: f32 = 0.62;
const PIKE_DOWN: f32 = 0.4;
/// Where the pike holds the card out (its centre), and how large.
const PIKE_SHOW_AT: Vec2 = Vec2::new(960.0, 610.0);
const PIKE_SHOW_K: f32 = 0.9;

/// Sends a card leaving the hand off along an arc from `from` (its centre) to `to`, shrinking
/// to `end` of its size, after `wait` seconds; it is gone when it lands.
pub(super) fn send_off(commands: &mut Commands, card: Entity, from: Vec2, to: Vec2, end: f32, wait: f32) {
    commands.entity(card).insert((
        Flight { from, to, wait, t: 0.0, dur: 0.42, arc: if to == from { 90.0 } else { 120.0 }, size: Vec2::new(CARD_W, CARD_H), scale: (1.0, end), show_first: true },
        GlobalZIndex(425),
        Pickable::IGNORE,
    ));
}

/// Where a card that has left the hand goes, as a place to fly to and a size to shrink to
/// there. One burnt away, or a power, fades where it was; one shuffled back flies to the draw
/// pile; any other goes to the discard, as played cards do, even before the fight says so.
pub(super) fn whither(h: &nav_core::Hero, uid: u32, power: bool, at: Vec2) -> (Vec2, f32) {
    if power || h.exhaust.iter().any(|c| c.uid == uid) {
        (at, 0.0)
    } else if h.draw.iter().any(|c| c.uid == uid) {
        (DRAW_AT, 0.12)
    } else {
        (DISCARD_AT, 0.12)
    }
}

/// One card back of a pile's stack, `at` from the stack's corner.
fn back(at: Vec2, face: Color) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(at.x),
            top: px(at.y),
            width: px(84),
            height: px(118),
            border: UiRect::all(px(3)),
            border_radius: BorderRadius::all(px(8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(face),
        BorderColor::all(theme::CANDLE.darker(0.12)),
        BoxShadow::new(Color::BLACK.with_alpha(0.6), px(0), px(3), px(0), px(6)),
        Pickable::IGNORE,
    )
}

/// The pile as the Spire shows it: a stack of card backs with the count on a disc at its
/// corner and its name beneath, centred on `x`; pressed, it spreads its cards out.
pub(super) fn pile_plate(x: f32, kind: PileKind, label: Label, l10n: &L10n, fonts: &Fonts) -> impl Bundle {
    let (face, name) = match kind {
        PileKind::Draw => (Color::srgb(0.24, 0.12, 0.09), "battle-draw"),
        PileKind::Discard => (Color::srgb(0.16, 0.15, 0.14), "battle-discard"),
    };
    (
        Node { position_type: PositionType::Absolute, left: px(x - 50.0), top: px(926), width: px(100), height: px(150), ..default() },
        BackgroundColor(Color::NONE),
        Button,
        Hovered::default(),
        Btn(Color::NONE),
        kind,
        observe(move |_: On<Activate>, mut commands: Commands, open: Query<(Entity, &PileView)>, show: ShowPile| {
            let was = open.iter().find(|(_, v)| v.0 == kind).is_some();
            for (e, _) in &open {
                commands.entity(e).despawn();
            }
            if !was {
                show.spread(&mut commands, kind);
            }
        }),
        children![
            back(Vec2::new(14.0, 6.0), face.darker(0.04)),
            back(Vec2::new(10.0, 3.0), face.darker(0.02)),
            (
                back(Vec2::new(6.0, 0.0), face),
                // a lozenge on the back, the deck's mark
                children![(
                    Node { width: px(30), height: px(30), border: UiRect::all(px(3)), ..default() },
                    BorderColor::all(theme::CANDLE),
                    UiTransform { rotation: Rot2::degrees(45.0), ..default() },
                    Pickable::IGNORE,
                )],
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    right: px(-8),
                    top: px(84),
                    width: px(44),
                    height: px(44),
                    border: UiRect::all(px(3)),
                    border_radius: BorderRadius::MAX,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(theme::SOOT),
                BorderColor::all(theme::CANDLE),
                Pickable::IGNORE,
                children![(label, theme::text("", &fonts.num, 24.0, theme::BONE))],
            ),
            (
                theme::text(l10n.t(name), &fonts.body, 17.0, theme::DIM),
                Node { position_type: PositionType::Absolute, top: px(126), width: px(112), left: px(0), ..default() },
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
            ),
        ],
    )
}

#[derive(bevy::ecs::system::SystemParam)]
struct ShowPile<'w, 's> {
    session: Res<'w, Session>,
    db: Res<'w, Db>,
    l10n: Res<'w, L10n>,
    fonts: Res<'w, Fonts>,
    art: Res<'w, Art>,
    assets: Res<'w, AssetServer>,
    layer: Single<'w, 's, Entity, With<FloatLayer>>,
}

impl ShowPile<'_, '_> {
    fn spread(&self, commands: &mut Commands, kind: PileKind) {
        let h = self.session.my_hero();
        let l10n = &*self.l10n;
        let mut sections: Vec<(String, Vec<Card>)> = Vec::new();
        match kind {
            PileKind::Draw => {
                let mut cards = h.draw.clone();
                // in the order of their names, not of the deck: what comes next stays unknown
                cards.sort_by_key(|c| (l10n.card_name(&c.def, c.upgraded), c.uid));
                sections.push((format!("{} · {}", l10n.t("battle-draw"), cards.len()), cards));
            }
            PileKind::Discard => {
                sections.push((format!("{} · {}", l10n.t("battle-discard"), h.discard.len()), h.discard.iter().rev().cloned().collect()));
                if !h.exhaust.is_empty() {
                    sections.push((format!("{} · {}", l10n.t("battle-exhaust"), h.exhaust.len()), h.exhaust.clone()));
                }
            }
        }
        let count: usize = sections.iter().map(|(_, c)| c.len()).sum();
        // as small as it takes for the whole pile to fit on the screen at once
        let k = if count <= 16 { 0.78 } else if count <= 30 { 0.62 } else if count <= 48 { 0.5 } else { 0.42 };
        let root = commands
            .spawn((
                PileView(kind),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(STAGE_W),
                    height: px(STAGE_H),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(px(60), px(40)),
                    row_gap: px(14),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.86)),
                GlobalZIndex(450),
                ChildOf(*self.layer),
            ))
            .observe(|ev: On<PointerClick>, mut commands: Commands, views: Query<Entity, With<PileView>>| {
                // the right button turns a card over here; only the left one closes the view
                if ev.button != PointerButton::Primary {
                    return;
                }
                for e in &views {
                    commands.entity(e).despawn();
                }
            })
            .id();
        if count == 0 {
            commands.spawn((theme::text(sections[0].0.clone(), &self.fonts.title, 40.0, theme::CANDLE), ChildOf(root)));
            commands.spawn((theme::text(l10n.t("battle-pile-empty"), &self.fonts.body, 26.0, theme::DIM), ChildOf(root)));
        }
        for (title, cards) in sections.into_iter().filter(|(_, c)| !c.is_empty()) {
            commands.spawn((theme::text(title, &self.fonts.title, 40.0, theme::CANDLE), ChildOf(root)));
            let row = commands
                .spawn((
                    Node { flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::Center, column_gap: px(12), row_gap: px(12), max_width: px(STAGE_W - 120.0), ..default() },
                    Pickable::IGNORE,
                    ChildOf(root),
                ))
                .id();
            for card in &cards {
                // a slot of the shrunk size; the card inside is full size and scaled about its middle
                let slot = commands.spawn((Node { width: px(CARD_W * k), height: px(CARD_H * k), ..default() }, Pickable::IGNORE, ChildOf(row))).id();
                let at = Vec2::new(-CARD_W * (1.0 - k) / 2.0, -CARD_H * (1.0 - k) / 2.0);
                if let Some(e) = spawn_card(commands, slot, card, h.form, Some(at), false, (), &self.db.0, l10n, &self.fonts, &self.art, &self.assets) {
                    // hoverable to be turned over, and a click still goes through to close the view
                    commands.entity(e).insert((UiTransform { scale: Vec2::splat(k), ..default() }, Pickable { should_block_lower: false, is_hoverable: true }));
                    // what its keywords mean, told beside it on hover, as in the hand
                    if let Some(def) = self.db.0.cards.get(&card.def) {
                        commands.entity(e).insert((Hovered::default(), crate::run_ui::CardHints::new(l10n.card_glossary(&def.view(card.upgraded, h.form)))));
                    }
                    if self.db.0.cards.get(&card.def).is_some_and(|def| crate::card_flip::upgrades(def, h.form, l10n)) {
                        commands.entity(e).insert(crate::card_flip::CardFlip::new(card.clone(), h.form));
                    }
                }
            }
        }
        commands.spawn((theme::text(l10n.t("battle-pile-close"), &self.fonts.body, 20.0, theme::DIM), ChildOf(root)));
    }
}

fn close_on_escape(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, views: Query<Entity, With<PileView>>) {
    if keys.just_pressed(KeyCode::Escape) {
        for e in &views {
            commands.entity(e).despawn();
        }
    }
}

/// Starts the flights the fight's events call for.
#[allow(clippy::too_many_arguments)]
fn cue_flights(
    mut commands: Commands,
    cues: Res<Cues>,
    session: Res<Session>,
    db: Res<Db>,
    l10n: Res<L10n>,
    fonts: Res<Fonts>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    layer: Single<Entity, With<FloatLayer>>,
    sfx: Res<Sfx>,
    mixer: Res<Mixer>,
) {
    let me = session.me;
    let mut gained = 0;
    for e in &cues.0 {
        match e {
            Event::Shuffled { player } if *player == me => {
                let n = session.my_hero().draw.len().clamp(3, SHUFFLE_FLIGHT);
                for i in 0..n {
                    commands.spawn((
                        Flight { from: DISCARD_AT, to: DRAW_AT, wait: i as f32 * 0.045, t: 0.0, dur: 0.5, arc: 170.0 + (i % 3) as f32 * 30.0, size: BACK, scale: (1.0, 1.0), show_first: false },
                        Node { position_type: PositionType::Absolute, width: px(BACK.x), height: px(BACK.y), border: UiRect::all(px(3)), border_radius: BorderRadius::all(px(7)), ..default() },
                        BackgroundColor(theme::PEAT),
                        BorderColor::all(theme::CANDLE),
                        UiTransform::IDENTITY,
                        Visibility::Hidden,
                        GlobalZIndex(420),
                        Pickable::IGNORE,
                        ChildOf(*layer),
                    ));
                }
                sfx.play(&mut commands, "card_shuffle", &mixer, 1.0);
            }
            Event::PikeDrew { player, card, upgraded } if *player == me => {
                // the back of a card, with the deck's lozenge, comes up off the draw pile
                let shown = Card { uid: u32::MAX, def: card.clone(), upgraded: *upgraded, free: false };
                commands.spawn((
                    PikeShow { t: 0.0, face: false, card: shown, form: session.my_hero().form },
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(DRAW_AT.x - CARD_W / 2.0),
                        top: px(DRAW_AT.y - CARD_H / 2.0),
                        width: px(CARD_W),
                        height: px(CARD_H),
                        border: UiRect::all(px(4)),
                        border_radius: BorderRadius::all(px(12)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.24, 0.12, 0.09)),
                    BorderColor::all(theme::CANDLE.darker(0.12)),
                    BoxShadow::new(Color::BLACK.with_alpha(0.6), px(0), px(8), px(0), px(18)),
                    UiTransform { scale: Vec2::splat(0.3), ..default() },
                    GlobalZIndex(440),
                    Pickable::IGNORE,
                    ChildOf(*layer),
                    children![(
                        Node { width: px(70), height: px(70), border: UiRect::all(px(6)), ..default() },
                        BorderColor::all(theme::CANDLE),
                        UiTransform { rotation: Rot2::degrees(45.0), ..default() },
                        Pickable::IGNORE,
                    )],
                ));
                sfx.play(&mut commands, "card_draw", &mixer, 1.0);
            }
            Event::CardAdded { player, card, pile } if *player == me => {
                // a card come from outside the deck shows itself first, then goes where it belongs
                let to = match pile {
                    Pile::Draw => DRAW_AT,
                    Pile::Discard => DISCARD_AT,
                    Pile::Hand => HAND_AT,
                };
                let k = 0.62;
                let from = Vec2::new(960.0 + gained as f32 * 90.0, 470.0);
                let shown = Card { uid: u32::MAX, def: card.clone(), upgraded: false, free: false };
                let size = Vec2::new(CARD_W, CARD_H);
                if let Some(e) = spawn_card(&mut commands, *layer, &shown, session.my_hero().form, Some(from - size / 2.0), false, (), &db.0, &l10n, &fonts, &art, &assets) {
                    commands.entity(e).insert((
                        Flight { from, to, wait: 0.55 + gained as f32 * 0.25, t: 0.0, dur: 0.45, arc: 90.0, size, scale: (k, 0.12), show_first: true },
                        UiTransform { scale: Vec2::splat(k), ..default() },
                        GlobalZIndex(430),
                        Pickable::IGNORE,
                    ));
                }
                if gained == 0 {
                    sfx.play(&mut commands, "card_gain", &mixer, 1.0);
                }
                gained += 1;
            }
            _ => {}
        }
    }
}

fn fly(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Flight, &mut Node, &mut UiTransform, &mut Visibility)>) {
    let dt = time.delta_secs();
    for (e, mut f, mut node, mut tf, mut vis) in &mut q {
        if f.wait > 0.0 {
            f.wait -= dt;
            if f.show_first && *vis != Visibility::Inherited {
                *vis = Visibility::Inherited;
            }
            continue;
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        f.t += dt / f.dur;
        if f.t >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        // slow to leave, fast to land
        let p = f.t * f.t;
        let at = f.from.lerp(f.to, p) - Vec2::Y * f.arc * (p * std::f32::consts::PI).sin();
        node.left = px(at.x - f.size.x / 2.0);
        node.top = px(at.y - f.size.y / 2.0);
        tf.scale = Vec2::splat(f.scale.0 + (f.scale.1 - f.scale.0) * p);
        tf.rotation = Rot2::radians((p * std::f32::consts::PI * 2.0).sin() * 0.25);
    }
}

/// Plays the pike's flights: up from the draw pile turning over, held out, down to the discard.
#[allow(clippy::too_many_arguments)]
fn pike_flight(
    mut commands: Commands,
    time: Res<Time>,
    db: Res<Db>,
    l10n: Res<L10n>,
    fonts: Res<Fonts>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut q: Query<(Entity, &mut PikeShow, &mut Node, &mut UiTransform, &mut BackgroundColor, &mut BorderColor)>,
) {
    let total = PIKE_UP + PIKE_HOLD + PIKE_DOWN;
    for (e, mut show, mut node, mut tf, mut bg, mut edge) in &mut q {
        show.t += time.delta_secs();
        let t = show.t;
        if t >= total {
            commands.entity(e).despawn();
            continue;
        }
        let (at, k, turn) = if t < PIKE_UP {
            // up off the pile along an arc, growing, turning over about its upright
            let p = t / PIKE_UP;
            let ease = 1.0 - (1.0 - p).powi(3);
            let at = DRAW_AT.lerp(PIKE_SHOW_AT, ease) - Vec2::Y * 120.0 * (ease * std::f32::consts::PI).sin();
            (at, 0.3 + (PIKE_SHOW_K - 0.3) * ease, p)
        } else if t < PIKE_UP + PIKE_HOLD {
            // held out, breathing a little
            let p = (t - PIKE_UP) / PIKE_HOLD;
            (PIKE_SHOW_AT, PIKE_SHOW_K * (1.0 + 0.03 * (p * std::f32::consts::PI).sin()), 1.0)
        } else {
            // slow to leave, fast to land, shrinking into the discard
            let p = (t - PIKE_UP - PIKE_HOLD) / PIKE_DOWN;
            let ease = p * p;
            let at = PIKE_SHOW_AT.lerp(DISCARD_AT, ease) - Vec2::Y * 90.0 * (ease * std::f32::consts::PI).sin();
            (at, PIKE_SHOW_K + (0.12 - PIKE_SHOW_K) * ease, 1.0)
        };
        // the turn: the back narrows to an edge, and the face widens out of it
        let width = (turn * std::f32::consts::PI).cos().abs().max(0.02);
        if turn >= 0.5 && !show.face {
            show.face = true;
            if let Some(def) = db.0.cards.get(&show.card.def) {
                let (card, form) = (show.card.clone(), show.form);
                *bg = BackgroundColor(crate::battle::card_frame(def));
                *edge = BorderColor::all(if def.hero.is_empty() { theme::DIM } else { theme::hero_color(&def.hero) });
                node.justify_content = JustifyContent::FlexStart;
                node.flex_direction = FlexDirection::Column;
                node.padding = UiRect::all(px(6));
                commands.entity(e).despawn_children().with_children(|c| crate::battle::card_face(c, &card, def, form, false, &l10n, &fonts, &art, &assets));
            }
        }
        node.left = px(at.x - CARD_W / 2.0);
        node.top = px(at.y - CARD_H / 2.0);
        tf.scale = Vec2::new(k * width, k);
    }
}
