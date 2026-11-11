//! A body for a painted figure: each unit is drawn through a shader (`assets/shaders/rig.wesl`)
//! that bends the picture up a spine, feet planted, the way a cut-out puppet would move. Idle,
//! the figure breathes, sways and its cloth and hair ripple; a blow it lands leans it into the
//! foe, a blow it takes bends it back and flashes it; a spell gathers it up and throws it forward.
//! The sprite node keeps its own transform for the larger moves (a step forward, a recoil); the
//! rig adds the bending that a flat picture cannot do by moving as a whole.
//!
//! A figure may also be cut into parts (see `FIGURES`): its picture is then drawn as several
//! skins, `<stem>_body` (the figure with its parts painted out and what was behind them painted
//! in) and one `<stem>_<part>` per part (the part alone, where it was), each turning about its
//! own joint: the weapon arm swings, the head nods and snaps back, the shield is raised.

use super::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// Share of the skin padded around the picture on each side, and above it, so that a bend is
/// never cut off at the picture's edge.
const PAD_X: f32 = 0.12;
const PAD_TOP: f32 = 0.08;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(UiMaterialPlugin::<RigMaterial>::default());
}

#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(super) struct RigMaterial {
    #[uniform(0)]
    pub rig: RigUniform,
    #[texture(1)]
    #[sampler(2)]
    pub picture: Handle<Image>,
}

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub(super) struct RigUniform {
    /// Time, lean, idle sway, cloth wave.
    pub motion: Vec4,
    /// Chest breath, squash, flash amount, fade.
    pub body: Vec4,
    pub tint: Vec4,
    pub flash: Vec4,
    /// Padding left, right, top; facing.
    pub pad: Vec4,
    /// A limb's joint on the picture (x, y in 0..1), its turn (radians, clockwise on screen) and
    /// the picture's width over height; no turn on a body.
    pub limb: Vec4,
    /// Above zero: the figure is drawn as its white outline only.
    pub sight: Vec4,
}

/// A part a figure may be cut into, each moved its own way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Part {
    /// The arm that strikes, with its weapon.
    Arm,
    Head,
    /// The shield, or the other arm.
    Shield,
}

impl Part {
    fn file(self) -> &'static str {
        match self {
            Part::Arm => "arm",
            Part::Head => "head",
            Part::Shield => "shield",
        }
    }
}

/// Figures cut into parts: the art stem, and each part with its joint on the picture and how
/// strongly (and which way) it follows its turn, in the order they are laid over the body. An
/// arm held up (a staff, a hand raised to curse) turns the other way from one hanging down.
pub(super) type Cut = (Part, Vec2, f32);
const FIGURES: &[(&str, &[Cut])] = &[
    ("heroes/bogatyr", &[(Part::Head, Vec2::new(0.52, 0.15), 1.0), (Part::Shield, Vec2::new(0.68, 0.30), 1.0), (Part::Arm, Vec2::new(0.275, 0.42), 1.0)]),
    ("heroes/vorozheya", &[(Part::Shield, Vec2::new(0.36, 0.355), 0.8), (Part::Head, Vec2::new(0.46, 0.22), 1.0), (Part::Arm, Vec2::new(0.66, 0.335), -0.7)]),
    ("heroes/volhv", &[(Part::Shield, Vec2::new(0.33, 0.36), 0.8), (Part::Head, Vec2::new(0.47, 0.18), 0.9), (Part::Arm, Vec2::new(0.635, 0.37), -0.45)]),
    ("heroes/volkolak", &[(Part::Shield, Vec2::new(0.70, 0.43), 0.8), (Part::Head, Vec2::new(0.58, 0.24), 1.0), (Part::Arm, Vec2::new(0.29, 0.42), 1.0)]),
    // the beast keeps the man's arms: the shield arm is the near one, as it is on the man
    ("heroes/volkolak_beast", &[(Part::Shield, Vec2::new(0.654, 0.36), 1.0), (Part::Head, Vec2::new(0.70, 0.24), 1.2), (Part::Arm, Vec2::new(0.265, 0.30), 0.8)]),
    ("heroes/skomoroh", &[(Part::Shield, Vec2::new(0.69, 0.44), 0.8), (Part::Head, Vec2::new(0.57, 0.22), 1.0), (Part::Arm, Vec2::new(0.20, 0.37), 0.8)]),
    ("enemies/bannik", &[(Part::Head, Vec2::new(0.584, 0.305), 1.0), (Part::Shield, Vec2::new(0.788, 0.373), 0.8), (Part::Arm, Vec2::new(0.55, 0.38), 0.9)]),
    ("enemies/ovinnik", &[(Part::Head, Vec2::new(0.178, 0.228), 1.0), (Part::Shield, Vec2::new(0.342, 0.415), 0.8), (Part::Arm, Vec2::new(0.183, 0.419), 1.0)]),
    ("enemies/kladovik", &[(Part::Head, Vec2::new(0.445, 0.231), 1.0), (Part::Shield, Vec2::new(0.286, 0.5), 0.8), (Part::Arm, Vec2::new(0.636, 0.446), 1.0)]),
    ("enemies/vodyanoy", &[(Part::Head, Vec2::new(0.492, 0.31), 1.0), (Part::Shield, Vec2::new(0.703, 0.365), 0.8), (Part::Arm, Vec2::new(0.332, 0.388), 1.0)]),
    ("enemies/chuchelo", &[(Part::Head, Vec2::new(0.51, 0.2), 0.8), (Part::Shield, Vec2::new(0.68, 0.431), 0.7), (Part::Arm, Vec2::new(0.311, 0.431), 0.7)]),
    ("enemies/verlioka", &[(Part::Head, Vec2::new(0.44, 0.253), 1.0), (Part::Shield, Vec2::new(0.733, 0.359), 0.8), (Part::Arm, Vec2::new(0.308, 0.3), 0.5)]),
    // cut anew from a colour mask: both arms off the body, the left one (behind the hair) painted
    // whole, so it is there when the head and its hair are struck off
    ("enemies/viy", &[(Part::Shield, Vec2::new(0.33, 0.25), 0.6), (Part::Head, Vec2::new(0.42, 0.2), 0.4), (Part::Arm, Vec2::new(0.72, 0.2), 1.0)]),
    ("heroes/znaharka", &[(Part::Shield, Vec2::new(0.40, 0.40), 0.7), (Part::Head, Vec2::new(0.61, 0.20), 0.9), (Part::Arm, Vec2::new(0.73, 0.40), -0.6)]),
    // the "shield" is his chimney, which only rocks; the arm holds the bucket and the pike
    ("heroes/emelya", &[(Part::Head, Vec2::new(0.443, 0.288), 0.9), (Part::Shield, Vec2::new(0.389, 0.326), 0.8), (Part::Arm, Vec2::new(0.702, 0.256), 0.8)]),
    ("enemies/death_duck", &[(Part::Head, Vec2::new(0.27, 0.74), 0.8)]),
    ("enemies/death_hare", &[(Part::Head, Vec2::new(0.36, 0.40), 0.8)]),
    ("enemies/death_oak", &[(Part::Head, Vec2::new(0.53, 0.30), 0.6), (Part::Shield, Vec2::new(0.68, 0.35), 0.6), (Part::Arm, Vec2::new(0.40, 0.30), 0.7)]),
    ("enemies/tree_guard", &[(Part::Head, Vec2::new(0.52, 0.36), 0.8), (Part::Shield, Vec2::new(0.62, 0.38), 0.8), (Part::Arm, Vec2::new(0.40, 0.42), 1.0)]),
    ("enemies/black_rider", &[(Part::Head, Vec2::new(0.521, 0.203), 1.0), (Part::Arm, Vec2::new(0.41, 0.33), -0.4)]),
    ("enemies/weeper", &[(Part::Head, Vec2::new(0.422, 0.30), 1.0), (Part::Arm, Vec2::new(0.36, 0.34), -0.4)]),
    ("enemies/nav_soul", &[(Part::Head, Vec2::new(0.486, 0.207), 1.0), (Part::Arm, Vec2::new(0.42, 0.30), 0.3)]),
    ("enemies/anchutka", &[(Part::Head, Vec2::new(0.53, 0.44), 1.0), (Part::Shield, Vec2::new(0.70, 0.22), -0.8), (Part::Arm, Vec2::new(0.32, 0.14), 0.8)]),
    ("enemies/poloz", &[(Part::Head, Vec2::new(0.55, 0.47), 0.7)]),
    ("enemies/sirin", &[(Part::Head, Vec2::new(0.42, 0.28), 1.0), (Part::Shield, Vec2::new(0.60, 0.30), -0.8), (Part::Arm, Vec2::new(0.36, 0.30), 0.8)]),
    ("enemies/koschei_shadow", &[(Part::Head, Vec2::new(0.440, 0.223), 1.0), (Part::Shield, Vec2::new(0.551, 0.417), 0.8), (Part::Arm, Vec2::new(0.337, 0.457), 1.0)]),
    ("enemies/koschei", &[(Part::Head, Vec2::new(0.502, 0.210), 1.0), (Part::Shield, Vec2::new(0.588, 0.440), 0.5), (Part::Arm, Vec2::new(0.24, 0.42), -0.5)]),
    ("enemies/volot_giant", &[(Part::Head, Vec2::new(0.47, 0.28), 1.0), (Part::Shield, Vec2::new(0.66, 0.27), -0.6), (Part::Arm, Vec2::new(0.28, 0.27), -0.6)]),
    ("enemies/volot", &[(Part::Head, Vec2::new(0.516, 0.150), 1.0), (Part::Arm, Vec2::new(0.55, 0.30), 0.6)]),
    ("enemies/mushroom_corpse", &[(Part::Head, Vec2::new(0.381, 0.214), 1.0), (Part::Shield, Vec2::new(0.630, 0.395), 0.8), (Part::Arm, Vec2::new(0.341, 0.392), 1.0)]),
    ("enemies/yrka", &[(Part::Head, Vec2::new(0.450, 0.190), 1.0), (Part::Shield, Vec2::new(0.369, 0.355), 0.8), (Part::Arm, Vec2::new(0.214, 0.270), 1.0)]),
    ("enemies/tinyak", &[(Part::Head, Vec2::new(0.504, 0.259), 1.0), (Part::Shield, Vec2::new(0.196, 0.433), 0.8), (Part::Arm, Vec2::new(0.854, 0.438), 1.0)]),
    ("enemies/tryasovitsa", &[(Part::Head, Vec2::new(0.490, 0.215), 1.0), (Part::Shield, Vec2::new(0.536, 0.379), 0.8), (Part::Arm, Vec2::new(0.408, 0.364), 1.0)]),
    ("enemies/shishiga", &[(Part::Head, Vec2::new(0.354, 0.345), 1.0), (Part::Shield, Vec2::new(0.633, 0.337), 0.8), (Part::Arm, Vec2::new(0.366, 0.375), 1.0)]),
    ("enemies/raven_upyr", &[(Part::Head, Vec2::new(0.207, 0.285), 1.0)]),
    ("enemies/raven", &[(Part::Head, Vec2::new(0.385, 0.539), 1.0)]),
    ("enemies/likho", &[(Part::Head, Vec2::new(0.487, 0.286), 1.0), (Part::Shield, Vec2::new(0.710, 0.313), 0.8), (Part::Arm, Vec2::new(0.469, 0.368), 1.0)]),
    ("enemies/lesavka", &[(Part::Head, Vec2::new(0.501, 0.266), 1.0), (Part::Shield, Vec2::new(0.335, 0.312), 0.35), (Part::Arm, Vec2::new(0.62, 0.33), 0.4)]),
    ("enemies/mavka", &[(Part::Head, Vec2::new(0.502, 0.337), 1.0), (Part::Arm, Vec2::new(0.27, 0.30), -0.6)]),
    ("enemies/drowned_maiden", &[(Part::Head, Vec2::new(0.581, 0.218), 1.0)]),
    ("enemies/grey_wolf", &[(Part::Head, Vec2::new(0.42, 0.40), 1.0)]),
    ("enemies/bolotnik", &[(Part::Head, Vec2::new(0.377, 0.260), 1.0), (Part::Shield, Vec2::new(0.222, 0.412), 0.8), (Part::Arm, Vec2::new(0.811, 0.363), 1.0)]),
    ("enemies/igosha", &[(Part::Head, Vec2::new(0.473, 0.346), 1.0)]),
    ("enemies/vurdalak_elder", &[(Part::Head, Vec2::new(0.509, 0.254), 1.0), (Part::Shield, Vec2::new(0.723, 0.460), 0.8), (Part::Arm, Vec2::new(0.25, 0.37), -0.6)]),
    ("enemies/shatun", &[(Part::Head, Vec2::new(0.387, 0.241), 1.0), (Part::Shield, Vec2::new(0.269, 0.485), 0.8), (Part::Arm, Vec2::new(0.690, 0.480), 1.0)]),
    ("enemies/upyr", &[(Part::Head, Vec2::new(0.458, 0.223), 1.0), (Part::Shield, Vec2::new(0.671, 0.402), 0.8), (Part::Arm, Vec2::new(0.250, 0.407), 1.0)]),
    ("enemies/auka", &[(Part::Head, Vec2::new(0.52, 0.29), 1.0), (Part::Shield, Vec2::new(0.74, 0.48), -0.6), (Part::Arm, Vec2::new(0.25, 0.43), -0.6)]),
    ("enemies/kikimora", &[(Part::Head, Vec2::new(0.46, 0.22), 1.0), (Part::Shield, Vec2::new(0.56, 0.33), 0.8), (Part::Arm, Vec2::new(0.64, 0.42), 1.0)]),
    ("enemies/leshy", &[(Part::Head, Vec2::new(0.48, 0.30), 1.0), (Part::Shield, Vec2::new(0.853, 0.33), 0.8), (Part::Arm, Vec2::new(0.497, 0.434), 1.0)]),
    ("enemies/koschei_guard", &[(Part::Head, Vec2::new(0.488, 0.153), 1.0), (Part::Shield, Vec2::new(0.653, 0.286), 0.8), (Part::Arm, Vec2::new(0.317, 0.393), 1.0)]),
    ("enemies/kostomakha", &[(Part::Head, Vec2::new(0.473, 0.143), 1.0), (Part::Shield, Vec2::new(0.888, 0.338), 0.8), (Part::Arm, Vec2::new(0.460, 0.381), 1.0)]),
    ("enemies/mara", &[(Part::Head, Vec2::new(0.54, 0.22), 1.0), (Part::Shield, Vec2::new(0.58, 0.45), 0.8), (Part::Arm, Vec2::new(0.32, 0.42), -0.5)]),
    ("enemies/poludnitsa", &[(Part::Head, Vec2::new(0.464, 0.219), 1.0), (Part::Shield, Vec2::new(0.387, 0.406), 0.8), (Part::Arm, Vec2::new(0.545, 0.406), 1.0)]),
    ("enemies/kot_bayun", &[(Part::Head, Vec2::new(0.5, 0.396), 1.0)]),
    ("enemies/mokhovik", &[(Part::Head, Vec2::new(0.5, 0.394), 1.0)]),
    ("enemies/nochnitsa", &[(Part::Head, Vec2::new(0.5, 0.3), 1.0)]),
    ("enemies/baba_yaga", &[(Part::Head, Vec2::new(0.499, 0.318), 1.0)]),
    ("enemies/polunochnitsa", &[(Part::Head, Vec2::new(0.499, 0.281), 1.0)]),
    ("enemies/zlyden", &[(Part::Head, Vec2::new(0.5, 0.347), 1.0)]),
    ("enemies/morovitsa", &[(Part::Head, Vec2::new(0.499, 0.281), 1.0)]),
    ("enemies/ogneya", &[(Part::Head, Vec2::new(0.5, 0.3), 1.0)]),
    ("enemies/sea_guard", &[(Part::Head, Vec2::new(0.499, 0.301), 1.0)]),
    ("enemies/gagana", &[(Part::Head, Vec2::new(0.5, 0.375), 1.0)]),
    ("enemies/gorynych", &[(Part::Head, Vec2::new(0.499, 0.347), 1.0)]),
    ("enemies/gorynych_fire", &[(Part::Head, Vec2::new(0.499, 0.347), 1.0)]),
    ("enemies/gorynych_poison", &[(Part::Head, Vec2::new(0.499, 0.347), 1.0)]),
    ("enemies/gorynych_thunder", &[(Part::Head, Vec2::new(0.499, 0.347), 1.0)]),
    ("enemies/solovey", &[(Part::Head, Vec2::new(0.499, 0.318), 1.0)]),
    ("enemies/korochun", &[(Part::Head, Vec2::new(0.499, 0.3), 1.0)]),
    ("enemies/vodyanoy_tsar", &[(Part::Head, Vec2::new(0.499, 0.318), 1.0)]),
];

/// The parts `stem` is cut into, with their joints.

pub(super) fn cuts_of(stem: &str) -> &'static [Cut] {
    FIGURES.iter().find(|(s, _)| *s == stem).map(|(_, c)| *c).unwrap_or(&[])
}

/// Whether `stem` is a figure cut into parts (its blows take longer: a weapon is swung).
pub(super) fn is_cut(stem: &str) -> bool {
    FIGURES.iter().any(|(s, _)| *s == stem)
}

/// The pictures of `stem` cut into parts: its body and each part with its joint, when all of
/// them are there.
pub(super) fn parts_of(art: &Art, assets: &AssetServer, stem: &str) -> Option<(Handle<Image>, Vec<(Cut, Handle<Image>)>)> {
    let (_, parts) = FIGURES.iter().find(|(s, _)| *s == stem)?;
    let body = art.image(assets, &format!("{stem}_body"))?;
    let parts = parts.iter().map(|cut| Some((*cut, art.image(assets, &format!("{stem}_{}", cut.0.file()))?))).collect::<Option<Vec<_>>>()?;
    Some((body, parts))
}

impl UiMaterial for RigMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/rig.wesl".into()
    }
}

/// The figure's skin: the shader-drawn picture inside a sprite node, larger than it by the pads.
/// A part's skin knows which part it is; the body's (or a whole picture's) is `None`.
#[derive(Component)]
pub(super) struct RigSkin {
    pub part: Option<Part>,
    /// How strongly, and which way, the part follows its turn.
    pub gain: f32,
}

/// How the figure is bent this frame, worked out from its animation (see `animate_sprites`).
#[derive(Clone, Copy)]
pub(super) struct Pose {
    pub lean: f32,
    pub sway: f32,
    pub wave: f32,
    pub breath: f32,
    pub squash: f32,
    pub flash: f32,
    pub flash_color: Color,
    pub tint: Color,
    pub fade: f32,
    /// How far each part is turned (radians, + clockwise on a hero's screen): the weapon arm
    /// (+ down and back, - raised toward the foe), the head (+ bowed, - thrown back), the shield
    /// (- raised before the body).
    pub swing: f32,
    pub nod: f32,
    pub guard: f32,
    /// Seen by one with no eyes: only the outline.
    pub outline: bool,
}

impl Pose {
    /// At rest: breathing, swaying a little, cloth stirring.
    pub fn idle(tint: Color, hero: bool) -> Pose {
        Pose { lean: 0.0, sway: if hero { 0.005 } else { 0.007 }, wave: 0.0035, breath: 0.018, squash: 1.0, flash: 0.0, flash_color: Color::WHITE, tint, fade: 1.0, swing: 0.0, nod: 0.0, guard: 0.0, outline: false }
    }
}

/// The skin for a picture drawn at `draw` stage pixels, as a child of the sprite node.
pub(super) fn skin(materials: &mut Assets<RigMaterial>, picture: Option<Handle<Image>>, tint: Color, draw: Vec2, hero: bool, part: Option<Cut>) -> impl Bundle {
    let w = draw.x / (1.0 - 2.0 * PAD_X);
    let h = draw.y / (1.0 - PAD_TOP);
    let face = if hero { 1.0 } else { -1.0 };
    let material = materials.add(RigMaterial {
        rig: RigUniform {
            pad: Vec4::new(PAD_X, PAD_X, PAD_TOP, face),
            tint: tint.to_linear().to_vec4(),
            body: Vec4::new(0.0, 1.0, 0.0, 1.0),
            limb: part.map(|(_, j, _)| Vec4::new(j.x, j.y, 0.0, draw.x / draw.y)).unwrap_or(Vec4::new(0.0, 0.0, 0.0, 1.0)),
            ..default()
        },
        picture: picture.unwrap_or_default(),
    });
    (
        RigSkin { part: part.map(|(p, ..)| p), gain: part.map(|(.., g)| g).unwrap_or(0.0) },
        MaterialNode(material),
        Node { position_type: PositionType::Absolute, left: px(-(w - draw.x) / 2.0), top: px(-(h - draw.y)), width: px(w), height: px(h), ..default() },
        Pickable::IGNORE,
    )
}

/// Writes a pose into a skin's material.
pub(super) fn pose(materials: &mut Assets<RigMaterial>, handle: &MaterialNode<RigMaterial>, skin: &RigSkin, t: f32, p: Pose) {
    let Some(mut m) = materials.get_mut(&handle.0) else { return };
    // a foe faces the other way: its picture is mirrored, so is the turn
    let turn = match skin.part {
        Some(Part::Arm) => p.swing,
        Some(Part::Head) => p.nod,
        Some(Part::Shield) => p.guard,
        None => 0.0,
    };
    m.rig.limb.z = turn * skin.gain * m.rig.pad.w;
    m.rig.motion = Vec4::new(t, p.lean, p.sway, p.wave);
    m.rig.body = Vec4::new(p.breath, p.squash, p.flash, p.fade);
    m.rig.tint = p.tint.to_linear().to_vec4();
    m.rig.flash = p.flash_color.to_linear().to_vec4();
    m.rig.sight = Vec4::new(if p.outline { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0);
}

/// Puts a new picture on a skin (a change of form).
pub(super) fn repaint(materials: &mut Assets<RigMaterial>, handle: &MaterialNode<RigMaterial>, picture: Handle<Image>) {
    if let Some(mut m) = materials.get_mut(&handle.0) {
        m.picture = picture;
    }
}
