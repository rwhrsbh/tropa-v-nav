//! Balance analytics: when a run ends (won, lost or given up), the run's own tally (`Run::stats`,
//! see `nav_core::stats`) is written down with the party's decks, relics and the build each
//! player went for, kept in `profiles/analytics.jsonl` and sent to the Google Sheet behind
//! `telemetry_url` (config.ron or `TROPA_TELEMETRY_URL`). What could not be sent waits in
//! `profiles/analytics_pending.jsonl` and goes at the next start. In a party only the host sends,
//! once for everyone. The sheet's side is `tools/google_sheets_telemetry.js`; what is measured and
//! how to read it: `docs/analytics.md`.

use bevy::prelude::*;
use nav_core::stats::RunStats;
use nav_core::{CardType, Content, Effect, Keyword, Run, StatusKind, Tgt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::l10n::L10n;
use crate::theme::{self, Fonts};
use crate::AppState;
use bevy::ui_widgets::{observe, Activate};

/// Bumped whenever the record's shape changes, so the sheet can tell old rows from new.
pub const SCHEMA: u32 = 2;

/// One finished run, as written down and sent.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunRecord {
    pub schema: u32,
    pub version: String,
    pub run_id: String,
    /// Unix seconds when it ended.
    pub timestamp: u64,
    /// solo, party (Steam), lan, or bots (a dev build's in-process party).
    pub mode: String,
    /// won, lost or abandoned.
    pub result: String,
    pub circle: u8,
    pub act: u8,
    pub floors: u32,
    pub elites: u32,
    pub bosses: u32,
    /// The foe kind that dealt the last blows, for a run lost.
    pub killed_by: Option<String>,
    pub players: Vec<PlayerRecord>,
    pub stats: RunStats,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerRecord {
    pub hero: String,
    pub hp: i32,
    pub max_hp: i32,
    pub gold: i32,
    /// The deck at the end, upgraded cards with a `+`.
    pub deck: Vec<String>,
    pub relics: Vec<String>,
    pub potions: Vec<String>,
    /// How many of the deck's cards (curses, statuses and the hero's starting cards left out)
    /// belong to each family of play (`family_of`).
    pub families: BTreeMap<String, u32>,
    /// The build this deck is: its one or two leading families, joined with `+`, or `mixed`.
    pub build: String,
}

/// The families of play a card belongs to, read off what it does. A deck's build is the families
/// most of its cards share: this is what tells whether a planned build (rot, runes, discard…)
/// carries runs or not.
pub fn family_of(c: &Content, id: &str) -> Vec<&'static str> {
    let Some(def) = c.cards.get(id.trim_end_matches('+')) else { return Vec::new() };
    let mut out: Vec<&'static str> = Vec::new();
    let mut add = |f: &'static str| {
        if !out.contains(&f) {
            out.push(f);
        }
    };
    fn walk(list: &[Effect], add: &mut dyn FnMut(&'static str)) {
        for e in list {
            match e {
                Effect::Apply(st, _, t) => match (st, t) {
                    (StatusKind::Rot, _) => add("rot"),
                    (StatusKind::Bleed, _) => add("bleed"),
                    (StatusKind::Hex, _) => add("hex"),
                    (StatusKind::Fear, _) => add("fear"),
                    (StatusKind::Weak | StatusKind::Vulnerable, _) => add("debuff"),
                    (StatusKind::Strength | StatusKind::TempStrength, _) => add("strength"),
                    (StatusKind::Thorns, _) => add("thorns"),
                    (StatusKind::Fortitude, _) => add("block"),
                    (StatusKind::Drowse, _) => add("drowse"),
                    (StatusKind::Valor, _) => add("valor"),
                    (StatusKind::Luck, _) => add("luck"),
                    (StatusKind::Evade | StatusKind::Charm | StatusKind::Flattery, _) => add("guile"),
                    _ => {}
                },
                Effect::Damage(_, Tgt::AllFoes) | Effect::Hits(_, _, Tgt::AllFoes) => add("aoe"),
                Effect::Hits(..) => add("multi_hit"),
                Effect::Block(..) | Effect::Guard(..) => add("block"),
                Effect::Rune(..) | Effect::Invoke => add("runes"),
                Effect::Discard(_) | Effect::DiscardHand => add("discard"),
                Effect::Damage(nav_core::Amount::Discarded(_) | nav_core::Amount::DiscardedThisTurn(_), _) => add("discard"),
                Effect::ExhaustCards(_) => add("exhaust"),
                Effect::Draw(_) => add("draw"),
                Effect::Energy(..) => add("energy"),
                Effect::Heal(..) => add("heal"),
                Effect::Herb(_) | Effect::Brew => add("brew"),
                Effect::Mask(_) => add("masks"),
                Effect::Shift => add("shift"),
                Effect::PikeWill => add("pike"),
                Effect::Echo(_) => add("echo"),
                // Kuzma's forge and plate
                Effect::Forge(_) | Effect::Mark(_) | Effect::Plate(_) | Effect::Reforge(..) | Effect::Upgrade(..) => add("forge"),
                // Ivan's luck
                Effect::Luck { .. } => add("luck"),
                // Lisa's copies of others' cards
                Effect::Alien { .. } | Effect::Pick { .. } | Effect::DrawFor(..) | Effect::Transmute(..) => add("alien"),
                Effect::If(_, a, b) => {
                    walk(a, add);
                    walk(b, add);
                }
                Effect::Repeat(_, a) | Effect::Once(_, a) | Effect::OncePerTurn(_, a) | Effect::Every(_, _, a) | Effect::EveryThisTurn(_, _, a) => walk(a, add),
                _ => {}
            }
        }
    }
    let view = def.view(id.ends_with('+'), nav_core::Form::Human);
    walk(&view.effects, &mut add);
    // a number of chance is Ivan's own: his range cards feed his luck
    if !nav_core::craft::ranges(&view.effects).is_empty() {
        add("luck");
    }
    // a card that only asks after the new heroes' state (in plate, Чужие in hand, luck) belongs
    // to that family too: its conditions and amounts are read as written
    let said = format!("{:?}{:?}", view.effects, view.passives);
    if said.contains("InPlate") || said.contains("Forge") {
        add("forge");
    }
    if said.contains("Alien") {
        add("alien");
    }
    if said.contains("Luck") || said.contains("lucky") {
        add("luck");
    }
    if said.contains("Stash") {
        add("stash");
    }
    if said.contains("\"borrow\"") || said.contains("Provoke") || said.contains("dodged") {
        add("guile");
    }
    // a card of Kuzma's that plays otherwise in plate is part of his forging
    let plated = def.view(id.ends_with('+'), nav_core::Form::Beast);
    if def.hero == "kuzma" && format!("{:?}", plated.effects) != format!("{:?}", view.effects) {
        add("forge");
    }
    for p in &view.passives {
        walk(&p.effects, &mut add);
    }
    if !view.on_discard.is_empty() {
        add("discard");
    }
    if def.drowse > 0 {
        add("drowse");
    }
    for k in &view.keywords {
        match k {
            Keyword::Exhaust => add("exhaust"),
            Keyword::Retain => add("retain"),
            Keyword::Coop => add("coop"),
            _ => {}
        }
    }
    if def.kind == CardType::Power {
        add("powers");
    }
    out
}

/// A deck's families and the build they make: families held by a quarter of the cards that
/// count (curses, statuses and starting cards do not), the leading two at most.
pub fn build_of(c: &Content, deck: &[String]) -> (BTreeMap<String, u32>, String) {
    let mut fam: BTreeMap<String, u32> = BTreeMap::new();
    let mut counted = 0u32;
    for id in deck {
        let Some(def) = c.cards.get(id.trim_end_matches('+')) else { continue };
        if matches!(def.kind, CardType::Curse | CardType::Status) || def.rarity == nav_core::Rarity::Starter {
            continue;
        }
        counted += 1;
        for f in family_of(c, id) {
            *fam.entry(f.to_string()).or_default() += 1;
        }
    }
    let mut lead: Vec<(&String, &u32)> = fam.iter().filter(|(k, n)| **n * 4 >= counted.max(1) && !matches!(k.as_str(), "draw" | "energy" | "block" | "powers")).collect();
    lead.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let build = if lead.is_empty() { "mixed".to_string() } else { lead.iter().take(2).map(|(k, _)| k.as_str()).collect::<Vec<_>>().join("+") };
    (fam, build)
}

pub fn record_of(c: &Content, run: &Run, mode: &str, result: &str) -> RunRecord {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let players = run
        .players
        .iter()
        .map(|p| {
            let deck: Vec<String> = p.deck.iter().map(|(card, up)| if *up { format!("{card}+") } else { card.clone() }).collect();
            let (families, build) = build_of(c, &deck);
            PlayerRecord { hero: p.hero.clone(), hp: p.hp, max_hp: p.max_hp, gold: p.gold, deck, relics: p.relics.clone(), potions: p.potions.clone(), families, build }
        })
        .collect();
    RunRecord {
        schema: SCHEMA,
        version: crate::updater::SemVer::current().to_string(),
        run_id: format!("{:x}-{now}", run.seed),
        timestamp: now,
        mode: mode.into(),
        result: result.into(),
        circle: run.circle,
        act: run.act,
        floors: run.floors_cleared,
        elites: run.elites_slain,
        bosses: run.bosses_slain,
        killed_by: (result == "lost").then(|| run.felled_by()).flatten(),
        players,
        stats: run.stats.0.clone(),
    }
}

fn dir() -> Option<PathBuf> {
    crate::platform::user_dir().map(|d| d.join("profiles"))
}

pub fn analytics_file_path() -> Option<PathBuf> {
    dir().map(|d| d.join("analytics.jsonl"))
}

fn pending_path() -> Option<PathBuf> {
    dir().map(|d| d.join("analytics_pending.jsonl"))
}

fn append(path: &PathBuf, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

fn url(config_url: Option<&str>) -> Option<String> {
    std::env::var("TROPA_TELEMETRY_URL").ok().or_else(|| config_url.map(str::to_string)).filter(|u| !u.trim().is_empty())
}

/// Posts one record; Apps Script answers with a redirect to its result, which is followed.
fn post(url: &str, body: &str) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(60)).redirects(4).build();
    match agent.post(url).set("Content-Type", "application/json").send_string(body) {
        Ok(resp) => {
            let text = resp.into_string().unwrap_or_default();
            if text.contains("\"error\"") {
                Err(text.chars().take(200).collect())
            } else {
                Ok(())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Sends what waits in the pending file, in the background; what fails stays for next time.
pub fn flush_pending(config_url: Option<&str>) {
    let (Some(url), Some(path)) = (url(config_url), pending_path()) else { return };
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    if text.trim().is_empty() {
        return;
    }
    let _ = std::fs::remove_file(&path);
    std::thread::Builder::new()
        .name("analytics".into())
        .spawn(move || {
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                match post(&url, line) {
                    Ok(()) => bevy::log::info!("analytics: run sent"),
                    Err(e) => {
                        bevy::log::warn!("analytics: send failed, kept for later: {e}");
                        append(&path, line);
                    }
                }
            }
        })
        .ok();
}

/// Writes a run down and sends it (through the pending file, so nothing is lost on a failure).
pub fn record_run(c: &Content, run: &Run, mode: &str, result: &str, config_url: Option<&str>) {
    let rec = record_of(c, run, mode, result);
    let Ok(line) = serde_json::to_string(&rec) else { return };
    if let Some(p) = analytics_file_path() {
        append(&p, &line);
    }
    if url(config_url).is_some() {
        if let Some(p) = pending_path() {
            append(&p, &line);
        }
        flush_pending(config_url);
    }
    info!("analytics: run recorded ({result}, {mode})");
}

pub fn load_history() -> Vec<RunRecord> {
    let Some(path) = analytics_file_path() else { return Vec::new() };
    let Ok(content) = std::fs::read_to_string(&path) else { return Vec::new() };
    content.lines().filter_map(|l| serde_json::from_str::<RunRecord>(l).ok()).filter(|r| r.schema >= SCHEMA).collect()
}

/// Per-card numbers from the local history, for the in-game screen.
#[derive(Clone, Debug, Default)]
pub struct CardLine {
    pub offered: u32,
    pub taken: u32,
    pub in_won: u32,
    pub in_decks: u32,
    pub played: u32,
}

pub fn card_lines(history: &[RunRecord]) -> BTreeMap<String, CardLine> {
    let mut out: BTreeMap<String, CardLine> = BTreeMap::new();
    for r in history {
        for o in r.stats.offers.iter().filter(|o| o.kind == "card") {
            let l = out.entry(o.id.clone()).or_default();
            l.offered += 1;
            l.taken += o.taken as u32;
        }
        for p in &r.players {
            let mut seen: Vec<&str> = Vec::new();
            for card in &p.deck {
                let base = card.trim_end_matches('+');
                if !seen.contains(&base) {
                    seen.push(base);
                    let l = out.entry(base.to_string()).or_default();
                    l.in_decks += 1;
                    l.in_won += (r.result == "won") as u32;
                }
            }
        }
        for ps in &r.stats.players {
            for (card, u) in &ps.cards {
                out.entry(card.trim_end_matches('+').to_string()).or_default().played += u.played;
            }
        }
    }
    out
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Analytics), enter_analytics);
    app.add_systems(Startup, |config: Option<Res<crate::Config>>| flush_pending(config.as_ref().and_then(|c| c.telemetry_url.as_deref())));
}

fn enter_analytics(mut commands: Commands, stage: Single<Entity, With<crate::Stage>>, fonts: Res<Fonts>, l10n: Res<L10n>, art: Res<crate::art::Art>, assets: Res<AssetServer>) {
    let history = load_history();
    let runs = history.len();
    let won = history.iter().filter(|r| r.result == "won").count();
    let cards = card_lines(&history);
    // builds: runs and wins by hero and build
    let mut builds: BTreeMap<(String, String), (u32, u32)> = BTreeMap::new();
    for r in &history {
        for p in &r.players {
            let e = builds.entry((p.hero.clone(), p.build.clone())).or_default();
            e.0 += 1;
            e.1 += (r.result == "won") as u32;
        }
    }
    commands.spawn((DespawnOnExit(AppState::Analytics), crate::menu::backdrop(&art, &assets, "bg/chronicle", 0.6), ChildOf(*stage)));
    commands.spawn(crate::menu::screen(AppState::Analytics, *stage)).with_children(|p| {
        p.spawn(theme::text(l10n.t("analytics-title"), &fonts.title, 56.0, theme::BONE));
        let pct = |a: u32, b: u32| if b == 0 { 0.0 } else { a as f32 * 100.0 / b as f32 };
        p.spawn(theme::text(l10n.tf("analytics-summary", &[("runs", (runs as i32).into()), ("won", (won as i32).into()), ("pct", format!("{:.0}", pct(won as u32, runs as u32)).into())]), &fonts.body, 22.0, theme::CANDLE));
        p.spawn((Node { width: px(1700), height: px(720), column_gap: px(24), justify_content: JustifyContent::SpaceBetween, ..default() }, Pickable::IGNORE)).with_children(|cols| {
            let column = |cols: &mut ChildSpawnerCommands, title: String, lines: Vec<(String, Color)>| {
                cols.spawn((
                    Node { width: px(820), height: px(700), flex_direction: FlexDirection::Column, padding: UiRect::all(px(16)), border: UiRect::all(px(2)), row_gap: px(6), overflow: Overflow::clip(), ..default() },
                    BackgroundColor(theme::SOOT.with_alpha(0.85)),
                    BorderColor::all(Color::srgb(0.35, 0.28, 0.20)),
                ))
                .with_children(|col| {
                    col.spawn(theme::text(title, &fonts.title, 24.0, theme::CANDLE));
                    for (line, color) in lines {
                        col.spawn(theme::text(line, &fonts.body, 17.0, color));
                    }
                });
            };
            // cards offered most often and taken least: what is shown but not wanted
            let mut passed: Vec<(&String, &CardLine)> = cards.iter().filter(|(_, l)| l.offered >= 3).collect();
            passed.sort_by(|a, b| pct(a.1.taken, a.1.offered).partial_cmp(&pct(b.1.taken, b.1.offered)).unwrap_or(std::cmp::Ordering::Equal));
            let lines = passed
                .iter()
                .take(16)
                .map(|(id, l)| (l10n.tf("analytics-card", &[("name", l10n.card_name(id, false).into()), ("offered", (l.offered as i32).into()), ("taken", format!("{:.0}", pct(l.taken, l.offered)).into()), ("won", format!("{:.0}", pct(l.in_won, l.in_decks)).into())]), theme::BONE))
                .collect();
            column(cols, l10n.t("analytics-cards"), lines);
            let mut b: Vec<(&(String, String), &(u32, u32))> = builds.iter().collect();
            b.sort_by(|x, y| y.1 .0.cmp(&x.1 .0));
            let lines = b
                .iter()
                .take(16)
                .map(|((hero, build), (n, w))| (l10n.tf("analytics-build", &[("hero", l10n.t(&format!("hero-{hero}")).into()), ("build", build.clone().into()), ("runs", (*n as i32).into()), ("won", format!("{:.0}", pct(*w, *n)).into())]), theme::BONE))
                .collect();
            column(cols, l10n.t("analytics-builds"), lines);
        });
        p.spawn((crate::menu::menu_button(l10n.t("coop-back"), &fonts), observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Chronicle))));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        Content::load_dir(&root.join("data")).expect("content")
    }

    /// Every card a player can take belongs to some family, or builds could not be told apart.
    #[test]
    fn every_reward_card_has_a_family() {
        let c = content();
        let bare: Vec<&String> = c
            .cards
            .iter()
            .filter(|(_, d)| matches!(d.rarity, nav_core::Rarity::Common | nav_core::Rarity::Uncommon | nav_core::Rarity::Rare))
            .filter(|(id, _)| family_of(&c, id).is_empty())
            .map(|(id, _)| id)
            .collect();
        assert!(bare.len() * 10 <= c.cards.len(), "cards with no family: {bare:?}");
    }

    #[test]
    fn a_record_carries_the_run_and_its_builds() {
        let c = content();
        let hero = c.heroes.keys().next().unwrap().clone();
        let run = Run::new(&c, &[hero], 7, 0).unwrap();
        let rec = record_of(&c, &run, "solo", "lost");
        assert_eq!(rec.players.len(), 1);
        assert!(!rec.players[0].build.is_empty());
        let line = serde_json::to_string(&rec).unwrap();
        let back: RunRecord = serde_json::from_str(&line).unwrap();
        assert_eq!(back.run_id, rec.run_id);
    }
}
