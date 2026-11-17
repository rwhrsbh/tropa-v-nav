//! Gorynych: three foes in the rules (`gorynych_fire`, `gorynych_poison`, `gorynych_thunder`),
//! one dragon on the screen. The body with three bloody neck stumps is drawn once, behind; each
//! head is a unit of its own drawn as its head and neck alone, painted on the same sheet so it
//! sits exactly on its stump. A head struck off flies and leaves its stump; one that grows back
//! is drawn again; when no head is left the body lies down.

use bevy::prelude::*;
use nav_core::Combat;

use super::{stands, stature, UnitLayer};
use crate::art::Art;

/// Where on its picture each head is (as a fraction of the sheet's width), in the order the
/// heads stand from left to right.
const HEADS: [(&str, f32); 3] = [("gorynych_fire", 0.236), ("gorynych_poison", 0.385), ("gorynych_thunder", 0.476)];

fn head(def: &str) -> Option<f32> {
    HEADS.iter().find(|(d, _)| *d == def).map(|(_, x)| *x)
}

/// How tall the dragon is drawn: the sheet is square.
fn height() -> f32 {
    stature::height_of(HEADS[0].0).unwrap_or(380.0)
}

/// The dragon's feet: the place of the first of its heads in the fight.
fn feet(shown: &Combat) -> Option<Vec2> {
    let i = shown.enemies.iter().position(|e| head(&e.def).is_some())?;
    let uid = shown.enemies[i].uid;
    Some(stands::foe(uid, || shown.enemies.iter().map(|e| (e.uid, e.alive)).collect()).unwrap_or_else(|| super::enemy_foot(i, shown.enemies.len())))
}

/// A head's own place on the stage: the dragon's feet moved under that head, so its intent and
/// its health stand over it. `shift` is how far its picture must move back to sit on the body.
pub(super) fn place(shown: &Combat, def: &str) -> Option<Vec2> {
    let x = head(def)?;
    Some(feet(shown)? + Vec2::new(shift_of(x), 0.0))
}

fn shift_of(x: f32) -> f32 {
    (x - 0.5) * height()
}

/// How far a head's picture is drawn left of its own place, to sit on the body.
pub(super) fn picture_shift(def: &str) -> f32 {
    head(def).map_or(0.0, |x| -shift_of(x))
}

/// Which head this is, from the left (0..3): its bar and name stand lower the further right, so
/// three bars side by side stay readable.
pub(super) fn rank(def: &str) -> Option<usize> {
    HEADS.iter().position(|(d, _)| *d == def)
}

/// How far a head's intent stands aside from its own place: the heads are close, their intents
/// would cover one another.
pub(super) fn intent_shift(def: &str) -> f32 {
    rank(def).map_or(0.0, |r| (r as f32 - 1.0) * 70.0)
}

/// A head struck off always flies.
pub(super) fn is_head(stem: &str) -> bool {
    stem.strip_prefix("enemies/").and_then(head).is_some()
}

#[derive(Component)]
pub(super) struct Body {
    lying: bool,
}

/// Draws the body once while any head is in the fight, and lays it down when none is left.
pub(super) fn draw_body(
    mut commands: Commands,
    session: Res<crate::session::Session>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    layer: Option<Single<Entity, With<UnitLayer>>>,
    mut bodies: Query<(Entity, &mut Body, &mut ImageNode, &mut Node)>,
) {
    let shown = &session.shown;
    let Some(foot) = feet(shown) else {
        for (e, ..) in &bodies {
            commands.entity(e).despawn();
        }
        return;
    };
    let Some(layer) = layer else { return };
    let any_alive = shown.enemies.iter().any(|e| e.alive && head(&e.def).is_some());
    let h = height();
    let standing = Node { position_type: PositionType::Absolute, left: px(foot.x - h / 2.0), top: px(foot.y - h), width: px(h), height: px(h), ..default() };
    let lying = Node { position_type: PositionType::Absolute, left: px(foot.x - h * 0.75), top: px(foot.y - h), width: px(h * 1.5), height: px(h), ..default() };
    match bodies.iter_mut().next() {
        None => {
            let (pic, node) = if any_alive { ("enemies/gorynych_body", standing) } else { ("enemies/gorynych_dead_headless", lying) };
            let Some(image) = art.image(&assets, pic) else { return };
            // behind the heads, which stand at the same feet
            commands.spawn((Body { lying: !any_alive }, ImageNode::new(image), node, ZIndex(foot.y as i32 - 1), Pickable::IGNORE, ChildOf(*layer)));
        }
        Some((_, mut body, mut image, mut node)) => {
            if !any_alive && !body.lying {
                if let Some(dead) = art.image(&assets, "enemies/gorynych_dead_headless") {
                    image.image = dead;
                    *node = lying;
                    body.lying = true;
                }
            } else if any_alive && body.lying {
                if let Some(up) = art.image(&assets, "enemies/gorynych_body") {
                    image.image = up;
                    *node = standing;
                    body.lying = false;
                }
            }
        }
    }
}
