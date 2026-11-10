//! What a player keeps between runs: village currency, unlocked Circles of Nav,
//! hero experience. Stored per Steam account under the user's app-data folder,
//! in a plain RON file so that Steam Cloud can pick it up later.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Comfort and sound choices. None of them changes the rules.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Animation speed: 1.0, 1.5 or 2.0.
    pub anim_speed: f32,
    pub less_shake: bool,
    /// Lift resting cards slightly, while keeping them below the units' HP bars.
    pub raised_hand: bool,
    /// "Ink": blood is drawn as black ink.
    pub ink: bool,
    pub music: f32,
    /// Every sound that is not music or speech.
    pub sfx: f32,
    /// The storyteller reading the tale.
    pub narrator: f32,
    /// Heroes' and enemies' spoken lines.
    pub voices: f32,
    /// Language of the spoken word, chosen apart from the text's:
    /// 0 follows the text, 1 old Russian, 2 Russian, 3 English.
    pub voice_lang: u8,
    /// Language of the text, once the player has chosen one in the game:
    /// 0 not yet (see `main`), 1 old Russian, 2 Russian, 3 English.
    pub text_lang: u8,
    /// 0 a window, 1 the whole screen without a frame, 2 true full screen.
    pub screen: u8,
    /// The window's size, an index into `WINDOW_SIZES`.
    pub window_size: u8,
}

/// Sizes a window may take; the stage is drawn at 1920x1080 and fitted into any of them.
pub const WINDOW_SIZES: [(u32, u32); 5] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440), (3840, 2160)];

impl Default for Settings {
    fn default() -> Self {
        Settings { anim_speed: 1.0, less_shake: false, raised_hand: false, ink: false, music: 0.6, sfx: 0.8, narrator: 1.0, voices: 0.8, voice_lang: 0, text_lang: 0, screen: 0, window_size: 1 }
    }
}

/// One finished run, as the graveyard's chronicle remembers it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub hero: String,
    pub won: bool,
    pub floors: u32,
    pub act: u8,
    pub circle: u8,
}

/// How many runs the chronicle keeps.
pub const HISTORY: usize = 40;

#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// Village currency (Обереги).
    pub oberegi: u32,
    /// Highest Circle of Nav the player may choose; a win on it opens the next.
    pub circle_unlocked: u8,
    pub hero_xp: BTreeMap<String, u32>,
    pub runs: u32,
    pub wins: u32,
    /// Village buildings by id and the level each has reached.
    pub buildings: BTreeMap<String, u8>,
    /// Enemies met, by id, and in how many fights.
    pub bestiary: BTreeMap<String, u32>,
    /// The most recent runs, newest last.
    pub history: Vec<RunRecord>,
    /// The opening of the tale has been shown once.
    pub prologue_seen: bool,
    /// How the last run ended; the next one opens by remembering it.
    pub last_ending: Option<String>,
    /// Endings reached, by id, and how many times.
    pub endings: BTreeMap<String, u32>,
    /// Acts whose secret in the tale's book this player has ever peeled open (1..=4).
    pub secrets: Vec<u8>,
    pub settings: Settings,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Profile {
    /// Loads the profile for this account, or starts a fresh one.
    pub fn load(account: u64) -> Profile {
        let path = profile_dir().map(|d| d.join(format!("{account}.ron")));
        let mut profile: Profile = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| ron::from_str(&s).ok())
            .unwrap_or_default();
        profile.path = path;
        profile
    }

    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // write beside and rename, so a crash mid-write can't leave half a file
        let tmp = path.with_extension("ron.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            if let Err(e) = std::fs::rename(&tmp, path) {
                warn!("profile not saved: {e}");
            }
        }
    }

    pub fn building(&self, id: &str) -> u8 {
        self.buildings.get(id).copied().unwrap_or(0)
    }

    /// Smithy, herbalist's hut and tavern levels: the pools a run hosted by this player draws on.
    pub fn village(&self) -> (u8, u8, u8) {
        (self.building("smithy").min(2), self.building("hut").min(2), self.building("tavern").min(2))
    }

    /// Relics of the tale's book whose secrets this player has found: in a run they host, they
    /// turn up like any other relic.
    pub fn secret_relics(&self) -> Vec<String> {
        self.secrets.iter().filter_map(|a| nav_core::SECRET_RELICS.get(*a as usize - 1)).map(|r| r.to_string()).collect()
    }

    /// Notes a secret found; the chronicle counts them.
    pub fn note_secret(&mut self, act: u8) {
        if (1..=4).contains(&act) && !self.secrets.contains(&act) {
            self.secrets.push(act);
            self.secrets.sort();
        }
    }

    /// Price of the next level of a building, or `None` when it is finished.
    pub fn next_price(&self, id: &str) -> Option<u32> {
        let (_, max) = BUILDINGS.iter().find(|(b, _)| *b == id)?;
        let level = self.building(id);
        (level < *max).then(|| 60 + 70 * level as u32)
    }

    /// Spends village currency on the next level. Returns whether it was built.
    pub fn build(&mut self, id: &str) -> bool {
        let Some(price) = self.next_price(id) else { return false };
        if self.oberegi < price {
            return false;
        }
        self.oberegi -= price;
        *self.buildings.entry(id.to_string()).or_insert(0) += 1;
        self.save();
        true
    }

    /// Experience gathered towards the next level, and what that level takes; `None` at the top.
    pub fn hero_progress(&self, hero: &str) -> Option<(u32, u32)> {
        let mut left = self.hero_xp.get(hero).copied().unwrap_or(0);
        let mut need = 40;
        for _ in 1..10 {
            if left < need {
                return Some((left, need));
            }
            left -= need;
            need += 20;
        }
        None
    }

    /// Adds what a run in progress has earned so far, so leaving it midway loses none of it.
    pub fn bank(&mut self, hero: &str, oberegi: u32) {
        if oberegi == 0 {
            return;
        }
        self.oberegi += oberegi;
        *self.hero_xp.entry(hero.to_string()).or_insert(0) += oberegi;
        self.save();
    }

    /// Hero level 1..=10 from experience; each level takes a little more.
    pub fn hero_level(&self, hero: &str) -> u32 {
        let xp = self.hero_xp.get(hero).copied().unwrap_or(0);
        let mut level = 1;
        let mut need = 40;
        let mut left = xp;
        while level < 10 && left >= need {
            left -= need;
            level += 1;
            need += 20;
        }
        level
    }

    /// Books a finished run. Returns the Circle this win has just opened, if any.
    pub fn record_run(&mut self, hero: &str, oberegi: u32, won: bool, circle: u8, floors: u32, act: u8) -> Option<u8> {
        self.history.push(RunRecord { hero: hero.to_string(), won, floors, act, circle });
        if self.history.len() > HISTORY {
            let extra = self.history.len() - HISTORY;
            self.history.drain(..extra);
        }
        self.oberegi += oberegi;
        *self.hero_xp.entry(hero.to_string()).or_insert(0) += oberegi;
        self.runs += 1;
        let mut opened = None;
        if won {
            self.wins += 1;
            if circle >= self.circle_unlocked && self.circle_unlocked < 10 {
                self.circle_unlocked = circle + 1;
                opened = Some(self.circle_unlocked);
            }
        }
        self.save();
        opened
    }
}

impl Profile {
    /// Remembers the ending a run came to.
    pub fn note_ending(&mut self, id: &str) {
        *self.endings.entry(id.to_string()).or_insert(0) += 1;
        self.last_ending = Some(id.to_string());
        self.save();
    }

    fn run_path(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| p.with_extension("run.ron"))
    }

    /// Keeps the solo run in progress, so closing the game doesn't end it.
    pub fn save_run(&self, run: &nav_core::Run) {
        let Some(path) = self.run_path() else { return };
        let Ok(text) = ron::to_string(run) else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("ron.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    pub fn load_run(&self) -> Option<nav_core::Run> {
        let text = std::fs::read_to_string(self.run_path()?).ok()?;
        let run: nav_core::Run = ron::from_str(&text).ok()?;
        // a finished run is not something to continue
        (!matches!(run.stage, nav_core::Stage::Victory | nav_core::Stage::Defeat)).then_some(run)
    }

    fn party_path(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| p.with_extension("party.ron"))
    }

    /// Keeps the party's run as its host has it, after every fight and every step on the map, so
    /// a dropped game or an evening's end can be picked up again by the same friends.
    pub fn save_party(&self, save: &PartySave) {
        let Some(path) = self.party_path() else { return };
        let Ok(text) = ron::to_string(save) else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("ron.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    pub fn load_party(&self) -> Option<PartySave> {
        let text = std::fs::read_to_string(self.party_path()?).ok()?;
        let save: PartySave = ron::from_str(&text).ok()?;
        (!matches!(save.run.stage, nav_core::Stage::Victory | nav_core::Stage::Defeat)).then_some(save)
    }

    pub fn clear_party(&self) {
        if let Some(path) = self.party_path() {
            let _ = std::fs::remove_file(path);
        }
    }

    pub fn clear_run(&self) {
        if let Some(path) = self.run_path() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// A party's run as its host saved it: who sat where, how far the shared log had got, the run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartySave {
    pub lobby: nav_net::Lobby,
    pub n: u32,
    pub run: nav_core::Run,
}

/// Village buildings and how many levels each has. Only the ones that already do something
/// are for sale; the guest hall of the design waits for its art. The palisade changes nothing in a run:
/// it rebuilds the village the player comes back to, as the design asks.
pub const BUILDINGS: &[(&str, u8)] = &[("palisade", 5), ("smithy", 2), ("hut", 2), ("tavern", 2), ("shrine", 2), ("graveyard", 1)];

/// Writes every enemy the player meets into the bestiary, once per fight.
/// Runs the autopilot played, and dev shortcuts, leave no trace.
pub fn note_enemies(net: Option<Res<crate::net::Net>>, mut profile: ResMut<Profile>, mut seen: Local<(u64, std::collections::BTreeSet<u32>)>) {
    let Some(net) = net else { return };
    if net.bot_me || net.ephemeral {
        return;
    }
    let Some(run) = net.node.state.as_ref() else { return };
    let Some(combat) = run.combat.as_ref() else { return };
    let fight = run.floors_cleared as u64 * 8 + run.act as u64;
    if seen.0 != fight {
        *seen = (fight, Default::default());
    }
    for e in &combat.enemies {
        if seen.1.insert(e.uid) {
            *profile.bestiary.entry(e.def.clone()).or_insert(0) += 1;
        }
    }
}

fn profile_dir() -> Option<PathBuf> {
    crate::platform::user_dir().map(|d| d.join("profiles"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_win_opens_the_next_circle_only_from_the_highest() {
        let mut p = Profile::default();
        assert_eq!(p.record_run("bogatyr", 50, false, 0, 9, 1), None);
        assert_eq!(p.record_run("bogatyr", 150, true, 0, 9, 1), Some(1));
        assert_eq!(p.record_run("bogatyr", 150, true, 0, 9, 1), None, "an old circle opens nothing new");
        assert_eq!(p.record_run("volhv", 150, true, 1, 9, 1), Some(2));
        assert_eq!((p.oberegi, p.runs, p.wins, p.circle_unlocked), (500, 4, 3, 2));
        assert_eq!(p.history.len(), 4);
        assert_eq!(p.history[3], RunRecord { hero: "volhv".into(), won: true, floors: 9, act: 1, circle: 1 });
        for _ in 0..HISTORY + 5 {
            p.record_run("volhv", 0, false, 0, 1, 1);
        }
        assert_eq!(p.history.len(), HISTORY, "the chronicle forgets the oldest runs");
    }

    #[test]
    fn buildings_cost_more_each_level_and_stop_when_finished() {
        let mut p = Profile::default();
        p.oberegi = 200;
        assert_eq!(p.next_price("smithy"), Some(60));
        assert!(p.build("smithy"));
        assert_eq!((p.oberegi, p.building("smithy"), p.next_price("smithy")), (140, 1, Some(130)));
        assert!(p.build("smithy"));
        assert_eq!((p.oberegi, p.next_price("smithy")), (10, None));
        assert!(!p.build("smithy"), "a finished building takes no more");
        assert!(!p.build("hut"), "not enough currency");
        assert_eq!(p.village(), (2, 0, 0));
    }

    #[test]
    fn levels_climb_and_stop_at_ten() {
        let mut p = Profile::default();
        assert_eq!(p.hero_level("bogatyr"), 1);
        p.hero_xp.insert("bogatyr".into(), 40);
        assert_eq!(p.hero_level("bogatyr"), 2);
        p.hero_xp.insert("bogatyr".into(), 100);
        assert_eq!(p.hero_level("bogatyr"), 3);
        p.hero_xp.insert("bogatyr".into(), 1_000_000);
        assert_eq!(p.hero_level("bogatyr"), 10);
    }
}
