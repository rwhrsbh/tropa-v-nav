//! What happened in a run, kept as it goes, for the balance analytics: every fight (who was
//! fought, how long, what it cost), every card played (how often, what damage and block it
//! brought), and every card, relic and potion offered to a player, taken or passed over. The
//! run carries it in `Run::stats`; the game sends it when the run ends (see the game's
//! `analytics` module and `docs/analytics.md`).

use crate::run::{Run, RunAction, RunEvent, Stage};
use crate::state::{Event, UnitRef};
use crate::{Content, Id};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The run's stats as the run holds them: left out of its comparison and hash, so two machines
/// whose runs stand the same are the same whatever each has tallied (a client rebuilt from the
/// host's state lacks the tally's working notes).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Stats(pub RunStats);

impl PartialEq for Stats {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Stats {}
impl std::hash::Hash for Stats {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunStats {
    pub players: Vec<PlayerStats>,
    pub fights: Vec<FightStats>,
    /// Everything offered to a player to take, and whether they took it.
    pub offers: Vec<Offer>,
    /// Omens met and the option chosen.
    pub omens: Vec<OmenPick>,
    /// The card each player is playing out now, to credit the damage and block that follow it.
    #[serde(skip)]
    playing: Vec<Option<Id>>,
    /// The foe that last hurt each hero, to name who felled them.
    #[serde(skip)]
    last_hurt_by: Vec<Option<Id>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerStats {
    pub hero: Id,
    /// By card id (an upgraded card counts under its own id with a `+`).
    pub cards: BTreeMap<Id, CardUse>,
    pub damage_dealt: i32,
    pub damage_taken: i32,
    pub block_gained: i32,
    pub healed: i32,
    pub downed: u32,
    pub potions_used: BTreeMap<Id, u32>,
    pub gold_gained: i32,
    pub gold_spent: i32,
    /// Cards that came into the deck, were upgraded or taken out, in order: (what, card, act, floor).
    pub deck_changes: Vec<(String, Id, u8, u8)>,
    /// Relics as they came: (relic, act, floor, where from).
    pub relics_gained: Vec<(Id, u8, u8, String)>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CardUse {
    pub played: u32,
    /// Health taken off foes (block they had counts too: the blow was spent on it).
    pub damage: i32,
    pub block: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FightStats {
    pub act: u8,
    pub floor: u8,
    /// fight, elite or boss.
    pub kind: String,
    pub enemies: Vec<Id>,
    pub won: bool,
    pub over: bool,
    pub rounds: u32,
    /// Health each player lost in it (start minus end; healing makes it smaller).
    pub hp_lost: Vec<i32>,
    #[serde(skip)]
    hp_start: Vec<i32>,
    /// Damage to heroes by foe kind.
    pub damage_by: BTreeMap<Id, i32>,
    pub downed: u32,
    pub cards_played: u32,
    pub potions_used: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    pub act: u8,
    pub floor: u8,
    pub player: u8,
    /// card, relic or potion.
    pub kind: String,
    /// reward, shop, chest, trophy.
    pub source: String,
    pub id: Id,
    pub taken: bool,
    /// The price asked, in the shop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OmenPick {
    pub act: u8,
    pub floor: u8,
    pub id: Id,
    pub option: u8,
}

/// What the stats need to know from before an action is applied.
pub(crate) struct Before {
    stage: &'static str,
    offered_cards: Vec<Id>,
    bought: Option<(String, Id)>,
    omen: Option<Id>,
}

fn stage_name(s: &Stage) -> &'static str {
    match s {
        Stage::Map { .. } => "map",
        Stage::Combat => "combat",
        Stage::Reward { .. } => "reward",
        Stage::Rest { .. } => "rest",
        Stage::Shop { .. } => "shop",
        Stage::Chest(_) => "chest",
        Stage::Trophy { .. } => "trophy",
        Stage::Omen { .. } => "omen",
        _ => "other",
    }
}

impl Run {
    fn floor_now(&self) -> u8 {
        self.at.map_or(0, |(f, _)| f)
    }

    pub(crate) fn stats_before(&self, player: u8, action: &RunAction) -> Before {
        let p = player as usize;
        let mut b = Before { stage: stage_name(&self.stage), offered_cards: Vec::new(), bought: None, omen: None };
        match (action, &self.stage) {
            (RunAction::TakeCard(_), Stage::Reward { rewards }) => {
                b.offered_cards = rewards.get(p).filter(|r| !r.done).map(|r| r.cards.clone()).unwrap_or_default();
            }
            (RunAction::BuyCard(i) | RunAction::BuyRelic(i) | RunAction::BuyPotion(i), Stage::Shop { stock, .. }) => {
                let (kind, list) = match action {
                    RunAction::BuyCard(_) => ("card", &stock[p].cards),
                    RunAction::BuyRelic(_) => ("relic", &stock[p].relics),
                    _ => ("potion", &stock[p].potions),
                };
                if let Some(Some((id, _))) = list.get(*i as usize) {
                    b.bought = Some((kind.into(), id.clone()));
                }
            }
            (RunAction::Choose(_), Stage::Omen { id, .. }) => b.omen = Some(id.clone()),
            _ => {}
        }
        b
    }

    /// Notes what an action did. `ev` are the events it raised.
    pub(crate) fn stats_after(&mut self, c: &Content, player: u8, action: &RunAction, before: Before, ev: &[RunEvent]) {
        let p = player as usize;
        let (act, floor) = (self.act, self.floor_now());
        let n = self.players.len();
        let st = &mut self.stats.0;
        while st.players.len() < n {
            let hero = self.players[st.players.len()].hero.clone();
            st.players.push(PlayerStats { hero, ..Default::default() });
        }
        st.playing.resize(n, None);
        st.last_hurt_by.resize(n, None);

        // offers: a reward's cards when the choice is made, a shop's stock and a chest's or
        // trophy's relics as the party comes to them
        if !before.offered_cards.is_empty() {
            let took = match action {
                RunAction::TakeCard(Some(i)) => before.offered_cards.get(*i as usize).cloned(),
                _ => None,
            };
            for id in &before.offered_cards {
                st.offers.push(Offer { act, floor, player, kind: "card".into(), source: "reward".into(), id: id.clone(), taken: took.as_ref() == Some(id), price: None });
            }
        }
        if let Some((kind, id)) = &before.bought {
            if let Some(o) = st.offers.iter_mut().rev().find(|o| o.player == player && o.source == "shop" && &o.kind == kind && &o.id == id && !o.taken) {
                o.taken = true;
            }
        }
        let now = stage_name(&self.stage);
        if now != before.stage {
            match &self.stage {
                Stage::Shop { stock, .. } => {
                    for (q, s) in stock.iter().enumerate() {
                        for (kind, list) in [("card", &s.cards), ("relic", &s.relics), ("potion", &s.potions)] {
                            for (id, price) in list.iter().flatten() {
                                st.offers.push(Offer { act, floor, player: q as u8, kind: kind.into(), source: "shop".into(), id: id.clone(), taken: false, price: Some(*price) });
                            }
                        }
                    }
                }
                Stage::Chest(ch) => {
                    for q in 0..n {
                        for id in &ch.offers {
                            st.offers.push(Offer { act, floor, player: q as u8, kind: "relic".into(), source: "chest".into(), id: id.clone(), taken: false, price: None });
                        }
                    }
                }
                Stage::Trophy { offers, .. } => {
                    for (q, list) in offers.iter().enumerate() {
                        for id in list {
                            st.offers.push(Offer { act, floor, player: q as u8, kind: "relic".into(), source: "trophy".into(), id: id.clone(), taken: false, price: None });
                        }
                    }
                }
                _ => {}
            }
        }
        if let (Some(id), RunAction::Choose(option)) = (&before.omen, action) {
            st.omens.push(OmenPick { act, floor, id: id.clone(), option: *option });
        }

        let enemy_def = |uid: u32| self.combat.as_ref().and_then(|k| k.enemy(uid)).map(|e| e.def.clone());
        for e in ev {
            match e {
                RunEvent::CombatStarted => {
                    let kind = match self.fighting {
                        Some(crate::NodeKind::Elite) => "elite",
                        Some(crate::NodeKind::Boss) => "boss",
                        _ => "fight",
                    };
                    let enemies = self.combat.as_ref().map(|k| k.enemies.iter().map(|e| e.def.clone()).collect()).unwrap_or_default();
                    let hp_start = self.players.iter().map(|pl| pl.hp).collect();
                    st.fights.push(FightStats { act, floor, kind: kind.into(), enemies, hp_start, ..Default::default() });
                    st.playing.iter_mut().for_each(|c| *c = None);
                }
                RunEvent::RelicGained { player: q, relic } => {
                    let from = match before.stage {
                        "reward" | "combat" => "elite",
                        other => other,
                    };
                    if let Some(ps) = st.players.get_mut(*q as usize) {
                        ps.relics_gained.push((relic.clone(), act, floor, from.into()));
                    }
                    if let Some(o) = st.offers.iter_mut().rev().find(|o| o.player == *q && o.kind == "relic" && &o.id == relic && !o.taken && (o.source == "chest" || o.source == "trophy")) {
                        o.taken = true;
                    }
                }
                RunEvent::CardTaken { player: q, card: Some(card) } | RunEvent::CardGained { player: q, card } => {
                    if let Some(ps) = st.players.get_mut(*q as usize) {
                        ps.deck_changes.push(("gained".into(), card.clone(), act, floor));
                    }
                }
                RunEvent::Upgraded { player: q, card } => {
                    if let Some(ps) = st.players.get_mut(*q as usize) {
                        ps.deck_changes.push(("upgraded".into(), card.clone(), act, floor));
                    }
                }
                RunEvent::CardRemoved { player: q, card } => {
                    if let Some(ps) = st.players.get_mut(*q as usize) {
                        ps.deck_changes.push(("removed".into(), card.clone(), act, floor));
                    }
                }
                RunEvent::Gold { player: q, delta } => {
                    if let Some(ps) = st.players.get_mut(*q as usize) {
                        if *delta > 0 {
                            ps.gold_gained += delta;
                        } else {
                            ps.gold_spent -= delta;
                        }
                    }
                }
                RunEvent::Combat(ce) => {
                    let fight = st.fights.last_mut().filter(|f| !f.over);
                    match ce {
                        Event::RoundStart(r) => {
                            if let Some(f) = fight {
                                f.rounds = *r;
                            }
                            st.playing.iter_mut().for_each(|c| *c = None);
                        }
                        Event::EnemyPhase => st.playing.iter_mut().for_each(|c| *c = None),
                        Event::CardPlayed { player: q, card, upgraded, .. } => {
                            let id = if *upgraded { format!("{card}+") } else { card.clone() };
                            if let Some(ps) = st.players.get_mut(*q as usize) {
                                ps.cards.entry(id.clone()).or_default().played += 1;
                            }
                            if let Some(slot) = st.playing.get_mut(*q as usize) {
                                *slot = Some(id);
                            }
                            if let Some(f) = fight {
                                f.cards_played += 1;
                            }
                        }
                        Event::Damage { src, dst, blocked, lost, .. } => match (src, dst) {
                            (Some(UnitRef::Hero(q)), UnitRef::Enemy(_)) => {
                                let dealt = lost + blocked;
                                if let Some(ps) = st.players.get_mut(*q as usize) {
                                    ps.damage_dealt += dealt;
                                    if let Some(Some(card)) = st.playing.get(*q as usize) {
                                        ps.cards.entry(card.clone()).or_default().damage += dealt;
                                    }
                                }
                            }
                            (Some(UnitRef::Enemy(uid)), UnitRef::Hero(q)) => {
                                let def = enemy_def(*uid);
                                if let Some(ps) = st.players.get_mut(*q as usize) {
                                    ps.damage_taken += lost;
                                }
                                if let (Some(def), Some(f)) = (def.clone(), fight) {
                                    *f.damage_by.entry(def).or_default() += lost;
                                }
                                if let Some(slot) = st.last_hurt_by.get_mut(*q as usize) {
                                    *slot = def;
                                }
                            }
                            (_, UnitRef::Hero(q)) => {
                                if let Some(ps) = st.players.get_mut(*q as usize) {
                                    ps.damage_taken += lost;
                                }
                            }
                            _ => {}
                        },
                        Event::Block { unit: UnitRef::Hero(q), amount, .. } if *amount > 0 => {
                            if let Some(ps) = st.players.get_mut(*q as usize) {
                                ps.block_gained += amount;
                                if let Some(Some(card)) = st.playing.get(*q as usize) {
                                    ps.cards.entry(card.clone()).or_default().block += amount;
                                }
                            }
                        }
                        Event::Heal { unit: UnitRef::Hero(q), amount, .. } => {
                            if let Some(ps) = st.players.get_mut(*q as usize) {
                                ps.healed += amount;
                            }
                        }
                        Event::Downed { player: q } => {
                            if let Some(ps) = st.players.get_mut(*q as usize) {
                                ps.downed += 1;
                            }
                            if let Some(f) = fight {
                                f.downed += 1;
                            }
                        }
                        Event::PotionUsed { player: q, potion, .. } => {
                            if let Some(ps) = st.players.get_mut(*q as usize) {
                                *ps.potions_used.entry(potion.clone()).or_default() += 1;
                            }
                            if let Some(f) = fight {
                                f.potions_used += 1;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        // a fight is closed once it is over, won or lost
        let phase = self.combat.as_ref().map(|k| k.phase);
        if let Some(f) = st.fights.last_mut().filter(|f| !f.over) {
            let done = match phase {
                Some(crate::Phase::Won) => Some(true),
                Some(crate::Phase::Lost) => Some(false),
                None => Some(!matches!(self.stage, Stage::Defeat)),
                _ => None,
            };
            if let Some(won) = done {
                f.over = true;
                f.won = won;
                f.hp_lost = f.hp_start.iter().zip(&self.players).map(|(a, pl)| a - pl.hp.max(0)).collect();
            }
        }
        let _ = (c, p);
    }

    /// The foe that felled a hero last, by kind, for a run lost.
    pub fn felled_by(&self) -> Option<Id> {
        self.stats.0.last_hurt_by.iter().flatten().last().cloned().or_else(|| {
            let f = self.stats.0.fights.last()?;
            f.damage_by.iter().max_by_key(|(_, d)| **d).map(|(k, _)| k.clone())
        })
    }
}
