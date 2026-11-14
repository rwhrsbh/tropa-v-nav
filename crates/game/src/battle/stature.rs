//! How tall each foe stands beside the heroes. Every picture is painted to fill its frame, so a
//! raven and a giant would otherwise be drawn the same size: this table gives each its stature
//! (a grown man 1.0) and how much of its picture's height the figure fills.

/// The stage height a man stands at; a hero is about this tall.
pub(super) const MAN: f32 = 345.0;
/// The tallest a foe may be drawn and still fit under the top of the screen.
const TALLEST: f32 = 560.0;

/// Foe, stature, share of the picture's height the figure fills.
const STATURE: &[(&str, f32, f32)] = &[
    ("upyr", 1.0, 0.936),
    ("auka", 0.85, 0.936),
    ("kikimora", 0.8, 0.936),
    ("bannik", 0.8, 0.98),
    ("ovinnik", 0.75, 0.98),
    ("kladovik", 0.8, 0.98),
    ("vodyanoy", 1.2, 0.98),
    ("chuchelo", 1.2, 0.98),
    ("verlioka", 1.5, 0.98),
    ("viy", 1.6, 0.98),
    ("leshy", 1.4, 0.937),
    ("koschei_guard", 1.1, 0.936),
    ("kostomakha", 1.1, 0.936),
    ("mara", 1.1, 0.936),
    ("poludnitsa", 1.0, 0.937),
    ("vurdalak_elder", 1.1, 0.937),
    ("shatun", 1.4, 0.937),
    ("bolotnik", 0.95, 0.937),
    ("igosha", 0.6, 0.936),
    ("likho", 1.1, 0.936),
    ("lesavka", 0.85, 0.936),
    ("mavka", 1.0, 0.936),
    ("drowned_maiden", 1.0, 0.936),
    ("grey_wolf", 0.7, 0.763),
    ("raven", 0.55, 0.935),
    ("raven_upyr", 1.1, 0.936),
    ("tryasovitsa", 1.0, 0.937),
    ("shishiga", 0.85, 0.931),
    ("yrka", 1.1, 0.936),
    ("tinyak", 0.8, 0.936),
    ("mushroom_corpse", 1.0, 0.936),
    ("volot", 1.6, 0.936),
    ("volot_giant", 1.9, 0.937),
    ("koschei", 1.3, 0.937),
    ("koschei_shadow", 1.2, 0.937),
    ("sirin", 1.1, 0.937),
    ("poloz", 1.4, 0.927),
    ("anchutka", 0.7, 0.921),
    ("nav_soul", 1.0, 0.936),
    ("weeper", 0.95, 0.936),
    ("black_rider", 1.8, 0.937),
    ("tree_guard", 1.5, 0.936),
    ("death_oak", 1.9, 0.937),
    ("death_hare", 0.5, 0.79),
    ("death_duck", 0.45, 0.574),
    ("death_chest", 0.5, 0.935),
    ("death_egg", 0.35, 0.889),
    ("death_egg_solo", 0.35, 0.889),
    ("death_oak_chest", 1.6, 0.937),
    ("ice_crown", 0.5, 0.85),
    ("swamp_light", 0.6, 0.935),
    ("kot_bayun", 1.25, 0.94),
    ("mokhovik", 0.85, 0.94),
    ("nochnitsa", 0.95, 0.94),
    ("baba_yaga", 1.35, 0.94),
    ("polunochnitsa", 1.1, 0.94),
    ("zlyden", 0.65, 0.92),
    ("morovitsa", 1.2, 0.94),
    ("solovey", 1.35, 0.94),
    ("korochun", 1.45, 0.94),
    ("vodyanoy_tsar", 1.5, 0.94),
    ("sea_guard", 1.05, 0.94),
    ("ogneya", 1.1, 0.94),
    ("gagana", 1.4, 0.94),
    ("gorynych_fire", 1.1, 0.94),
    ("gorynych_poison", 1.1, 0.94),
    ("gorynych_thunder", 1.1, 0.94),
];

/// The drawn height for a foe's picture, when its stature is known.
pub(super) fn height_of(def: &str) -> Option<f32> {
    let (_, stature, fill) = STATURE.iter().find(|(d, ..)| *d == def)?;
    Some((MAN * stature * 0.94 / fill.max(0.3)).clamp(140.0, TALLEST))
}
