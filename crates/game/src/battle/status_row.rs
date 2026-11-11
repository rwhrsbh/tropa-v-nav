//! What lies on a unit, as a row of small pictures with a number on each, under its name: its
//! statuses, the curses and spells on it, and a hero's own things (the moon, the mask, the runes
//! carved, the herbs in the cauldron). Hovering the unit names them all and says what they do.
//! A picture is `assets/status/<key>` (herbs and masks wear their cards' art); while a picture
//! is missing, the chip shows the first letters of the name.

use super::*;

const CHIP: f32 = 34.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, sync_rows.after(sync_units).run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>));
}

/// The row under a unit's name. `shown` is what it shows now, so it is rebuilt only on change.
#[derive(Component)]
pub(super) struct StatusRow {
    unit: UnitRef,
    shown: Vec<(String, Option<i32>)>,
}

pub(super) fn status_row(unit: UnitRef) -> impl Bundle {
    (
        StatusRow { unit, shown: Vec::new() },
        Node { flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::Center, column_gap: px(4), row_gap: px(4), margin: UiRect::top(px(4)), max_width: px(UNIT_W), ..default() },
        GlobalZIndex(20),
        Pickable::IGNORE,
    )
}

/// One thing on a unit.
pub(super) struct Chip {
    /// Art stem of its picture, also what tells chips apart.
    pub icon: String,
    pub n: Option<i32>,
    /// How `n` is written, when not as a plain number.
    pub shown: Option<String>,
    /// A boon (gold edge), a bane (red edge) or neither.
    pub good: Option<bool>,
    pub name: String,
    pub help: String,
}

fn boon(st: StatusKind) -> bool {
    matches!(st, StatusKind::Strength | StatusKind::TempStrength | StatusKind::Fortitude | StatusKind::Thorns | StatusKind::Valor | StatusKind::Mist | StatusKind::Drowse)
}

fn help(l10n: &L10n, key: &str) -> String {
    let k = format!("{key}-help");
    if l10n.has(&k) {
        l10n.t(&k)
    } else {
        String::new()
    }
}

/// Everything on a unit, in the order the row shows it.
pub(super) fn chips(l10n: &L10n, session: &Session, unit: UnitRef) -> Vec<Chip> {
    let mut out = Vec::new();
    let status = |out: &mut Vec<Chip>, map: &Statuses| {
        for (st, n) in map.iter() {
            let key = format!("status-{st:?}");
            out.push(Chip { icon: format!("status/{st:?}"), n: Some(*n), shown: Some(l10n.status_amount(*st, *n)), good: Some(boon(*st)), name: l10n.t(&key), help: help(l10n, &key) });
        }
    };
    match unit {
        UnitRef::Hero(i) => {
            let (Some(h), Some(real)) = (session.shown.hero(i), session.truth.hero(i).or(session.shown.hero(i))) else { return out };
            if h.downed {
                out.push(Chip { icon: "status/downed".into(), n: None, shown: None, good: Some(false), name: l10n.t("battle-downed"), help: String::new() });
                return out;
            }
            if h.mechanic == Mechanic::Moon {
                let form = l10n.t(&format!("form-{:?}", h.form));
                out.push(Chip { icon: format!("status/moon_{:?}", h.form), n: Some(real.moon as i32), shown: None, good: None, name: format!("{form} · {}", l10n.t("res-moon")), help: format!("{}/5. {}", real.moon, l10n.t("mech-Moon")) });
            }
            if let Some(m) = h.mask {
                let key = format!("mask-{m:?}");
                out.push(Chip { icon: format!("cards/sk_mask_{}", format!("{m:?}").to_lowercase()), n: None, shown: None, good: Some(true), name: l10n.t(&key), help: help(l10n, &key) });
            }
            // the same rune carved twice is one chip with a two on it
            let mut runes: Vec<(RuneKind, i32)> = Vec::new();
            for r in &h.runes {
                match runes.iter_mut().find(|(k, _)| k == r) {
                    Some((_, n)) => *n += 1,
                    None => runes.push((*r, 1)),
                }
            }
            for (r, n) in runes {
                let key = format!("rune-{r:?}");
                out.push(Chip { icon: format!("status/rune_{r:?}"), n: (n > 1).then_some(n), shown: None, good: Some(true), name: l10n.t(&key), help: help(l10n, &key) });
            }
            let mut herbs: Vec<(String, i32)> = Vec::new();
            for x in &real.cauldron {
                match herbs.iter_mut().find(|(k, _)| k == x) {
                    Some((_, n)) => *n += 1,
                    None => herbs.push((x.clone(), 1)),
                }
            }
            for (x, n) in herbs {
                out.push(Chip { icon: format!("cards/herb_{x}"), n: (n > 1).then_some(n), shown: None, good: None, name: format!("{}: {}", l10n.t("res-cauldron"), l10n.t(&format!("herb-{x}"))), help: help(l10n, &format!("herb-{x}")) });
            }
            if h.walks_headless {
                out.push(Chip { icon: "status/hollow".into(), n: None, shown: None, good: Some(false), name: l10n.t("trait-walks-headless"), help: help(l10n, "trait-walks-headless") });
            }
            if h.lost_limbs > 0 {
                // each hero loses what that arm held
                let key = match h.lost_arms {
                    nav_core::combat::WEAPON_ARM => format!("trait-lost-weapon-{}", h.def),
                    nav_core::combat::SHIELD_ARM => format!("trait-lost-shield-{}", h.def),
                    _ => format!("trait-lost-both-{}", h.def),
                };
                out.push(Chip { icon: "status/lost_limb".into(), n: (h.lost_limbs > 1).then_some(h.lost_limbs as i32), shown: None, good: Some(false), name: l10n.t(&key), help: help(l10n, &key) });
            }
            for flag in ["marked", "blind", "coiled", "confused", "mired", "asleep", "charmed", "frozen", "winter", "draw_less", "energy_less", "no_intents", "doom"] {
                if let Some(n) = real.flags.get(flag) {
                    let key = format!("trait-{flag}");
                    out.push(Chip { icon: format!("status/{flag}"), n: (*n > 1 || flag == "doom").then_some(*n), shown: None, good: Some(false), name: l10n.t(&key), help: help(l10n, &key) });
                }
            }
            status(&mut out, &h.statuses);
        }
        UnitRef::Enemy(uid) => {
            let Some(e) = session.shown.enemy(uid) else { return out };
            // the real state carries the traits and the countdown
            if let Some(real) = session.truth.enemy(uid) {
                if real.flags.contains_key("immortal") {
                    out.push(Chip { icon: "status/immortal".into(), n: None, shown: None, good: Some(true), name: l10n.t("trait-immortal"), help: help(l10n, "trait-immortal") });
                }
                // what its kind does, while it can: the hare's dodge is gone once spent this round
                for flag in [
                    "dodge", "flying", "hiding", "enrage", "lifesteal", "piercing", "rises", "hunter", "pack", "spores", "vengeful",
                    "burning", "greedy", "hollow", "growing", "kinetic_moss", "bayun_perch", "spell_mirror", "flicker", "gorynych_head", "yaga_mortar", "overheat",
                ] {
                    if real.flags.contains_key(flag) && !(flag == "dodge" && real.flags.contains_key("turn_dodged")) {
                        let key = format!("trait-{flag}");
                        out.push(Chip { icon: format!("status/{flag}"), n: None, shown: None, good: Some(true), name: l10n.t(&key), help: help(l10n, &key) });
                    }
                }
                if let Some(n) = real.flags.get("kinetic_charge") {
                    if *n > 0 {
                        let key = "trait-kinetic_charge";
                        out.push(Chip { icon: "status/kinetic_charge".into(), n: Some(*n), shown: None, good: Some(true), name: l10n.t(key), help: help(l10n, key) });
                    }
                }
                if let Some(n) = real.flags.get("last_breath") {
                    out.push(Chip { icon: "status/last_breath".into(), n: Some(*n), shown: None, good: None, name: l10n.n("trait-last-breath", *n), help: String::new() });
                }
            }
            status(&mut out, &e.statuses);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn sync_rows(
    mut commands: Commands,
    session: Res<Session>,
    l10n: Res<L10n>,
    fonts: Res<Fonts>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut rows: Query<(Entity, &mut StatusRow)>,
) {
    for (row, mut r) in &mut rows {
        let chips = chips(&l10n, &session, r.unit);
        let now: Vec<(String, Option<i32>)> = chips.iter().map(|c| (c.icon.clone(), c.n)).collect();
        if now == r.shown {
            continue;
        }
        r.shown = now;
        commands.entity(row).despawn_children();
        for c in chips {
            let edge = match c.good {
                Some(true) => theme::CANDLE,
                Some(false) => theme::FRESH_BLOOD,
                None => Color::srgb(0.42, 0.33, 0.22),
            };
            let picture = art.image(&assets, &c.icon);
            // a card's art is wider than tall: shown whole in height, its sides clipped
            let wide = c.icon.starts_with("cards/");
            let chip = commands
                .spawn((
                    Node {
                        width: px(CHIP),
                        height: px(CHIP),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(8)),
                        overflow: Overflow::clip(),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(theme::SOOT.with_alpha(0.92)),
                    BorderColor::all(edge),
                    Pickable::IGNORE,
                    ChildOf(row),
                ))
                .with_children(|chip| {
                    match picture {
                        Some(h) => {
                            let w = if wide { CHIP * 1.34 } else { CHIP - 4.0 };
                            chip.spawn((Node { width: px(w), height: px(CHIP - 4.0), flex_shrink: 0.0, ..default() }, ImageNode::new(h), Pickable::IGNORE));
                        }
                        None => {
                            let glyph: String = c.name.chars().filter(|ch| ch.is_alphabetic()).take(2).collect();
                            chip.spawn(theme::text(glyph, &fonts.num, 15.0, theme::BONE));
                        }
                    }
                })
                .id();
            // the number sits in the chip's corner, large and dark-ringed so it reads over any picture
            if let Some(n) = c.n {
                commands.spawn((
                    Node { position_type: PositionType::Absolute, right: px(1), bottom: px(-2), ..default() },
                    theme::text(c.shown.clone().unwrap_or_else(|| n.to_string()), &fonts.num, 19.0, theme::BONE),
                    TextShadow { offset: Vec2::new(1.5, 1.5), color: Color::BLACK },
                    ChildOf(chip),
                ));
            }
        }
    }
}
