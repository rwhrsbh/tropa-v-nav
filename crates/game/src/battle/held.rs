//! A card another player has in their hand's grip: the whole card, where it is on their screen,
//! with an arrow in their hero's colour to where they aim it. Let go, it flies to what it was
//! aimed at and is gone.

use super::{aim, spawn_card, CardWidget, FloatLayer, Session, CARD_H, CARD_W};
use crate::art::Art;
use crate::l10n::L10n;
use crate::net::{Net, NetStatus};
use crate::session::Db;
use crate::theme::{self, Fonts};
use crate::AppState;
use bevy::prelude::*;
use nav_core::{Card, Form};
use nav_net::Note;

/// How often the others are told where the card is, per second.
const RATE: f32 = 20.0;
/// How long a card stays with no word of it before it is taken as let go, in seconds.
const QUIET: f32 = 1.5;
/// How long a card let go takes to fly off.
const FLIGHT: f32 = 0.25;
const DOTS: usize = 9;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (tell_held, show_held).run_if(in_state(AppState::Battle)));
}

#[derive(Default)]
struct Told {
    since: f32,
    holding: bool,
}

/// What this player holds, told to the others a few times a second, and once when let go.
fn tell_held(time: Res<Time>, mut told: Local<Told>, net: Option<ResMut<Net>>, session: Res<Session>, cards: Query<&CardWidget>, aimed: Res<aim::Aim>, mut status: ResMut<NetStatus>) {
    told.since += time.delta_secs();
    let Some(mut net) = net else { return };
    let h = session.my_hero();
    let held = cards.iter().find(|w| w.dragging).and_then(|w| h.hand.iter().find(|c| c.uid == w.uid).map(|c| (w, c)));
    let msg = match held {
        Some((w, card)) => {
            if told.since < 1.0 / RATE {
                return;
            }
            let aim_at = matches!(aimed.what, Some(aim::Aimed::Card(_))).then_some(aimed.to.to_array());
            told.holding = true;
            Some((card.def.clone(), card.upgraded, w.shown.to_array(), aim_at))
        }
        None if told.holding => {
            told.holding = false;
            None
        }
        None => return,
    };
    told.since = 0.0;
    // dev: `TROPA_ECHO_HELD=1` shows this player's own card as another's, a little to the right
    #[cfg(feature = "dev")]
    if std::env::var("TROPA_ECHO_HELD").is_ok() {
        let echoed = msg.clone().map(|(c, u, at, a)| (c, u, [at[0] + 380.0, at[1]], a));
        status.held.push(Note::CardHeld { from: session.me.wrapping_add(1), card: echoed.as_ref().map(|e| (e.0.clone(), e.1)), at: echoed.as_ref().map_or([0.0; 2], |e| e.2), aim: echoed.and_then(|e| e.3) });
    }
    let _ = &mut status;
    net.node.card_held(msg.as_ref().map(|m| (m.0.clone(), m.1)), msg.as_ref().map_or([0.0; 2], |m| m.2), msg.and_then(|m| m.3));
}

struct Held {
    card: Entity,
    dots: Vec<Entity>,
    /// Which card, to know a new one from it.
    def: (String, bool),
    at: Vec2,
    to: Vec2,
    aim: Option<Vec2>,
    quiet: f32,
    /// Let go: seconds of its flight so far.
    leaving: Option<f32>,
    /// Frames it has been shown: a card just spawned is not in the world yet.
    frames: u32,
}

#[allow(clippy::too_many_arguments)]
fn show_held(
    mut commands: Commands,
    time: Res<Time>,
    mut status: ResMut<NetStatus>,
    mut shown: Local<[Option<Held>; 4]>,
    session: Res<Session>,
    layer: Single<Entity, With<FloatLayer>>,
    mut nodes: Query<(&mut Node, &mut Visibility)>,
    mut looks: Query<&mut UiTransform>,
    db: Res<Db>,
    l10n: Res<L10n>,
    fonts: Res<Fonts>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    let dt = time.delta_secs();
    for note in std::mem::take(&mut status.held) {
        let Note::CardHeld { from, card, at, aim } = note else { continue };
        let Some(slot) = shown.get_mut(from as usize % 4) else { continue };
        let at = Vec2::from_array(at);
        match card {
            None => {
                if let Some(held) = slot {
                    held.leaving = Some(0.0);
                }
            }
            Some(def) => {
                if slot.as_ref().is_some_and(|h| h.def != def) {
                    if let Some(old) = slot.take() {
                        commands.entity(old.card).despawn();
                        old.dots.iter().for_each(|d| commands.entity(*d).despawn());
                    }
                }
                if slot.is_none() {
                    let c = Card { uid: u32::MAX, def: def.0.clone(), upgraded: def.1, free: false, ..Default::default() };
                    let Some(e) = spawn_card(&mut commands, *layer, &c, Form::Human, Some(at), false, (), &db.0, &l10n, &fonts, &art, &assets) else { continue };
                    let color = session.truth.hero(from).map(|h| theme::hero_color(&h.def)).unwrap_or(theme::BONE);
                    commands.entity(e).insert((Pickable::IGNORE, GlobalZIndex(280), Outline::new(px(3), px(0), color), UiTransform::IDENTITY));
                    let dots = (0..DOTS)
                        .map(|i| {
                            let size = 9.0 + 13.0 * i as f32 / (DOTS - 1) as f32;
                            commands
                                .spawn((
                                    Node { position_type: PositionType::Absolute, width: px(size), height: px(size), border_radius: BorderRadius::MAX, ..default() },
                                    BackgroundColor(color),
                                    GlobalZIndex(281),
                                    Visibility::Hidden,
                                    Pickable::IGNORE,
                                    ChildOf(*layer),
                                ))
                                .id()
                        })
                        .collect();
                    *slot = Some(Held { card: e, dots, def, at, to: at, aim: None, quiet: 0.0, leaving: None, frames: 0 });
                }
                if let Some(held) = slot {
                    held.to = at;
                    held.aim = aim.map(Vec2::from_array);
                    held.quiet = 0.0;
                    held.leaving = None;
                }
            }
        }
    }
    for slot in shown.iter_mut() {
        let Some(held) = slot else { continue };
        held.quiet += dt;
        held.frames += 1;
        if held.quiet > QUIET && held.leaving.is_none() {
            held.leaving = Some(0.0);
        }
        // the battle has been left and the card with it
        if nodes.get(held.card).is_err() {
            if held.frames > 2 {
                *slot = None;
            }
            continue;
        }
        let mut gone = false;
        let scale = match &mut held.leaving {
            Some(t) => {
                *t += dt;
                let p = (*t / FLIGHT).min(1.0);
                gone = p >= 1.0;
                if let Some(a) = held.aim {
                    held.to = a - Vec2::new(CARD_W, CARD_H) / 2.0;
                }
                1.0 - p
            }
            None => 1.0,
        };
        held.at = held.at.lerp(held.to, (dt * 18.0).min(1.0));
        if let Ok((mut node, _)) = nodes.get_mut(held.card) {
            node.left = px(held.at.x);
            node.top = px(held.at.y);
        }
        if let Ok(mut tf) = looks.get_mut(held.card) {
            tf.scale = Vec2::splat(scale.max(0.01));
        }
        // the arrow, from the card to where it is aimed
        let from = held.at + Vec2::new(CARD_W / 2.0, 0.0);
        for (i, d) in held.dots.iter().enumerate() {
            if let Ok((mut node, mut vis)) = nodes.get_mut(*d) {
                let want = match (held.aim, held.leaving) {
                    (Some(b), None) => {
                        let t = (i + 1) as f32 / DOTS as f32;
                        let ctrl = Vec2::new(from.x + (b.x - from.x) * 0.15, from.y.min(b.y) - 120.0 - (b.x - from.x).abs() * 0.25);
                        let p = from * (1.0 - t) * (1.0 - t) + ctrl * 2.0 * t * (1.0 - t) + b * t * t;
                        node.left = px(p.x - 8.0);
                        node.top = px(p.y - 8.0);
                        Visibility::Inherited
                    }
                    _ => Visibility::Hidden,
                };
                if *vis != want {
                    *vis = want;
                }
            }
        }
        if gone {
            commands.entity(held.card).despawn();
            held.dots.iter().for_each(|d| commands.entity(*d).despawn());
            *slot = None;
        }
    }
}
