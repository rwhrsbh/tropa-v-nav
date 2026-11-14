//! What a killing blow leaves. A foe cut into parts (see `rig`) loses its head, and on a heavy
//! blow an arm too: each flies off, falls, bounces and rolls along the ground, bleeding as it
//! goes, while the neck and the stump spurt and then seep. The body falls over and stays where it
//! fell until the fight is over. Every drop that reaches the ground stays there as a stain, the
//! more of them the harder a thing came down; heroes bleed bright, the dead and the unclean bleed
//! rot, sap, ichor or dust.

use super::*;
use std::f32::consts::FRAC_PI_2;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Blows>()
        .init_resource::<Severed>()
        .add_systems(Update, (play_frames, trickle_down.run_if(resource_exists::<Session>), (fly_severed, hide_lost_limbs).chain().run_if(resource_exists::<Session>)).run_if(in_state(AppState::Battle)))
        .add_systems(Startup, |mut commands: Commands, assets: Res<AssetServer>| commands.insert_resource(BloodArt::load(&assets))).add_systems(OnEnter(AppState::Battle), |mut b: ResMut<Blows>| b.0.clear()).add_systems(Update, (fall, fly_gibs, bleed).chain().after(dying).run_if(in_state(AppState::Battle)));
}

/// The last blow each unit took: its weight (0..1) and the HP it cost.
#[derive(Resource, Default)]
pub(super) struct Blows(pub std::collections::HashMap<UnitRef, (f32, u32)>);

/// Stains on the ground, under the units.
#[derive(Component)]
pub(super) struct StainLayer;

/// A body that has fallen: which way, how far through its fall, where its feet are.
#[derive(Component)]
pub(super) struct Corpse {
    /// Who it was, should they get up again.
    pub unit: UnitRef,
    t: f32,
    side: f32,
    /// The widget's feet, from its top-left corner.
    feet: Vec2,
    /// Stage point of those feet.
    ground: Vec2,
    pub angle: f32,
    /// The picture of the body lying dead, which the standing one gives way to (`None` when
    /// something was struck off, since that picture is whole).
    lying: Option<Entity>,
    /// How far a body going over whole is held up, so what stuck out behind it stays on the ground.
    lift: f32,
    /// What runs in its veins.
    blood: Color,
}

impl Corpse {
    /// Takes the body away, the picture of it lying included.
    pub fn clear_away(&self, commands: &mut Commands, body: Entity) {
        if let Some(lying) = self.lying {
            commands.entity(lying).despawn();
        }
        commands.entity(body).despawn();
    }
}

/// The body of one who lost their head in an earlier fight, laid where they would stand.
#[derive(Component)]
pub(super) struct LaidOut(u8);

/// Who has lost their head comes along as a body: it lies on the ground of every fight after.
pub(super) fn lay_out_the_headless(
    mut commands: Commands,
    session: Res<Session>,
    db: Res<crate::session::Db>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    layer: Option<Single<Entity, With<CorpseLayer>>>,
    units: Query<&UnitWidget>,
    corpses: Query<&Corpse>,
    laid: Query<&LaidOut>,
) {
    let Some(layer) = layer else { return };
    for h in session.shown.heroes.iter().filter(|h| h.beheaded) {
        let unit = UnitRef::Hero(h.slot);
        // still falling, or fallen in this very fight
        if units.iter().any(|w| w.unit == unit) || corpses.iter().any(|c| c.unit == unit) || laid.iter().any(|l| l.0 == h.slot) {
            continue;
        }
        let Some((foot, size)) = unit_place(&session.shown, unit) else { continue };
        let stem = format!("heroes/{}", h.def);
        let Some(picture) = art.image(&assets, &format!("{stem}_dead_headless")) else { continue };
        let draw = hero_draw(&stem, size) * super::hp_stature(&db.0, &h.def, h.max_hp);
        commands.spawn((
            LaidOut(h.slot),
            ImageNode::new(picture),
            Node {
                position_type: PositionType::Absolute,
                left: px(foot.x - draw.y * 0.75),
                top: px(foot.y - draw.y),
                width: px(draw.y * 1.5),
                height: px(draw.y),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(*layer),
        ));
    }
}

/// The bodies lying dead, over the stains and under the living.
#[derive(Component)]
pub(super) struct CorpseLayer;

/// Seconds after the blow when the falling body gives way to the one lying dead, and how long
/// that takes.
pub(super) const SWAP_AT: f32 = 0.55;
pub(super) const SWAP_FOR: f32 = 0.06;

/// A part struck off: flying, bouncing, then rolling, dripping all the while.
#[derive(Component)]
struct Gib {
    at: Vec2,
    vel: Vec2,
    angle: f32,
    spin: f32,
    ground: f32,
    radius: f32,
    weight: f32,
    blood: Color,
    /// Seconds it still bleeds.
    bleed: f32,
    /// Ground covered since the last smear.
    smear: f32,
    still: bool,
}

/// Blood coming out of a body: a spurt that pulses, weakening, and then a seep.
#[derive(Component)]
struct Bleeder {
    /// The body it comes from, and the point on that body (from its widget's top-left).
    corpse: Entity,
    local: Vec2,
    dir: Vec2,
    color: Color,
    t: f32,
    life: f32,
    due: f32,
    strength: f32,
}

/// The colour of what runs in `unit`'s veins.
pub(super) fn blood_of(comfort: &Comfort, shown: &Combat, unit: UnitRef) -> Color {
    if comfort.ink {
        return Color::srgb(0.03, 0.03, 0.05);
    }
    let UnitRef::Enemy(uid) = unit else { return Color::srgb(0.80, 0.03, 0.02) };
    let def = shown.enemy(uid).map(|e| e.def.as_str()).unwrap_or("");
    match def {
        // the drowned and the ghosts: a pale cold ichor
        "nav_soul" | "weeper" | "mara" | "drowned_maiden" | "mavka" | "koschei_shadow" | "swamp_light" | "sirin" | "vodyanoy" | "nochnitsa" | "polunochnitsa" | "morovitsa" | "sea_guard" | "vodyanoy_tsar" => Color::srgb(0.50, 0.60, 0.64),
        // wood and fungus bleed sap
        "leshy" | "tree_guard" | "death_oak" | "death_oak_chest" | "mushroom_corpse" | "lesavka" | "chuchelo" | "mokhovik" => Color::srgb(0.42, 0.40, 0.10),
        // things, not bodies: splinters and ash, meltwater, a dying glow
        "death_chest" | "death_egg" | "death_egg_solo" => Color::srgb(0.30, 0.25, 0.18),
        "ice_crown" | "korochun" => Color::srgb(0.62, 0.74, 0.80),
        // bone has only dust in it
        "kostomakha" | "koschei" | "ovinnik" | "viy" => Color::srgb(0.42, 0.36, 0.26),
        // the swamp's things run green-black
        "bolotnik" | "kikimora" | "tinyak" | "shishiga" | "yrka" => Color::srgb(0.26, 0.32, 0.08),
        // old dead blood, black and thick
        "koschei_guard" | "black_rider" | "vurdalak_elder" | "volot" | "volot_giant" | "gorynych" | "gorynych_fire" | "gorynych_poison" | "gorynych_thunder" | "gagana" => Color::srgb(0.30, 0.06, 0.04),
        // warm brigand/beast blood
        "solovey" | "kot_bayun" => Color::srgb(0.68, 0.08, 0.05),
        // the rest rot: a dark brown-red
        _ => Color::srgb(0.52, 0.09, 0.04),
    }
}

/// Photographed blood (Flow), painted grey so it takes any colour: stains, splashes and drops.
#[derive(Resource, Clone)]
pub(super) struct BloodArt {
    pub drops: Vec<Handle<Image>>,
    splashes: Vec<Handle<Image>>,
    /// Large stains: a splat, a pool, old dried blood.
    big: Vec<Handle<Image>>,
    pub smear: Handle<Image>,
    /// Photographed dust (Flow): wisps and low clouds.
    pub dust: Vec<Handle<Image>>,
    /// Frame by frame (Flow sprite sheets): a jet out of a wound, a splash off a blow, a pool
    /// spreading on the ground.
    pub spurt: Vec<Handle<Image>>,
    pub splash: Vec<Handle<Image>>,
    pub pool: Vec<Handle<Image>>,
    /// A heavy blow bursting out in all directions, a mist left by a killing one, a trickle down
    /// the badly hurt.
    pub gush: Vec<Handle<Image>>,
    pub mist: Vec<Handle<Image>>,
    pub trickle: Vec<Handle<Image>>,
}

/// Limbs struck off this frame, to be sent flying.
#[derive(Resource, Default)]
pub(super) struct Severed(pub Vec<(UnitRef, rig::Part)>);

/// A limb struck off a hero who is still standing goes flying, dripping, like any other part.
#[allow(clippy::too_many_arguments)]
fn fly_severed(
    mut commands: Commands,
    mut severed: ResMut<Severed>,
    session: Res<Session>,
    comfort: Res<Comfort>,
    images: Res<Assets<Image>>,
    rigs: Res<Assets<rig::RigMaterial>>,
    layer: Single<Entity, With<FloatLayer>>,
    art: Res<BloodArt>,
    sprites: Query<(&UnitSprite, &Children)>,
    skins: Query<(&rig::RigSkin, &MaterialNode<rig::RigMaterial>)>,
) {
    for (unit, part) in severed.0.drain(..) {
        let Some((foot, _)) = unit_place(&session.shown, unit) else { continue };
        let Some((sprite, children)) = sprites.iter().find(|(s, _)| s.unit == unit) else { continue };
        let blood = blood_of(&comfort, &session.shown, unit);
        let draw = sprite.draw;
        let pic = Vec2::new(foot.x - draw.x / 2.0, foot.y - draw.y);
        let at_body = foot - Vec2::new(0.0, draw.y * 0.5);
        play(&mut commands, *layer, &art.gush, at_body, Vec2::splat(draw.y * 0.8), Vec2::splat(0.5), 0.0, blood, 20.0, false, false);
        for (skin, material) in skins.iter_many(children).flatten() {
            if skin.part != Some(part) {
                continue;
            }
            let Some(m) = rigs.get(&material.0) else { continue };
            let picture = m.picture.clone();
            let (centre, radius) = centre_of(&images, &picture).unwrap_or((Vec2::splat(0.5), 0.08));
            let at = pic + centre * draw;
            commands
                .spawn((
                    Gib { at, vel: Vec2::new(-260.0, -820.0), angle: 0.0, spin: -9.0, ground: foot.y + 30.0, radius: radius * draw.y, weight: 0.8, blood, bleed: 3.5, smear: 0.0, still: false },
                    Node { position_type: PositionType::Absolute, left: px(at.x), top: px(at.y), width: px(0), height: px(0), ..default() },
                    UiTransform::IDENTITY,
                    GlobalZIndex(15),
                    Pickable::IGNORE,
                    ChildOf(*layer),
                ))
                .with_child((
                    ImageNode::new(picture),
                    Node { position_type: PositionType::Absolute, left: px(-centre.x * draw.x), top: px(-centre.y * draw.y), width: px(draw.x), height: px(draw.y), ..default() },
                    Pickable::IGNORE,
                ));
        }
    }
}

/// What has been struck off stays off, for the rest of the run: the first limb is the striking
/// arm, the second the other.
fn hide_lost_limbs(session: Res<Session>, sprites: Query<(&UnitSprite, &Children)>, mut skins: Query<(&rig::RigSkin, &mut Visibility)>) {
    for (sprite, children) in &sprites {
        let UnitRef::Hero(i) = sprite.unit else { continue };
        let arms = session.shown.hero(i).map_or(0, |h| h.lost_arms);
        let no_head = session.shown.hero(i).is_some_and(|h| h.walks_headless);
        if arms == 0 && !no_head {
            continue;
        }
        let mut iter = skins.iter_many_mut(children);
        while let Some(Ok((skin, mut vis))) = iter.fetch_next() {
            let gone = match skin.part {
                Some(rig::Part::Head) => no_head,
                Some(rig::Part::Arm) => arms & nav_core::combat::WEAPON_ARM != 0,
                Some(rig::Part::Shield) => arms & nav_core::combat::SHIELD_ARM != 0,
                None => false,
            };
            if gone {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

/// Whoever carries the bleeding status shows it: now and then a trickle runs down from a wound.
/// Low health alone draws none, or blood would promise a status that is not there.
fn trickle_down(
    mut commands: Commands,
    time: Res<Time>,
    session: Res<Session>,
    comfort: Res<Comfort>,
    art: Res<BloodArt>,
    layer: Single<Entity, With<FloatLayer>>,
    units: Query<&UnitWidget, Without<Corpse>>,
    mut clock: Local<(f32, u32)>,
) {
    clock.0 -= time.delta_secs();
    if clock.0 > 0.0 {
        return;
    }
    clock.1 = clock.1.wrapping_add(1);
    // one trickle at a time over the whole field, so it stays a detail
    clock.0 = 0.7 + (clock.1.wrapping_mul(2654435761) >> 28) as f32 * 0.09;
    let hurt: Vec<UnitRef> = units
        .iter()
        .map(|w| w.unit)
        .filter(|u| match u {
            UnitRef::Hero(i) => session.shown.hero(*i).is_some_and(|h| !h.downed && h.statuses.get(&StatusKind::Bleed).is_some_and(|n| *n > 0)),
            UnitRef::Enemy(e) => session.shown.enemy(*e).is_some_and(|e| e.alive && e.statuses.get(&StatusKind::Bleed).is_some_and(|n| *n > 0)),
        })
        .collect();
    if hurt.is_empty() {
        return;
    }
    let unit = hurt[clock.1 as usize % hurt.len()];
    let Some((foot, size)) = unit_place(&session.shown, unit) else { return };
    let side = ((clock.1.wrapping_mul(40503) >> 8) % 100) as f32 / 100.0 - 0.5;
    let h = size.y * 0.3;
    let blood = blood_of(&comfort, &session.shown, unit);
    play(&mut commands, *layer, &art.trickle, foot - Vec2::new(size.x * 0.35 * side, size.y * 0.62), Vec2::new(h * 0.6, h), Vec2::new(0.5, 0.0), 0.0, blood, 7.0, false, false);
}

/// A blood effect played frame by frame; a pool keeps its last frame, the rest are gone after.
#[derive(Component)]
pub(super) struct Frames {
    frames: Vec<Handle<Image>>,
    t: f32,
    fps: f32,
    keep: bool,
}

/// Spawns a frame-by-frame effect, `size` stage pixels, its anchor (0..1 of the frame, e.g. the
/// bottom middle for a jet) at `at`, turned by `angle`, under `parent`.
#[allow(clippy::too_many_arguments)]
pub(super) fn play(commands: &mut Commands, parent: Entity, frames: &[Handle<Image>], at: Vec2, size: Vec2, anchor: Vec2, angle: f32, color: Color, fps: f32, keep: bool, flip: bool) {
    let Some(first) = frames.first() else { return };
    commands.spawn((
        Frames { frames: frames.to_vec(), t: 0.0, fps, keep },
        ImageNode { color, flip_x: flip, ..ImageNode::new(first.clone()) },
        Node { position_type: PositionType::Absolute, left: px(at.x - size.x * anchor.x), top: px(at.y - size.y * anchor.y), width: px(size.x), height: px(size.y), ..default() },
        UiTransform { rotation: Rot2::radians(angle), ..default() },
        GlobalZIndex(15),
        Pickable::IGNORE,
        ChildOf(parent),
    ));
}

fn play_frames(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Frames, &mut ImageNode)>) {
    for (e, mut f, mut image) in &mut q {
        f.t += time.delta_secs();
        let k = (f.t * f.fps) as usize;
        if k >= f.frames.len() {
            if f.keep {
                commands.entity(e).remove::<Frames>();
            } else {
                commands.entity(e).despawn();
            }
            continue;
        }
        if image.image != f.frames[k] {
            image.image = f.frames[k].clone();
        }
    }
}

impl BloodArt {
    fn load(assets: &AssetServer) -> BloodArt {
        let many = |stem: &str, n: usize| (0..n).map(|i| assets.load(crate::art::path(&format!("blood/{stem}_{i}.png")))).collect();
        BloodArt {
            drops: many("drop", 8),
            splashes: many("splash", 7),
            big: ["splat_a", "pool", "dried", "spray", "smear", "smear"].iter().map(|s| assets.load(crate::art::path(&format!("blood/{s}.png")))).collect(),
            smear: assets.load(crate::art::path("blood/smear.png")),
            spurt: (0..8).map(|i| assets.load(crate::art::path(&format!("blood/anim/spurt_{i}.png")))).collect(),
            splash: (0..8).map(|i| assets.load(crate::art::path(&format!("blood/anim/splash_{i}.png")))).collect(),
            pool: (0..8).map(|i| assets.load(crate::art::path(&format!("blood/anim/pool_{i}.png")))).collect(),
            gush: (0..8).map(|i| assets.load(crate::art::path(&format!("blood/anim/gush_{i}.png")))).collect(),
            mist: (0..8).map(|i| assets.load(crate::art::path(&format!("blood/anim/mist_{i}.png")))).collect(),
            trickle: (0..8).map(|i| assets.load(crate::art::path(&format!("blood/anim/trickle_{i}.png")))).collect(),
            dust: (0..6).map(|i| assets.load(crate::art::path(&format!("fx/wisp_{i}.png")))).chain(["fx/dust_low.png", "fx/dust_drift.png"].map(|p| assets.load(crate::art::path(p)))).collect(),
        }
    }
}

/// A stain where a drop of `size` landed at `at`, in `color`: a small drop leaves a drip, a
/// heavy one a splash, a great deal a pool; all lie flat on the ground, turned every which way.
pub(super) fn stain(commands: &mut Commands, art: &BloodArt, layer: Entity, at: Vec2, size: f32, color: Color, seed: u32) {
    let r = |i: u32| fx::scatter(seed, i);
    let pick = |v: &Vec<Handle<Image>>| v[(r(5) * v.len() as f32) as usize % v.len().max(1)].clone();
    let (picture, grow) = if size > 20.0 { (pick(&art.big), 2.8) } else if size > 9.0 { (pick(&art.splashes), 3.0) } else { (pick(&art.drops), 2.2) };
    let w = size * grow * (0.8 + 0.4 * r(1));
    // seen at a slant: the ground is flattened
    let h = w * (0.38 + 0.1 * r(2));
    commands.spawn((
        ImageNode { color: color.mix(&Color::BLACK, 0.1 * r(3)).with_alpha(0.85 + 0.15 * r(4)), flip_x: r(6) < 0.5, flip_y: r(7) < 0.5, ..ImageNode::new(picture) },
        Node { position_type: PositionType::Absolute, left: px(at.x - w / 2.0), top: px(at.y - h / 2.0), width: px(w), height: px(h), ..default() },
        Pickable::IGNORE,
        ChildOf(layer),
    ));
}

/// The parts of a foe just killed fly off, and its body begins to fall.
#[allow(clippy::too_many_arguments)]
pub(super) fn strike_down(
    commands: &mut Commands,
    widget: Entity,
    unit: UnitRef,
    origin: Vec2,
    foot: Vec2,
    draw: Vec2,
    hero: bool,
    blood: Color,
    parts: Vec<(rig::Part, Vec2, Handle<Image>, Entity)>,
    images: &Assets<Image>,
    float_layer: Entity,
    corpse_layer: Entity,
    stain_layer: Entity,
    art: &BloodArt,
    lying: Option<Handle<Image>>,
    lying_headless: Option<Handle<Image>>,
    standing: Option<Handle<Image>>,
    seed: u32,
    weight: f32,
) -> bool {
    let r = |i: u32| fx::scatter(seed, i);
    // a foe falls back, away from the heroes; a hero the other way
    let side = if hero { -1.0 } else { 1.0 };
    // the picture's top-left on the stage, and the widget's
    let pic = Vec2::new(foot.x - draw.x / 2.0, foot.y - draw.y);
    // only a heavy killing blow strikes anything off, and each part takes its own chance: a
    // light one only fells the body, a crushing one may take every part at once
    let mut flying: Vec<bool> = parts
        .iter()
        .enumerate()
        .map(|(k, (part, ..))| {
            // a hero falls only to a blow that took the head, and only the head
            if hero {
                return *part == rig::Part::Head;
            }
            let chance = match part {
                rig::Part::Head => weight * 1.3 - 0.2,
                rig::Part::Arm => weight * 1.1 - 0.25,
                rig::Part::Shield => weight * 0.9 - 0.3,
            };
            r(70 + k as u32) < chance
        })
        .collect();
    // a body still whole crumples and gives way to its picture lying dead, a beheaded one to its
    // picture lying dead without a head, and that picture has its arms, so only the head flies;
    // one cut up otherwise goes over as it is
    let head_off = parts.iter().zip(&flying).any(|((p, ..), f)| *f && *p == rig::Part::Head);
    if head_off && lying_headless.is_some() {
        for (f, (p, ..)) in flying.iter_mut().zip(&parts) {
            *f = *f && *p == rig::Part::Head;
        }
    }
    let others_off = parts.iter().zip(&flying).any(|((p, ..), f)| *f && *p != rig::Part::Head);
    let lying = match (head_off, others_off) {
        (false, false) => lying,
        (true, false) => lying_headless,
        _ => None,
    };
    let lying = lying.map(|picture| {
        commands
            .spawn((
                ImageNode { color: Color::WHITE.with_alpha(0.0), ..ImageNode::new(picture) },
                Node {
                    position_type: PositionType::Absolute,
                    left: px(foot.x - draw.y * 0.75),
                    top: px(foot.y - draw.y),
                    width: px(draw.y * 1.5),
                    height: px(draw.y),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(corpse_layer),
            ))
            .id()
    });
    let swaps = lying.is_some();
    // it turns about its feet, so all it holds out on that side would swing under the ground
    let lift = if swaps { 0.0 } else { standing.and_then(|p| reach_past_feet(images, &p, side)).unwrap_or(0.3) * draw.x * 0.85 };
    // what was in the air when it fell hangs there a moment
    play(commands, float_layer, &art.mist, foot - Vec2::new(0.0, draw.y * 0.55), Vec2::splat(draw.y * 0.9), Vec2::splat(0.5), 0.0, blood.with_alpha(0.55), 9.0, false, side < 0.0);
    // blood spreads out from under the body as it lies, the more if it was cut to pieces
    let wide = draw.y * if flying.contains(&true) { 1.3 } else { 0.9 };
    play(commands, stain_layer, &art.pool, foot + Vec2::new(side * draw.y * 0.25, 8.0), Vec2::new(wide, wide * 0.36), Vec2::splat(0.5), 0.0, blood.with_alpha(0.9), 3.0, true, side < 0.0);
    // the dead lie behind the living
    commands.entity(widget).insert((Corpse { unit, t: 0.0, side, feet: foot - origin, ground: foot, angle: 0.0, lying, lift, blood }, ZIndex(0)));
    for (k, (part, joint, picture, skin)) in parts.into_iter().enumerate() {
        if !flying[k] {
            continue;
        }
        let k = k as u32;
        commands.entity(skin).insert(Visibility::Hidden);
        let (centre, radius) = centre_of(images, &picture).unwrap_or((joint - Vec2::new(0.0, 0.06), 0.08));
        let at = pic + centre * draw;
        let weight = match part {
            rig::Part::Head => 1.0,
            rig::Part::Arm => 0.7,
            rig::Part::Shield => 0.85,
        };
        let up = if part == rig::Part::Head { 900.0 } else { 650.0 };
        // up and back over the body, now and then forward over the heroes' heads
        let back = if r(60 + k) < 0.75 { side } else { -side };
        let vel = Vec2::new(back * (110.0 + 200.0 * r(10 + k)), -(up + 300.0 * r(20 + k)));
        commands
            .spawn((
                Gib {
                    at,
                    vel,
                    angle: 0.0,
                    spin: side * (5.0 + 8.0 * r(30 + k)),
                    // on the earth in front of the units, not on their bars
                    ground: foot.y + 2.0 + 16.0 * r(40 + k),
                    radius: radius * draw.y,
                    weight,
                    blood,
                    bleed: 3.5,
                    smear: 0.0,
                    still: false,
                },
                Node { position_type: PositionType::Absolute, left: px(at.x), top: px(at.y), width: px(0), height: px(0), ..default() },
                UiTransform::IDENTITY,
                // over the units, under the hand of cards
                GlobalZIndex(15),
                Pickable::IGNORE,
                ChildOf(float_layer),
            ))
            .with_child((
                ImageNode::new(picture),
                Node { position_type: PositionType::Absolute, left: px(-centre.x * draw.x), top: px(-centre.y * draw.y), width: px(draw.x), height: px(draw.y), ..default() },
                Pickable::IGNORE,
            ));
        // and where it was cut from, the body spurts
        let wound = pic + joint * draw - origin;
        let dir = if part == rig::Part::Head { Vec2::new(side * 0.25, -1.0) } else { Vec2::new(-side * 0.6, -0.5) };
        commands.spawn(Bleeder { corpse: widget, local: wound, dir: dir.normalize(), color: blood, t: 0.0, life: 4.5, due: 0.0, strength: weight });
    }
    swaps
}

/// How far (0..0.5 of the width) a picture's paint reaches past its middle, where the feet stand,
/// on one side.
fn reach_past_feet(images: &Assets<Image>, picture: &Handle<Image>, side: f32) -> Option<f32> {
    let image = images.get(picture)?;
    let data = image.data.as_ref()?;
    let (w, h) = (image.width() as usize, image.height() as usize);
    if data.len() < w * h * 4 {
        return None;
    }
    let mut far = 0usize;
    for y in (0..h).step_by(4) {
        for x in (0..w).step_by(2) {
            if data[(y * w + x) * 4 + 3] > 128 {
                let past = if side > 0.0 { x.saturating_sub(w / 2) } else { (w / 2).saturating_sub(x) };
                far = far.max(past);
            }
        }
    }
    Some(far as f32 / w as f32)
}

/// Where a part's paint is centred on its picture (0..1), and how far it reaches.
fn centre_of(images: &Assets<Image>, picture: &Handle<Image>) -> Option<(Vec2, f32)> {
    let image = images.get(picture)?;
    let data = image.data.as_ref()?;
    let (w, h) = (image.width() as usize, image.height() as usize);
    if data.len() < w * h * 4 {
        return None;
    }
    let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in (0..h).step_by(4) {
        for x in (0..w).step_by(4) {
            if data[(y * w + x) * 4 + 3] > 128 {
                sx += x as f64;
                sy += y as f64;
                n += 1.0;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (n > 0.0).then(|| {
        let reach = ((x1 - x0) as f32 / w as f32).max((y1 - y0) as f32 / h as f32) * 0.5;
        (Vec2::new((sx / n) as f32 / w as f32, (sy / n) as f32 / h as f32), reach.min(0.2))
    })
}

/// A body topples about its feet, slowly and then all at once, and settles with a bump.
fn fall(
    time: Res<Time>,
    mut q: Query<(&mut Corpse, &mut UiTransform, &ComputedNode)>,
    mut lying: Query<(&mut ImageNode, &mut UiTransform), Without<Corpse>>,
    mut bursts: ResMut<fx::Bursts>,
) {
    let dt = time.delta_secs();
    for (mut c, mut tf, node) in &mut q {
        let before = c.t;
        c.t += dt;
        // the body hits the ground as one picture gives way to the other: a burst of blood hides
        // the change
        if c.lying.is_some() && before < SWAP_AT && c.t >= SWAP_AT {
            // the blood still in it is thrown out as it hits
            bursts.0.push(fx::Burst::Spurt { at: c.ground - Vec2::new(0.0, 25.0), dir: Vec2::new(-c.side * 0.7, -0.7).normalize(), force: 0.8, blood: c.blood, ground: c.ground.y });
            bursts.0.push(fx::Burst::Spurt { at: c.ground - Vec2::new(0.0, 20.0), dir: Vec2::new(c.side * 0.4, -1.0).normalize(), force: 0.9, blood: c.blood, ground: c.ground.y });
        }
        let p = ((c.t - 0.15) / 0.75).clamp(0.0, 1.0);
        // gravity: the fall gathers speed; at the end it hits and rocks a little
        let ease = p * p;
        let bump = if p >= 1.0 { 0.06 * (-(c.t - 0.9) * 7.0).exp() * ((c.t - 0.9) * 18.0).sin() } else { 0.0 };
        // a body that will lie as painted only sags back as its knees go, then gives way
        let reach = if c.lying.is_some() { 0.32 } else { FRAC_PI_2 * 0.93 };
        c.angle = c.side * (reach * ease - bump);
        if let Some((mut image, mut ltf)) = c.lying.and_then(|e| lying.get_mut(e).ok()) {
            let k = ((c.t - SWAP_AT) / SWAP_FOR).clamp(0.0, 1.0);
            image.color = Color::WHITE.with_alpha(k);
            // it lands: squashed a moment and settling
            let land = (1.0 - k) * 0.12 + 0.05 * (-(c.t - SWAP_AT - SWAP_FOR).max(0.0) * 9.0).exp() * (k * 9.0).sin();
            ltf.scale = Vec2::new(1.0 + land, 1.0 - land);
        }
        let size = node.size() * node.inverse_scale_factor();
        // a transform turns about the middle: move the middle so the feet stay put
        let arm = size / 2.0 - c.feet;
        let turned = Vec2::from_angle(c.angle).rotate(arm);
        tf.rotation = Rot2::radians(c.angle);
        tf.translation = Val2::px(turned.x - arm.x, turned.y - arm.y - c.lift * ease);
    }
}

fn fly_gibs(
    mut commands: Commands,
    time: Res<Time>,
    stains: Option<Single<Entity, With<StainLayer>>>,
    mut gibs: Query<(&mut Gib, &mut Node, &mut UiTransform)>,
    mut seed: Local<u32>,
    art: Res<BloodArt>,
) {
    let dt = time.delta_secs().min(0.05);
    let Some(stains) = stains.map(|s| *s) else { return };
    for (mut g, mut node, mut tf) in &mut gibs {
        *seed = seed.wrapping_add(1);
        let r = |i: u32| fx::scatter(*seed, i);
        if !g.still {
            g.vel.y += 2400.0 * dt;
            let step = g.vel * dt;
            g.at += step;
            g.angle += g.spin * dt;
            let floor = g.ground - g.radius * 0.55;
            if g.at.y >= floor {
                g.at.y = floor;
                if g.vel.y > 140.0 {
                    // a hit on the ground: the harder and heavier, the more it splashes
                    let hit = g.vel.y * g.weight;
                    let n = (hit / 90.0).clamp(2.0, 18.0) as u32;
                    for i in 0..n {
                        let off = Vec2::new((r(i * 2) - 0.5) * g.radius * 3.0 * (hit / 800.0).min(2.0), (r(i * 2 + 1) - 0.5) * 18.0);
                        stain(&mut commands, &art, stains, Vec2::new(g.at.x, g.ground) + off, 6.0 + 16.0 * r(i + 50) * (hit / 900.0).min(1.5), g.blood, *seed + i);
                    }
                    g.vel.y = -g.vel.y * 0.32;
                    g.vel.x *= 0.72;
                    g.spin = g.vel.x / g.radius.max(1.0);
                } else {
                    // rolling: it turns as it goes, slowing in the dirt
                    g.vel.y = 0.0;
                    g.vel.x *= (1.0 - 2.2 * dt).max(0.0);
                    g.spin = g.vel.x / g.radius.max(1.0);
                    g.smear += step.x.abs();
                    if g.bleed > 0.0 && g.smear > 16.0 {
                        g.smear = 0.0;
                        // a smear along the way it rolls
                        let w = 70.0 + 50.0 * g.weight * (g.bleed / 3.5);
                        let h = w * 0.3;
                        commands.spawn((
                            ImageNode { color: g.blood.mix(&Color::BLACK, 0.2).with_alpha(0.8), flip_x: g.vel.x < 0.0, ..ImageNode::new(art.smear.clone()) },
                            Node { position_type: PositionType::Absolute, left: px(g.at.x - w / 2.0), top: px(g.ground + (r(7) - 0.5) * 6.0 - h / 2.0), width: px(w), height: px(h), ..default() },
                            Pickable::IGNORE,
                            ChildOf(stains),
                        ));
                    }
                    if g.vel.x.abs() < 6.0 {
                        g.still = true;
                    }
                }
            }
            // the edges of the clearing throw it back
            let (lo, hi) = (60.0, STAGE_W - 60.0);
            if (g.at.x < lo && g.vel.x < 0.0) || (g.at.x > hi && g.vel.x > 0.0) {
                g.vel.x = -g.vel.x * 0.5;
                g.spin = -g.spin * 0.5;
            }
            // come to rest, a head lies on its side or face, an arm along the ground
            if g.still {
                let lie = if g.weight < 0.9 { std::f32::consts::PI } else { FRAC_PI_2 };
                g.angle = (g.angle / lie).round() * lie;
            }
            node.left = px(g.at.x);
            node.top = px(g.at.y);
            tf.rotation = Rot2::radians(g.angle);
        }
        // a struck-off part keeps dripping where it lies, less and less
        if g.bleed > 0.0 {
            g.bleed -= dt;
            if r(99) < dt * 6.0 * (g.bleed / 3.5) {
                let drop_at = g.at + Vec2::new((r(98) - 0.5) * g.radius, 0.0);
                if g.still {
                    stain(&mut commands, &art, stains, Vec2::new(drop_at.x, g.ground + (r(97) - 0.5) * 8.0), 5.0 + 6.0 * r(96), g.blood, *seed + 3);
                }
            }
        }
    }
}

/// A wound on a body pumps blood out in pulses, weakening, then only seeps; every drop lands as
/// a stain. The wound moves with the body as it falls.
fn bleed(
    mut commands: Commands,
    time: Res<Time>,
    mut bursts: ResMut<fx::Bursts>,
    corpses: Query<(&Corpse, &UiGlobalTransform, &ComputedNode)>,
    mut bleeders: Query<(Entity, &mut Bleeder)>,
    scale: Res<UiScale>,
    art: Res<BloodArt>,
    float_layer: Single<Entity, With<FloatLayer>>,
) {
    let dt = time.delta_secs();
    for (e, mut b) in &mut bleeders {
        b.t += dt;
        if b.t >= b.life {
            commands.entity(e).despawn();
            continue;
        }
        let Ok((corpse, global, node)) = corpses.get(b.corpse) else {
            commands.entity(e).despawn();
            continue;
        };
        b.due -= dt;
        if b.due > 0.0 {
            continue;
        }
        // where the wound is now: the corpse's centre on the stage, and the wound turned with it
        let size = node.size() * node.inverse_scale_factor();
        let centre = global.translation / scale.0;
        let at = centre + Vec2::from_angle(corpse.angle).rotate(b.local - size / 2.0);
        let dir = Vec2::from_angle(corpse.angle).rotate(b.dir);
        // a spurt with every beat for the first two seconds, weaker each time; then a seep
        let spurting = b.t < 2.0;
        let beat = 0.42;
        let force = if spurting { b.strength * (1.0 - b.t / 2.4) } else { 0.15 * b.strength * (1.0 - (b.t - 2.0) / (b.life - 2.0)) };
        bursts.0.push(fx::Burst::Spurt { at, dir, force, blood: b.color, ground: corpse.ground.y });
        // a strong beat throws a jet out of the wound, drawn frame by frame
        if spurting && force > 0.25 {
            let h = 120.0 + 160.0 * force;
            play(&mut commands, *float_layer, &art.spurt, at, Vec2::new(h * 0.33, h), Vec2::new(0.1, 1.0), dir.x.atan2(-dir.y), b.color, 16.0, false, dir.x < 0.0);
        }
        b.due = if spurting { beat } else { 0.25 };
    }
}
