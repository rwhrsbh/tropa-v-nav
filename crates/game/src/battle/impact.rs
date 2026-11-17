//! The weight of a blow. Hits shake the screen by trauma: each blow adds to it, and the shake
//! grows with its square, so a scratch barely stirs the view and a great blow throws it about.
//! A hero struck hard flashes the edges of the screen red; a blow taking a third of their health
//! (or 25 at once) slows time for a moment. A great blow stopped dead by block bursts frost-white
//! off the shield, holds the frame for an instant and rings heavily.

use super::*;
use crate::audio::{Mixer, Sfx};

/// Furthest the view is thrown, in stage pixels, and how far it tilts, at full trauma.
const SHAKE_MAX: f32 = 46.0;
const TILT_MAX: f32 = 0.035;
/// Trauma lost per second.
const SETTLE: f32 = 1.5;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Impacts>().init_resource::<HitStop>().add_systems(
        Update,
        (feel, hold_time, flash_edges, burst_rings).chain().after(cue_effects).run_if(in_state(AppState::Battle)).distributive_run_if(resource_exists::<Session>),
    );
}

/// Blows waiting to be felt, put here by `cue_effects`.
#[derive(Resource, Default)]
pub(super) struct Impacts(pub Vec<Impact>);

pub(super) enum Impact {
    /// A unit lost HP; `share` is the part of its whole health.
    Hurt { unit: UnitRef, lost: i32, share: f32 },
    /// A blow stopped by block, all of it or part.
    Blocked { unit: UnitRef, blocked: i32, through: i32 },
    /// Something shook the ground (a boss rising to its second form).
    Quake(f32),
}

/// Time held back after a great blow: how long yet, in real seconds, and how slow.
#[derive(Resource, Default)]
struct HitStop {
    left: f32,
    speed: f32,
}

#[derive(Component)]
struct EdgeFlash {
    t: f32,
    life: f32,
    peak: f32,
}

#[derive(Component)]
struct Ring {
    t: f32,
    at: Vec2,
}

impl Shake {
    pub(super) fn add(&mut self, trauma: f32) {
        self.trauma = (self.trauma + trauma).min(1.0);
    }
}

fn red(alpha: f32) -> BackgroundGradient {
    RadialGradient {
        stops: vec![
            ColorStop::new(Color::NONE, percent(45)),
            ColorStop::new(theme::FRESH_BLOOD.with_alpha(alpha * 0.55), percent(78)),
            ColorStop::new(Color::srgb(0.35, 0.0, 0.0).with_alpha(alpha), percent(100)),
        ],
        shape: RadialGradientShape::FarthestCorner,
        ..default()
    }
    .into()
}

#[allow(clippy::too_many_arguments)]
fn feel(
    mut commands: Commands,
    mut impacts: ResMut<Impacts>,
    mut shake: ResMut<Shake>,
    mut stop: ResMut<HitStop>,
    session: Res<Session>,
    layer: Single<Entity, With<FloatLayer>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    sfx: Res<Sfx>,
    mixer: Res<Mixer>,
) {
    for imp in impacts.0.drain(..) {
        match imp {
            Impact::Hurt { unit, lost, share } => {
                let hero = matches!(unit, UnitRef::Hero(_));
                // a blow on the party is felt harder than one the party deals
                let weight = if hero { 0.18 + share * 2.2 + lost as f32 * 0.012 } else { 0.08 + share * 1.2 + lost as f32 * 0.006 };
                shake.add(weight.min(0.95));
                if hero {
                    let peak = (0.35 + share * 1.6).min(0.85);
                    commands.spawn((
                        EdgeFlash { t: 0.0, life: 0.35 + share * 0.9, peak },
                        Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() },
                        red(peak),
                        GlobalZIndex(250),
                        Pickable::IGNORE,
                        ChildOf(*layer),
                    ));
                    // a third of a life, or a great blow, and time itself staggers
                    if share >= 0.33 || lost >= 25 {
                        stop.left = 0.32;
                        stop.speed = 0.22;
                        sfx.play(&mut commands, "hit_heavy", &mixer, 1.3);
                    }
                }
            }
            Impact::Blocked { unit, blocked, through } => {
                if blocked < 12 || through > 0 {
                    shake.add((blocked as f32 * 0.008).min(0.2));
                    continue;
                }
                // a great blow stopped dead on the shield
                shake.add((0.25 + blocked as f32 * 0.012).min(0.7));
                stop.left = stop.left.max(0.09);
                stop.speed = stop.speed.min(0.05).max(0.0);
                if stop.speed == 0.0 {
                    stop.speed = 0.05;
                }
                sfx.play(&mut commands, "block", &mixer, 1.5);
                sfx.play(&mut commands, "hit_heavy", &mixer, 0.6);
                if let Some((foot, sz)) = unit_place(&session.shown, unit) {
                    let at = Vec2::new(foot.x, foot.y - sz.y * 0.5);
                    commands.spawn((
                        Ring { t: 0.0, at },
                        Node { position_type: PositionType::Absolute, border: UiRect::all(px(10)), border_radius: BorderRadius::MAX, ..default() },
                        BorderColor::all(theme::FROST),
                        BackgroundColor(theme::FROST.with_alpha(0.12)),
                        GlobalZIndex(240),
                        Pickable::IGNORE,
                        ChildOf(*layer),
                    ));
                    commands.spawn((
                        Floater { ttl: 1.2, life: 1.2 },
                        theme::text(l10n.t("battle-blocked"), &fonts.title, 64.0, theme::FROST),
                        TextShadow { offset: Vec2::splat(3.0), color: Color::BLACK },
                        Node { position_type: PositionType::Absolute, left: px(at.x - 120.0), top: px(at.y - sz.y * 0.45), ..default() },
                        GlobalZIndex(241),
                        ChildOf(*layer),
                    ));
                }
            }
            Impact::Quake(t) => shake.add(t),
        }
    }
}

/// Holds time slow after a great blow, then lets it run again over a moment.
fn hold_time(real: Res<Time<Real>>, mut virt: ResMut<Time<Virtual>>, mut stop: ResMut<HitStop>) {
    let dt = real.delta_secs();
    if stop.left > 0.0 {
        stop.left -= dt;
        virt.set_relative_speed(stop.speed.max(0.02));
    } else {
        let now = virt.relative_speed();
        if now < 1.0 {
            virt.set_relative_speed((now + dt * 4.0).min(1.0));
        }
    }
}

fn flash_edges(mut commands: Commands, real: Res<Time<Real>>, mut q: Query<(Entity, &mut EdgeFlash, &mut BackgroundGradient)>) {
    for (e, mut f, mut g) in &mut q {
        f.t += real.delta_secs();
        let left = 1.0 - f.t / f.life;
        if left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        // in at once, out slowly
        *g = red(f.peak * left * left);
    }
}

fn burst_rings(mut commands: Commands, real: Res<Time<Real>>, mut q: Query<(Entity, &mut Ring, &mut Node, &mut BorderColor, &mut BackgroundColor)>) {
    for (e, mut r, mut node, mut edge, mut bg) in &mut q {
        r.t += real.delta_secs() / 0.45;
        if r.t >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let ease = 1.0 - (1.0 - r.t).powi(3);
        let size = 120.0 + 380.0 * ease;
        node.left = px(r.at.x - size / 2.0);
        node.top = px(r.at.y - size / 2.0);
        node.width = px(size);
        node.height = px(size);
        node.border = UiRect::all(px(14.0 * (1.0 - r.t) + 2.0));
        *edge = BorderColor::all(theme::FROST.with_alpha(1.0 - r.t));
        bg.0 = theme::FROST.with_alpha(0.18 * (1.0 - r.t));
    }
}

/// Throws the view about by the trauma left, in real time so a held moment still shakes.
pub(super) fn shake_screen(real: Res<Time<Real>>, comfort: Res<Comfort>, mut shake: ResMut<Shake>, mut root: Single<&mut UiTransform, With<BattleRoot>>) {
    if shake.trauma <= 0.0 {
        if root.translation != Val2::ZERO || root.rotation != Rot2::IDENTITY {
            root.translation = Val2::ZERO;
            root.rotation = Rot2::IDENTITY;
        }
        return;
    }
    let dt = real.delta_secs();
    shake.t += dt;
    let k = shake.trauma * shake.trauma * if comfort.less_shake { 0.25 } else { 1.0 };
    let t = shake.t;
    // three detuned waves stand in for noise: never the same twice, never a plain wobble
    let n = |a: f32, b: f32, c: f32| ((t * a).sin() + (t * b + 1.3).sin() * 0.6 + (t * c + 2.1).sin() * 0.35) / 1.95;
    root.translation = Val2::px(n(67.0, 113.0, 191.0) * SHAKE_MAX * k, n(59.0, 127.0, 173.0) * SHAKE_MAX * 0.7 * k);
    root.rotation = Rot2::radians(n(43.0, 89.0, 151.0) * TILT_MAX * k);
    shake.trauma = (shake.trauma - SETTLE * dt).max(0.0);
}
