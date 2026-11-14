//! Aiming, as in Slay the Spire: a card that needs a target, once lifted out of the hand, stops
//! in the middle above it and an arrow runs from it to the cursor; over a unit it may land on the
//! arrow turns red (a foe) or green (a friend). A potion thrown at something does the same from
//! its place on the belt. While aiming, and while an untargeted card is held up to be played,
//! the outcome is shown beforehand on every unit it touches: HP lost or healed and what is left,
//! block gained, and the energy that will remain. The forecast is the rules themselves, played
//! out on a copy of the fight.

use super::*;
use bevy::picking::hover::Hovered;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::asset::RenderAssetUsages;

/// Dots in the arrow's tail.
const DOTS: usize = 18;
/// Where a card being aimed waits: in the middle, above the hand.
pub(super) const AIM_SPOT: Vec2 = Vec2::new((STAGE_W - CARD_W) / 2.0, HAND_RISEN - 85.0);
/// How far a card must be lifted out of the hand before it is aimed, or played if it needs no aim.
pub(super) const LIFT_TO_AIM: f32 = 130.0;
/// How far a potion that needs no target must be pulled from the belt to be drunk on release.
pub(super) const POTION_PULL: f32 = 120.0;

const FOE: Color = Color::srgb(0.86, 0.18, 0.12);
const FRIEND: Color = Color::srgb(0.36, 0.82, 0.38);
const NOBODY: Color = Color::srgb(0.85, 0.81, 0.72);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Aim>().add_systems(
        Update,
        (track, (draw_arrow, forecast)).chain().after(pick_hover).before(layout_hand).run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>),
    );
    app.add_systems(Update, raise_hint.run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>));
    // the energy line is refreshed by update_labels; the forecast goes over it
    app.add_systems(Update, forecast_energy.after(update_labels).run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>));
}

/// What is being aimed, from where, and at whom.
#[derive(Resource, Default)]
pub(super) struct Aim {
    pub what: Option<Aimed>,
    pub from: Vec2,
    pub to: Vec2,
    /// The unit under the cursor, if the aimed thing may land on it.
    pub target: Option<UnitRef>,
    /// The action that would be taken on release, for the forecast.
    pub action: Option<CombatAction>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Aimed {
    Card(u32),
    Potion(u8),
}

#[derive(Component)]
pub(super) struct ArrowDot(usize);
#[derive(Component)]
pub(super) struct ArrowHead(Vec<Handle<Image>>);
/// The forecast over one unit's HP bar.
#[derive(Component)]
pub(super) struct Forecast(pub UnitRef);
/// A potion chip being dragged.
#[derive(Component)]
pub(super) struct Thrown;

/// How many headings the arrow's head is drawn in, all the way round.
const HEADINGS: usize = 36;

/// A filled triangle pointing along `angle` (screen space, y down), for the arrow's head.
/// Drawn turned rather than turned by the UI, so it needs no transform at all.
fn head_picture(angle: f32) -> Image {
    const S: u32 = 64;
    let (sin, cos) = angle.sin_cos();
    let mut data = Vec::with_capacity((S * S * 4) as usize);
    for y in 0..S {
        for x in 0..S {
            let (px_, py_) = (x as f32 / (S - 1) as f32 - 0.5, y as f32 / (S - 1) as f32 - 0.5);
            // back into the triangle's own frame, where it points right; shrunk so that turned
            // any way it still fits in the square
            let (fx, fy) = ((px_ * cos + py_ * sin) * 1.42 + 0.5, (-px_ * sin + py_ * cos) * 1.42 + 0.5);
            // a slim head, inside when |y - 0.5| < (1 - x) * 0.32, softened over a pixel: a squat one
            // looks the same turned a third of the way round, and the eye can't tell where it points
            let edge = ((1.0 - fx) * 0.32 - (fy - 0.5).abs()).min(fx);
            let a = (edge * S as f32).clamp(0.0, 1.0);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    Image::new(Extent3d { width: S, height: S, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)
}

/// The arrow's pieces, hidden until something is aimed. Spawned with the battle screen.
pub(super) fn spawn_arrow(p: &mut ChildSpawnerCommands, images: &mut Assets<Image>) {
    for i in 0..DOTS {
        let size = 9.0 + 13.0 * i as f32 / (DOTS - 1) as f32;
        p.spawn((
            ArrowDot(i),
            Node { position_type: PositionType::Absolute, width: px(size), height: px(size), border_radius: BorderRadius::all(percent(50)), border: UiRect::all(px(2)), ..default() },
            BackgroundColor(NOBODY),
            BorderColor::all(Color::BLACK.with_alpha(0.6)),
            GlobalZIndex(300),
            Visibility::Hidden,
            Pickable::IGNORE,
        ));
    }
    p.spawn((
        ArrowHead((0..HEADINGS).map(|i| images.add(head_picture(i as f32 / HEADINGS as f32 * std::f32::consts::TAU))).collect()),
        Node { position_type: PositionType::Absolute, width: px(68), height: px(68), ..default() },
        ImageNode::default(),
        GlobalZIndex(301),
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
}

/// Does a card or potion with this aim need a unit chosen for it?
pub(super) fn needs_unit(target: CardTarget, alone: bool) -> bool {
    !(target == CardTarget::None || (target == CardTarget::Ally && alone))
}

/// May a thing aimed so land on this unit?
fn takes(session: &Session, aim: CardTarget, passable: bool, unit: UnitRef, card: bool) -> bool {
    let me = session.me;
    match unit {
        UnitRef::Enemy(uid) => aim == CardTarget::Enemy && session.truth.enemy(uid).is_some_and(|e| e.alive),
        UnitRef::Hero(i) => {
            let Some(h) = session.truth.hero(i) else { return false };
            if passable {
                return i != me && !h.downed;
            }
            // any card laid on a fallen ally raises them
            if card && h.downed && i != me {
                return true;
            }
            match aim {
                CardTarget::Ally => i != me && !h.downed,
                CardTarget::AnyHero => !h.downed,
                CardTarget::Downed => h.downed,
                _ => false,
            }
        }
    }
}

/// Works out what is aimed this frame: a held card lifted out of the hand that needs a unit, or
/// a dragged potion that does; and which unit under the cursor may take it.
#[allow(clippy::too_many_arguments)]
fn track(
    session: Res<Session>,
    db: Res<Db>,
    mut aim: ResMut<Aim>,
    mut cards: Query<&mut CardWidget>,
    chips: Query<(&PotionChip, &Thrown, &UiGlobalTransform, &ComputedNode)>,
    units: Query<(&UnitWidget, &Hovered)>,
    pointers: Query<(&bevy::picking::pointer::PointerId, &bevy::picking::pointer::PointerLocation)>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Res<UiScale>,
) {
    let cursor = stage_pointer(&pointers, &window, scale.0);
    let h = session.my_hero();
    let alone = session.truth.standing_heroes().count() == 1;
    let under = units.iter().find(|(_, hov)| hov.0).map(|(w, _)| w.unit);
    // blind, a card gives nothing away: every card is aimed alike, and nothing lights up
    let blind = h.flags.contains_key("blind");
    let mut next = Aim::default();
    for mut w in &mut cards {
        let lifted = w.dragging && -w.drag.y > LIFT_TO_AIM;
        let Some(card) = h.hand.iter().find(|c| c.uid == w.uid) else { continue };
        let Some(def) = db.0.cards.get(&card.def) else { continue };
        let view = def.view(card.upgraded, h.form);
        let passable = view.has(Keyword::Passable);
        let aiming = lifted && (blind || needs_unit(view.target, alone) || passable);
        if w.aiming != aiming {
            w.aiming = aiming;
        }
        if !w.dragging {
            continue;
        }
        if aiming && blind {
            next = Aim { what: Some(Aimed::Card(w.uid)), from: AIM_SPOT + Vec2::new(CARD_W / 2.0, 0.0), to: cursor.unwrap_or_default(), target: None, action: None };
        } else if aiming {
            let target = under.filter(|u| takes(&session, view.target, passable, *u, true));
            let action = target.map(|t| match t {
                UnitRef::Hero(ally) if session.truth.hero(ally).is_some_and(|a| a.downed) && view.target != CardTarget::Downed => CombatAction::Raise { ally, discard: w.uid },
                UnitRef::Hero(to) if passable => CombatAction::Pass { card: w.uid, to },
                t => CombatAction::Play { card: w.uid, target: Some(t) },
            });
            next = Aim { what: Some(Aimed::Card(w.uid)), from: AIM_SPOT + Vec2::new(CARD_W / 2.0, 0.0), to: cursor.unwrap_or_default(), target, action };
        } else if lifted {
            // an untargeted card held over a fallen ally raises them, paid with the card: say so
            // with the arrow, or letting go looks like the card's own doing
            let fallen = under.filter(|u| matches!(u, UnitRef::Hero(i) if *i != session.me && session.truth.hero(*i).is_some_and(|a| a.downed)));
            next = match fallen {
                Some(UnitRef::Hero(ally)) => Aim {
                    what: Some(Aimed::Card(w.uid)),
                    from: AIM_SPOT + Vec2::new(CARD_W / 2.0, 0.0),
                    to: cursor.unwrap_or_default(),
                    target: fallen,
                    action: Some(CombatAction::Raise { ally, discard: w.uid }),
                },
                // no arrow, but the forecast
                _ => Aim { what: None, action: Some(CombatAction::Play { card: w.uid, target: None }), ..default() },
            };
        }
    }
    for (chip, _, at, node) in &chips {
        let Some(def) = db.0.potions.get(&chip.id) else { continue };
        // the chip's centre, from window pixels to the stage
        let centre = at.translation / window.scale_factor();
        // from the flask's lower rim: the belt hangs high, so the arrow starts below it
        let from = crate::stage_point(&window, scale.0, centre) + Vec2::new(0.0, node.size().y / window.scale_factor() / scale.0 / 2.0);
        let (target, action) = if needs_unit(def.target, alone) {
            let target = under.filter(|u| takes(&session, def.target, false, *u, false));
            (target, target.map(|t| CombatAction::UsePotion { slot: chip.slot, target: Some(t) }))
        } else {
            // a draught for nobody in particular: pulled far enough, the arrow turns green and
            // letting go drinks it
            let pulled = cursor.is_some_and(|c| c.distance(from) > POTION_PULL);
            (pulled.then_some(UnitRef::Hero(session.me)), pulled.then_some(CombatAction::UsePotion { slot: chip.slot, target: None }))
        };
        next = Aim { what: Some(Aimed::Potion(chip.slot)), from, to: cursor.unwrap_or_default(), target, action };
    }
    *aim = next;
}

/// Lays the arrow's dots along a curve rising from the aimed thing to the cursor.
fn draw_arrow(
    aim: Res<Aim>,
    mut dots: Query<(&ArrowDot, &mut Node, &mut Visibility, &mut BackgroundColor), Without<ArrowHead>>,
    mut head: Query<(&ArrowHead, &mut Node, &mut Visibility, &mut ImageNode)>,
) {
    let shown = aim.what.is_some();
    let color = match aim.target {
        Some(UnitRef::Enemy(_)) => FOE,
        Some(UnitRef::Hero(_)) => FRIEND,
        None => NOBODY,
    };
    let (a, b) = (aim.from, aim.to);
    // a curve that bows upward, more the further sideways it has to reach; from the belt high on
    // the left it only sags a little, so it never runs off the top of the screen
    let ctrl = match aim.what {
        Some(Aimed::Potion(_)) => Vec2::new(a.x + (b.x - a.x) * 0.5, (a.y.min(b.y) - 40.0).max(a.y.max(b.y) - 0.6 * (b - a).length()).max(20.0)),
        _ => Vec2::new(a.x + (b.x - a.x) * 0.15, a.y.min(b.y) - 120.0 - (b.x - a.x).abs() * 0.25),
    };
    let at = |t: f32| a * (1.0 - t) * (1.0 - t) + ctrl * 2.0 * t * (1.0 - t) + b * t * t;
    let vis = if shown { Visibility::Inherited } else { Visibility::Hidden };
    for (dot, mut node, mut v, mut bg) in &mut dots {
        if *v != vis {
            *v = vis;
        }
        if !shown {
            continue;
        }
        let t = (dot.0 + 1) as f32 / (DOTS + 2) as f32;
        let p = at(t);
        let size = 9.0 + 13.0 * dot.0 as f32 / (DOTS - 1) as f32;
        node.left = px(p.x - size / 2.0);
        node.top = px(p.y - size / 2.0);
        bg.0 = color;
    }
    let Ok((pictures, mut node, mut v, mut img)) = head.single_mut() else { return };
    if *v != vis {
        *v = vis;
    }
    if shown {
        let dir = (b - at(0.94)).normalize_or(Vec2::NEG_Y);
        node.left = px(b.x - 34.0);
        node.top = px(b.y - 34.0);
        let turn = dir.y.atan2(dir.x).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        let picture = &pictures.0[(turn * HEADINGS as f32).round() as usize % HEADINGS];
        if img.image != *picture {
            img.image = picture.clone();
        }
        img.color = color;
    }
}

/// Plays the aimed action out on a copy of the fight and writes what would change over each unit.
fn forecast(session: Res<Session>, db: Res<Db>, aim: Res<Aim>, mut labels: Query<(&Forecast, &mut Text, &mut TextColor, &mut Visibility)>) {
    let mut changes: Vec<(UnitRef, String, Color)> = vec![];
    if let Some(action) = &aim.action {
        let mut copy = session.truth.clone();
        copy.foretelling = true;
        // what is left to chance is not foretold: a random target (left out of the copy, so the
        // card's own blow still shows beside a relic striking at random), tangled paths, a charm
        let me = session.my_hero();
        let astray = ["blind", "confused", "charmed"].iter().any(|f| me.flags.contains_key(*f));
        if !astray && nav_core::apply(&db.0, &mut copy, session.me, action).is_ok() && copy.rng == session.truth.rng {
            let before = &session.truth;
            for h in &before.heroes {
                if let Some(after) = copy.hero(h.slot) {
                    changes.extend(diff(UnitRef::Hero(h.slot), (h.hp, h.block), (after.hp, after.block)));
                }
            }
            for e in before.enemies.iter().filter(|e| e.alive) {
                if let Some(after) = copy.enemy(e.uid) {
                    let now = if after.alive { (after.hp, after.block) } else { (0, 0) };
                    changes.extend(diff(UnitRef::Enemy(e.uid), (e.hp, e.block), now));
                }
            }
        }
    }
    for (f, mut text, mut color, mut vis) in &mut labels {
        let mine: Vec<&(UnitRef, String, Color)> = changes.iter().filter(|c| c.0 == f.0).collect();
        let want = if mine.is_empty() { Visibility::Hidden } else { Visibility::Inherited };
        if *vis != want {
            *vis = want;
        }
        let line = mine.iter().map(|c| c.1.as_str()).collect::<Vec<_>>().join("  ");
        if text.0 != line {
            text.0 = line;
        }
        if let Some(c) = mine.first() {
            color.0 = c.2;
        }
    }
}

/// What changes for one unit: HP lost or healed with what is left, and block gained or broken.
fn diff(unit: UnitRef, (hp, block): (i32, i32), (hp2, block2): (i32, i32)) -> Vec<(UnitRef, String, Color)> {
    let mut out = vec![];
    if hp2 < hp {
        let left = if hp2 <= 0 { "☠".to_string() } else { hp2.to_string() };
        out.push((unit, format!("−{} → {left}", hp - hp2), FOE));
    } else if hp2 > hp {
        out.push((unit, format!("+{} → {hp2}", hp2 - hp), FRIEND));
    }
    if block2 != block {
        let d = block2 - block;
        out.push((unit, format!("{}{d} ▣", if d > 0 { "+" } else { "" }), theme::FROST));
    }
    out
}

/// While a card is aimed or held up to be played, the energy line shows what will be left.
fn forecast_energy(session: Res<Session>, db: Res<Db>, aim: Res<Aim>, mut labels: Query<(&Label, &mut Text)>) {
    if session.my_hero().flags.contains_key("blind") {
        return;
    }
    let Some(CombatAction::Play { card, .. }) = aim.action.clone().or_else(|| match aim.what {
        Some(Aimed::Card(uid)) => Some(CombatAction::Play { card: uid, target: None }),
        _ => None,
    }) else {
        return;
    };
    let h = session.my_hero();
    let Some(c) = h.hand.iter().find(|c| c.uid == card) else { return };
    let Some(def) = db.0.cards.get(&c.def) else { return };
    let view = def.view(c.upgraded, h.form);
    let cost = if c.free { 0 } else if view.x_cost { h.energy } else { nav_core::combat::energy_cost(h, c, &view) };
    for (label, mut text) in &mut labels {
        if *label == Label::Energy {
            let line = format!("{} → {}", h.energy, (h.energy - cost).max(0));
            if text.0 != line {
                text.0 = line;
            }
        }
    }
}

/// The line over the hand that says how a fallen ally is got up, while one lies and this
/// hero can do it: nothing else on the screen tells it.
#[derive(Component)]
struct RaiseHint;

fn raise_hint(
    mut commands: Commands,
    session: Res<Session>,
    l10n: Res<L10n>,
    fonts: Res<Fonts>,
    stage: Single<Entity, With<crate::Stage>>,
    mut hint: Query<(&mut Visibility, &mut Text), With<RaiseHint>>,
) {
    let me = session.my_hero();
    let someone_down = !me.downed && session.truth.heroes.iter().any(|h| h.slot != session.me && h.downed && !h.beheaded);
    // a discard to pick for says so first: until it is made, nothing else can be done
    let (line, shown) = match &me.choosing {
        Some(ch) => (l10n.tf("battle-choose-discard", &[("n", (ch.left as i32).into())]), true),
        None => (l10n.tf("battle-raise-hint", &[("n", session.truth.raise_cost.into())]), someone_down),
    };
    let Ok((mut vis, mut text)) = hint.single_mut() else {
        commands.spawn((
            RaiseHint,
            Node { position_type: PositionType::Absolute, top: px(AIM_SPOT.y - 60.0), width: px(STAGE_W), justify_content: JustifyContent::Center, ..default() },
            theme::text(line, &fonts.body, 24.0, theme::CANDLE),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
            TextShadow { offset: Vec2::new(2.0, 2.0), color: Color::BLACK },
            Visibility::Hidden,
            GlobalZIndex(40),
            // `theme::text` brings its own Pickable::IGNORE: a second one panics
            DespawnOnExit(AppState::Battle),
            ChildOf(*stage),
        ));
        return;
    };
    let want = if shown { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
    if text.0 != line {
        text.0 = line;
    }
}
