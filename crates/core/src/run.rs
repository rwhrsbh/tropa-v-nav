//! A whole run: the path map, votes, fights, rewards and rest. Like combat,
//! it is a deterministic reducer, so every machine in a party holds the same run.

use crate::combat;
use crate::data::*;
use crate::rng::Rng;
use crate::state::*;
use serde::{Deserialize, Serialize};

pub const FLOORS: usize = 15;
pub const MAP_WIDTH: u8 = 7;
pub const POTION_SLOTS: usize = 3;
/// Is this one short of a head, and still carrying it? A head thrown away cannot be sewn back on.
fn head_to_sew(pl: &Player) -> bool {
    (pl.beheaded || pl.walks_headless) && carries(pl, &format!("own_head_{}", pl.hero))
}

/// The arm this one lacks and still carries, the weapon arm first, with its card's name.
fn arm_to_sew(pl: &Player) -> Option<(u8, String)> {
    use crate::combat::{SHIELD_ARM, WEAPON_ARM};
    [(WEAPON_ARM, "weapon"), (SHIELD_ARM, "shield")].into_iter().find_map(|(arm, word)| {
        let card = format!("own_{word}_{}", pl.hero);
        (pl.lost_arms & arm != 0 && carries(pl, &card)).then_some((arm, card))
    })
}

fn carries(pl: &Player, card: &str) -> bool {
    pl.deck.iter().any(|(id, _)| id.as_str() == card)
}

/// Takes one card by this name out of the deck.
fn take_from_deck(pl: &mut Player, card: &str) {
    if let Some(i) = pl.deck.iter().position(|(id, _)| id.as_str() == card) {
        pl.deck.remove(i);
    }
}

/// Whether the player in seat `p` can have this option of an omen as far as sewing goes: there must
/// be something to sew, and she sews one thing a visit. Any other option is open.
pub fn seam_open(players: &[Player], def: &EventDef, option: &EventOption, p: usize, picks: &[Option<u8>]) -> bool {
    let sews = |o: &EventOption| o.fx.iter().any(|f| matches!(f, RunFx::SewHeadOn | RunFx::SewLimbOn));
    if !sews(option) {
        return true;
    }
    let taken = picks.iter().enumerate().any(|(q, v)| q != p && v.is_some_and(|i| def.options.get(i as usize).is_some_and(sews)));
    let me = &players[p];
    let target = (option.fx.contains(&RunFx::SewHeadOn) && players.iter().any(head_to_sew)) || (option.fx.contains(&RunFx::SewLimbOn) && arm_to_sew(me).is_some());
    !me.beheaded && !taken && target
}

/// Percent of max HP that one whose head is sewn back on stands up with.
pub const RAISED_PCT: i32 = 25;
/// Water raises a body but sews nothing: it walks headless on this percent of its health.
pub const WATER_PCT: i32 = 10;
/// Percent of max HP a head put back costs for good, and a limb sewn back on.
pub const HEAD_SEAM_PCT: i32 = 35;
pub const LIMB_SEAM_PCT: i32 = 20;
/// However often they are sewn, nobody is left with less to lose than this.
const LEAST_MAX_HP: i32 = 10;
/// Percent chance an omen on the road is the seamstress, while someone is missing a part.
pub const SEAMSTRESS_CHANCE: i32 = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NodeKind {
    Fight,
    Elite,
    Event,
    Rest,
    Shop,
    Chest,
    Boss,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MapNode {
    pub kind: NodeKind,
    /// Column 0..MAP_WIDTH, for drawing.
    pub col: u8,
    /// Indices into the next floor this node leads to.
    pub next: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Map {
    pub floors: Vec<Vec<MapNode>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Player {
    pub hero: Id,
    pub hp: i32,
    pub max_hp: i32,
    /// Gold is personal: every player has their own purse.
    pub gold: i32,
    /// `(card id, upgraded)`
    pub deck: Vec<(Id, bool)>,
    pub relics: Vec<Id>,
    pub potions: Vec<Id>,
    pub falls: u32,
    #[serde(default)]
    pub lost_limbs: u8,
    /// Which: `combat::WEAPON_ARM` and `combat::SHIELD_ARM` as bits.
    #[serde(default)]
    pub lost_arms: u8,
    /// Head struck off: this seat only watches from here on.
    #[serde(default)]
    pub beheaded: bool,
    /// Raised by water alone: up and fighting, with no head and a tenth of their health.
    #[serde(default)]
    pub walks_headless: bool,
    /// What their max HP was before the water, for the seamstress to work from; 0 if it was not.
    #[serde(default)]
    pub whole_max_hp: i32,
    /// Cards removed at the shop so far; each next removal costs more.
    pub removals: u32,
}

/// What one player is offered after a fight or at a chest.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Reward {
    pub gold: i32,
    pub cards: Vec<Id>,
    pub relic: Option<Id>,
    pub done: bool,
}

/// One player's shelves at the pedlar: `(id, price)`; a sold item becomes `None`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stock {
    pub cards: Vec<Option<(Id, i32)>>,
    pub relics: Vec<Option<(Id, i32)>>,
    pub potions: Vec<Option<(Id, i32)>>,
    /// The player has already thrown a potion at the pedlar this visit.
    #[serde(default)]
    pub pelted: bool,
}

/// A chest with a relic for every player. Each claims one; those who claimed the same one throw
/// rock-paper-scissors for it, and whoever loses takes one of the relics nobody claimed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Chest {
    pub offers: Vec<Id>,
    /// The offer each player has claimed; `LOST` once they have lost the throw for it.
    pub picks: Vec<Option<u8>>,
    /// The offer each player has ended up with.
    pub won: Vec<Option<u8>>,
    /// Players still throwing for the contested offer `prize`.
    pub contest: Vec<u8>,
    pub prize: Option<u8>,
    /// This round's throws (0 rock, 1 paper, 2 scissors), per player.
    pub throws: Vec<Option<u8>>,
    /// The round thrown last, `(player, shape)`, for the screen to show.
    pub last: Vec<(u8, u8)>,
    pub rounds: u32,
    /// The chest is shared out and the relics handed over.
    pub settled: bool,
    /// Players who have looked their fill and want to go on.
    pub done: Vec<bool>,
}

impl Chest {
    /// The claim of a player who lost the throw for what they claimed.
    pub const LOST: u8 = u8::MAX;
}

/// Which throw beats the other: rock blunts scissors, paper wraps rock, scissors cut paper.
pub fn beats(a: u8, b: u8) -> bool {
    (a + 2) % 3 == b
}

/// What the pedlar pays, flustered, for a potion broken over his head.
pub const PELT_GOLD: i32 = 25;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    /// Choosing the next node; `votes[player]` indexes into [`Run::choices`].
    Map { votes: Vec<Option<u8>> },
    Combat,
    Reward { rewards: Vec<Reward> },
    Rest { done: Vec<bool> },
    /// The pedlar: every player has their own shelves.
    Shop { stock: Vec<Stock>, done: Vec<bool> },
    /// An omen. `picks[player]` is the chosen option (a vote, for vote omens).
    Omen { id: Id, picks: Vec<Option<u8>> },
    /// A boss trophy: `offers[player]` are that player's own to take one of.
    Trophy { offers: Vec<Vec<Id>>, picks: Vec<Option<u8>> },
    /// A chest shared by the party: a relic for every player.
    Chest(Chest),
    Victory,
    Defeat,
}

/// What the party's village and hero levels have opened. It widens variety,
/// never power: nothing here makes a hero stronger at the start of a run.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Unlocks {
    /// Smithy level 0..=2: common relics, then uncommon, then rare.
    pub relic_tier: u8,
    /// Herbalist's hut level 0..=2: ten potions per level.
    pub potion_tier: u8,
    /// Tavern level 0..=2: twenty omens, then fifteen more per level.
    pub omen_tier: u8,
    /// Hero level of each player, in slot order: uncommon cards from 2, rare from 4.
    pub hero_levels: Vec<u32>,
    /// Which starting relic each player took, in slot order: 0 is the hero's own,
    /// 1 and 2 are the shrine's alternatives.
    #[serde(default)]
    pub starters: Vec<u8>,
    /// Relics of the tale's book whose secret someone has once found: from then on they turn up
    /// like any other relic.
    #[serde(default)]
    pub secrets: Vec<Id>,
}

impl Unlocks {
    /// Everything open: tests, the simulator and dev shortcuts.
    pub fn all(players: usize) -> Unlocks {
        Unlocks { relic_tier: 2, potion_tier: 2, omen_tier: 2, hero_levels: vec![10; players], starters: vec![], secrets: vec![] }
    }

    fn relic_ok(&self, rarity: Rarity) -> bool {
        match rarity {
            Rarity::Rare => self.relic_tier >= 2,
            Rarity::Uncommon => self.relic_tier >= 1,
            _ => true,
        }
    }

    fn card_ok(&self, player: usize, def: &CardDef) -> bool {
        let level = self.hero_levels.get(player).copied().unwrap_or(1);
        level >= card_level(def) as u32
    }
}

/// The hero level that opens a card to the rewards: its own, or the one its rarity asks.
/// Percent of the health lost that a hero gets back on coming to a new act.
pub const ACT_HEAL_PCT: i32 = 80;

pub fn card_level(def: &CardDef) -> u8 {
    if def.level > 0 {
        return def.level;
    }
    match def.rarity {
        Rarity::Rare => 4,
        Rarity::Uncommon => 2,
        _ => 1,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Run {
    pub seed: u64,
    pub rng: Rng,
    /// Circle of Nav, 0..=10.
    pub circle: u8,
    pub unlocks: Unlocks,
    pub act: u8,
    pub map: Map,
    /// `(floor, node)` the party stands on; `None` before the first step of an act.
    pub at: Option<(u8, u8)>,
    /// Every `(floor, node)` the party has stood on in this act, in order.
    #[serde(default)]
    pub trail: Vec<(u8, u8)>,
    pub players: Vec<Player>,
    pub stage: Stage,
    pub combat: Option<Combat>,
    /// What is being fought, for rewards.
    pub fighting: Option<NodeKind>,
    /// Tallies for the village currency at the end.
    pub floors_cleared: u32,
    pub elites_slain: u32,
    pub bosses_slain: u32,
    /// Acts whose secret in the tale's book has been found this run.
    #[serde(default)]
    pub secrets: Vec<u8>,
    /// Omens already met this run; none comes twice while others are left.
    #[serde(default)]
    pub omens_seen: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunAction {
    /// Put your token on a next node (index into [`Run::choices`]).
    Vote(u8),
    Combat(CombatAction),
    /// Take one of the offered cards, or `None` to skip.
    TakeCard(Option<u8>),
    /// At the fire: sleep.
    Sleep,
    /// At the fire: upgrade the card at this deck index.
    Upgrade(u16),
    /// At the fire: spend the rest on dead water and living water, for an ally without a head.
    Raise { who: u8 },
    /// Hand some of your gold to an ally (shop and fire only).
    GiveGold { to: u8, amount: i32 },
    BuyCard(u8),
    BuyRelic(u8),
    BuyPotion(u8),
    /// Pay the pedlar to take a card out of your deck for good.
    RemoveCard(u16),
    LeaveShop,
    /// Choose (or vote for) an option of the omen.
    Choose(u8),
    /// Throw away a potion to free the slot.
    DropPotion(u8),
    /// At a contested chest: rock (0), paper (1) or scissors (2).
    Throw(u8),
    /// Done looking at the shared-out chest.
    Onward,
    /// Drink a healing potion between fights: only its healing works there.
    DrinkPotion(u8),
    /// Break a potion over the pedlar's head: he pays to be left alone (once a visit).
    PeltPedlar(u8),
    /// The player tore a page of the tale's book, losing this share of the leaf (percent).
    /// Whatever is torn from the tale is torn from the one who held it.
    TearPage { lost: u8 },
    /// The player peeled a glued leaf of the tale's book apart, whole, and read the secret of
    /// this act (1..=4): its relic is theirs.
    FindSecret { act: u8 },
    /// The player shut the book before the tale's chapter was done.
    SkipTale,
}

/// What a potion heals when drunk between fights: `(amount, everyone)`. Potions that do
/// nothing but fight have none, and cannot be drunk there.
pub fn out_of_fight_heals(effects: &[Effect]) -> Vec<(i32, bool)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Heal(Amount::N(n), t) => Some((*n, *t == Tgt::AllFriends)),
            _ => None,
        })
        .collect()
}

/// The relics hidden in the tale's book, one per act's secret.
pub const SECRET_RELICS: [&str; 5] = ["kotomka", "erased_name", "ice_tale", "unheard_tale", "alatyr_heart"];
/// What the unheard tale gives each time the book is shut early.
pub const UNHEARD_GOLD: i32 = 15;

/// The curse a torn page of the tale lays on whoever tore it.
pub const BOOK_SPOIL: &str = "book_spoil";
/// A tear that takes more than this share of the leaf (percent) also costs max HP...
pub const BIG_TEAR: u8 = 25;
/// ...this much of it, for the rest of the run.
pub const BIG_TEAR_HP: i32 = 5;

/// At a schism the party must part: when every other player has gone the same way,
/// that way is closed to the last one. `None` alone, or while the others differ or still think.
pub fn schism_closed(picks: &[Option<u8>], p: usize) -> Option<u8> {
    let mut others = picks.iter().enumerate().filter(|(i, _)| *i != p).map(|(_, v)| *v);
    let first = others.next()??;
    others.all(|v| v == Some(first)).then_some(first)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunError {
    Combat(ActionError),
    WrongStage,
    NoSuchPlayer,
    BadChoice,
    AlreadyDone,
    NotEnoughGold,
    /// A schism: the others all went that way, so this player must take another.
    MustSplit,
    SoldOut,
    NoRoom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunEvent {
    /// `who` stands again, their head back on, by `player`'s doing.
    Raised { player: u8, who: u8 },
    /// `player` has one limb more, sewn on.
    LimbSewn { player: u8 },
    Combat(Event),
    Voted { player: u8, choice: u8 },
    /// A tied vote was settled by lot among `tied`.
    Lot { tied: Vec<u8>, chosen: u8 },
    Moved { floor: u8, node: u8, kind: NodeKind },
    CombatStarted,
    Rewards,
    CardTaken { player: u8, card: Option<Id> },
    Rested { player: u8, healed: i32 },
    Upgraded { player: u8, card: Id },
    Gold { player: u8, delta: i32 },
    RelicGained { player: u8, relic: Id },
    PotionGained { player: u8, potion: Id },
    CardGained { player: u8, card: Id },
    CardRemoved { player: u8, card: Id },
    /// A round of rock-paper-scissors at the chest: `(player, shape)` for everyone who threw.
    Thrown { throws: Vec<(u8, u8)> },
    Hp { player: u8, delta: i32 },
    OmenResolved { option: u8 },
    ActCleared { act: u8 },
    Ended { won: bool },
}

/// Rules that change with the Circle of Nav; effects stack as the circle rises.
#[derive(Clone, Copy, Debug)]
pub struct Circle(pub u8);

impl Circle {
    pub fn elite_weight(self) -> u32 {
        if self.0 >= 1 { 12 } else { 8 }
    }
    pub fn enemy_dmg(self, kind: NodeKind) -> i32 {
        match kind {
            NodeKind::Boss if self.0 >= 4 => 115,
            NodeKind::Fight if self.0 >= 2 => 110,
            _ => 100,
        }
    }
    pub fn rest_percent(self) -> i32 {
        if self.0 >= 5 { 20 } else { 30 }
    }
    pub fn starts_with_toska(self) -> bool {
        self.0 >= 6
    }
    pub fn gold_percent(self) -> i32 {
        if self.0 >= 7 { 75 } else { 100 }
    }
    pub fn enemy_hp(self) -> i32 {
        if self.0 >= 8 { 110 } else { 100 }
    }
    pub fn koschei_shadow(self) -> bool {
        self.0 >= 10
    }
    pub fn raise_cost(self) -> i32 {
        if self.0 >= 9 { 3 } else { 2 }
    }
}

pub fn generate_map(rng: &mut Rng, circle: Circle) -> Map {
    // six walkers leave the bottom and drift up; where they tread there are nodes
    let mut cols: Vec<Vec<u8>> = vec![Vec::new(); FLOORS];
    let mut edges: Vec<Vec<(u8, u8)>> = vec![Vec::new(); FLOORS];
    for _walker in 0..6 {
        let mut col = rng.below(MAP_WIDTH as u32) as u8;
        for floor in 0..FLOORS {
            if !cols[floor].contains(&col) {
                cols[floor].push(col);
            }
            if floor + 1 < FLOORS {
                let step = rng.range(-1, 1);
                let mut to = (col as i32 + step).clamp(0, MAP_WIDTH as i32 - 1) as u8;
                // paths may merge and split but never cross each other
                let crosses = edges[floor].iter().any(|(a, b)| (*a < col && *b > to) || (*a > col && *b < to));
                if crosses {
                    to = col;
                }
                if !edges[floor].contains(&(col, to)) {
                    edges[floor].push((col, to));
                }
                col = to;
            }
        }
    }
    for c in &mut cols {
        c.sort();
    }
    let mut floors: Vec<Vec<MapNode>> = Vec::new();
    for floor in 0..FLOORS {
        let nodes = cols[floor]
            .iter()
            .map(|col| {
                let mut next: Vec<u8> = edges[floor]
                    .iter()
                    .filter(|(a, _)| a == col)
                    .filter_map(|(_, b)| cols.get(floor + 1).and_then(|n| n.iter().position(|x| x == b)).map(|i| i as u8))
                    .collect();
                next.sort();
                next.dedup();
                MapNode { kind: NodeKind::Fight, col: *col, next }
            })
            .collect();
        floors.push(nodes);
    }
    // kinds: fixed floors first, the rest by share (design doc, section 5)
    for floor in 0..FLOORS {
        for i in 0..floors[floor].len() {
            let kind = match floor {
                0 => NodeKind::Fight,
                8 => NodeKind::Chest,
                14 => NodeKind::Rest,
                _ => {
                    let parents: Vec<NodeKind> =
                        if floor == 0 { vec![] } else { floors[floor - 1].iter().filter(|p| p.next.contains(&(i as u8))).map(|p| p.kind).collect() };
                    let mut pick = NodeKind::Fight;
                    for _ in 0..8 {
                        let mut bag: Vec<(NodeKind, u32)> = vec![(NodeKind::Fight, 45), (NodeKind::Event, 22)];
                        // no fire right before the guaranteed one
                        if floor != 13 {
                            bag.push((NodeKind::Rest, 12));
                        }
                        if floor >= 5 {
                            bag.push((NodeKind::Elite, circle.elite_weight()));
                        }
                        if floor >= 2 {
                            bag.push((NodeKind::Shop, 6));
                        }
                        let total: u32 = bag.iter().map(|b| b.1).sum();
                        let mut roll = rng.below(total);
                        for (k, w) in bag {
                            if roll < w {
                                pick = k;
                                break;
                            }
                            roll -= w;
                        }
                        // two fires, shops or elites in a row make a dull path
                        let special = matches!(pick, NodeKind::Rest | NodeKind::Shop | NodeKind::Elite);
                        if !(special && parents.contains(&pick)) {
                            break;
                        }
                        pick = NodeKind::Fight;
                    }
                    pick
                }
            };
            floors[floor][i].kind = kind;
        }
    }
    Map { floors }
}

/// Generates a 6-floor map for Act 4 (Buyan Island) leading to the final battle with Koschei.
pub fn generate_buyan_map(_rng: &mut Rng, _circle: Circle) -> Map {
    let f0 = vec![
        MapNode { kind: NodeKind::Fight, col: 2, next: vec![0, 1] },
        MapNode { kind: NodeKind::Fight, col: 4, next: vec![0, 1] },
    ];
    let f1 = vec![
        MapNode { kind: NodeKind::Event, col: 2, next: vec![0, 1] },
        MapNode { kind: NodeKind::Event, col: 4, next: vec![0, 1] },
    ];
    let f2 = vec![
        MapNode { kind: NodeKind::Elite, col: 2, next: vec![0, 1] },
        MapNode { kind: NodeKind::Elite, col: 4, next: vec![0, 1] },
    ];
    let f3 = vec![
        MapNode { kind: NodeKind::Shop, col: 2, next: vec![0, 1] },
        MapNode { kind: NodeKind::Rest, col: 4, next: vec![0, 1] },
    ];
    let f4 = vec![
        MapNode { kind: NodeKind::Fight, col: 2, next: vec![0] },
        MapNode { kind: NodeKind::Event, col: 4, next: vec![0] },
    ];
    let f5 = vec![
        MapNode { kind: NodeKind::Rest, col: 3, next: vec![] },
    ];
    Map { floors: vec![f0, f1, f2, f3, f4, f5] }
}

impl Run {
    /// A run with everything unlocked.
    pub fn new(c: &Content, heroes: &[Id], seed: u64, circle: u8) -> Option<Run> {
        Run::new_with(c, heroes, seed, circle, Unlocks::all(heroes.len()))
    }

    pub fn new_with(c: &Content, heroes: &[Id], seed: u64, circle: u8, unlocks: Unlocks) -> Option<Run> {
        let mut rng = Rng::new(seed);
        let circle_rules = Circle(circle);
        let mut players = Vec::new();
        for (slot, h) in heroes.iter().enumerate() {
            let d = c.heroes.get(h)?;
            let relic = match unlocks.starters.get(slot).copied().unwrap_or(0) {
                0 => &d.relic,
                k => d.alt_relics.get(k as usize - 1).filter(|id| c.relics.contains_key(*id)).unwrap_or(&d.relic),
            };
            let mut deck: Vec<(Id, bool)> = d.deck.iter().map(|id| (id.clone(), false)).collect();
            if circle_rules.starts_with_toska() && c.cards.contains_key("toska") {
                deck.push(("toska".into(), false));
            }
            players.push(Player { hero: h.clone(), hp: d.max_hp, max_hp: d.max_hp, gold: 99, deck, relics: vec![relic.clone()], potions: vec![], falls: 0, lost_limbs: 0, lost_arms: 0, beheaded: false, walks_headless: false, whole_max_hp: 0, removals: 0 });
        }
        let map = generate_map(&mut rng, circle_rules);
        let n = players.len();
        Some(Run {
            seed,
            rng,
            circle,
            unlocks,
            act: 1,
            map,
            at: None,
            trail: vec![],
            players,
            stage: Stage::Map { votes: vec![None; n] },
            combat: None,
            fighting: None,
            floors_cleared: 0,
            elites_slain: 0,
            bosses_slain: 0,
            secrets: vec![],
            omens_seen: vec![],
        })
    }

    pub fn rules(&self) -> Circle {
        Circle(self.circle)
    }

    /// Nodes the party may step to next: `(node index on that floor, kind)`.
    /// Empty when the only way on is the boss.
    pub fn choices(&self) -> Vec<(u8, NodeKind)> {
        // the last stretch has no map: only Koschei waits
        if self.map.floors.is_empty() {
            return vec![(0, NodeKind::Boss)];
        }
        match self.at {
            None => self.map.floors[0].iter().enumerate().map(|(i, n)| (i as u8, n.kind)).collect(),
            Some((floor, node)) => {
                let next = floor as usize + 1;
                if next >= self.map.floors.len() {
                    return vec![(0, NodeKind::Boss)];
                }
                self.map.floors[floor as usize][node as usize].next.iter().map(|i| (*i, self.map.floors[next][*i as usize].kind)).collect()
            }
        }
    }

    /// Village currency earned so far (design doc, section 8).
    pub fn oberegi(&self) -> u32 {
        let won = if self.stage == Stage::Victory { 100 } else { 0 };
        self.floors_cleared + self.elites_slain * 10 + self.bosses_slain * 30 + won
    }

    pub fn hash64(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        struct Fnv(u64);
        impl Hasher for Fnv {
            fn finish(&self) -> u64 {
                self.0
            }
            fn write(&mut self, bytes: &[u8]) {
                for b in bytes {
                    self.0 ^= *b as u64;
                    self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
                }
            }
        }
        let mut h = Fnv(0xCBF2_9CE4_8422_2325);
        self.hash(&mut h);
        h.finish()
    }

    /// Applies one player's action. On `Err` the run is untouched.
    pub fn apply(&mut self, c: &Content, player: u8, action: &RunAction) -> Result<Vec<RunEvent>, RunError> {
        // the headless only watch: the pass below says for them what must be said
        if self.players.get(player as usize).is_some_and(|p| p.beheaded) && !matches!(action, RunAction::Combat(_) | RunAction::TearPage { .. }) {
            return Err(RunError::WrongStage);
        }
        let mut ev = self.apply_one(c, player, action)?;
        self.the_headless_pass(c, &mut ev);
        Ok(ev)
    }

    /// Whoever has lost their head chooses nothing any more: what they would have had to say is
    /// said for them, and a road goes the way the first of the living points.
    fn the_headless_pass(&mut self, c: &Content, ev: &mut Vec<RunEvent>) {
        // whatever else went round, nothing has healed a corpse
        for pl in self.players.iter_mut().filter(|pl| pl.beheaded) {
            pl.hp = 0;
        }
        for _ in 0..32 {
            let mut acted = false;
            for q in 0..self.players.len() {
                if !self.players[q].beheaded {
                    continue;
                }
                let action = match &self.stage {
                    Stage::Map { votes } if votes[q].is_none() => {
                        let living = (0..self.players.len()).find(|o| !self.players[*o].beheaded);
                        living.and_then(|o| votes[o]).map(RunAction::Vote)
                    }
                    Stage::Map { .. } | Stage::Combat => None,
                    // a corpse takes no card and buys nothing
                    Stage::Reward { rewards } if rewards.get(q).is_some_and(|r| !r.done) => Some(RunAction::TakeCard(None)),
                    Stage::Reward { .. } => None,
                    Stage::Rest { done } if done.get(q) == Some(&false) => Some(RunAction::Sleep),
                    Stage::Rest { .. } => None,
                    Stage::Shop { done, .. } if done.get(q) == Some(&false) => Some(RunAction::LeaveShop),
                    Stage::Shop { .. } => None,
                    _ => run_bot(c, self, q as u8),
                };
                if let Some(action) = action {
                    if let Ok(more) = self.apply_one(c, q as u8, &action) {
                        ev.extend(more);
                        acted = true;
                    }
                }
            }
            if !acted {
                break;
            }
        }
        // nor has what was said for them
        for pl in self.players.iter_mut().filter(|pl| pl.beheaded) {
            pl.hp = 0;
        }
    }

    fn apply_one(&mut self, c: &Content, player: u8, action: &RunAction) -> Result<Vec<RunEvent>, RunError> {
        let p = player as usize;
        if p >= self.players.len() {
            return Err(RunError::NoSuchPlayer);
        }
        let mut ev = Vec::new();
        match (action, &mut self.stage) {
            (RunAction::Vote(choice), Stage::Map { votes }) => {
                let options = {
                    // borrow dance: choices() needs &self
                    let at = self.at;
                    let map = &self.map;
                    match at {
                        _ if map.floors.is_empty() => 1,
                        None => map.floors[0].len(),
                        Some((f, n)) if (f as usize + 1) < map.floors.len() => map.floors[f as usize][n as usize].next.len(),
                        Some(_) => 1,
                    }
                };
                if *choice as usize >= options {
                    return Err(RunError::BadChoice);
                }
                votes[p] = Some(*choice);
                ev.push(RunEvent::Voted { player, choice: *choice });
                if votes.iter().all(|v| v.is_some()) {
                    let cast: Vec<u8> = votes.iter().flatten().copied().collect();
                    // a party that disagrees draws lots, each vote one ticket: two against one
                    // goes two thirds of the time to the two, one against one is an even toss.
                    // The host's vote weighs no more than anyone's
                    let winner = if cast.iter().all(|v| *v == cast[0]) {
                        cast[0]
                    } else {
                        let chosen = *self.rng.pick(&cast).unwrap_or(&cast[0]);
                        ev.push(RunEvent::Lot { tied: cast.clone(), chosen });
                        chosen
                    };
                    self.travel(c, winner, &mut ev);
                }
            }
            (RunAction::Combat(a), Stage::Combat) => {
                let combat = self.combat.as_mut().ok_or(RunError::WrongStage)?;
                let events = combat::apply(c, combat, player, a).map_err(RunError::Combat)?;
                ev.extend(events.into_iter().map(RunEvent::Combat));
                let phase = combat.phase;
                match phase {
                    Phase::Won => self.after_fight(c, &mut ev),
                    Phase::Lost => {
                        self.stage = Stage::Defeat;
                        ev.push(RunEvent::Ended { won: false });
                    }
                    Phase::Heroes => {}
                }
            }
            (RunAction::TakeCard(pick), Stage::Reward { rewards }) => {
                let r = &mut rewards[p];
                if r.done {
                    return Err(RunError::AlreadyDone);
                }
                let card = match pick {
                    Some(i) => Some(r.cards.get(*i as usize).ok_or(RunError::BadChoice)?.clone()),
                    None => None,
                };
                r.done = true;
                if let Some(card) = &card {
                    self.players[p].deck.push((card.clone(), false));
                }
                ev.push(RunEvent::CardTaken { player, card });
                if rewards.iter().all(|r| r.done) {
                    self.to_map(c, &mut ev);
                }
            }
            (RunAction::Sleep, Stage::Rest { done }) | (RunAction::Upgrade(_), Stage::Rest { done }) => {
                if done[p] {
                    return Err(RunError::AlreadyDone);
                }
                let pl = &mut self.players[p];
                match action {
                    RunAction::Sleep => {
                        let pct = Circle(self.circle).rest_percent();
                        let healed = (pl.max_hp * pct / 100).min(pl.max_hp - pl.hp);
                        pl.hp += healed;
                        ev.push(RunEvent::Rested { player, healed });
                    }
                    RunAction::Upgrade(i) => {
                        let slot = pl.deck.get_mut(*i as usize).ok_or(RunError::BadChoice)?;
                        let upgradable = c.cards.get(&slot.0).is_some_and(|d| d.up.is_some());
                        if slot.1 || !upgradable {
                            return Err(RunError::BadChoice);
                        }
                        slot.1 = true;
                        ev.push(RunEvent::Upgraded { player, card: slot.0.clone() });
                    }
                    _ => unreachable!(),
                }
                done[p] = true;
                if done.iter().all(|d| *d) {
                    self.to_map(c, &mut ev);
                }
            }
            (RunAction::Raise { who }, Stage::Rest { done }) => {
                let w = *who as usize;
                if done[p] {
                    return Err(RunError::AlreadyDone);
                }
                if !self.players.get(w).is_some_and(|pl| pl.beheaded) {
                    return Err(RunError::BadChoice);
                }
                done[p] = true;
                let all = done.iter().all(|d| *d);
                self.pour_water(w, player, &mut ev);
                if all {
                    self.to_map(c, &mut ev);
                }
            }
            (RunAction::GiveGold { to, amount }, Stage::Rest { .. } | Stage::Shop { .. }) => {
                let to = *to as usize;
                if to >= self.players.len() || to == p || *amount <= 0 {
                    return Err(RunError::BadChoice);
                }
                if self.players[p].gold < *amount {
                    return Err(RunError::NotEnoughGold);
                }
                self.players[p].gold -= amount;
                self.players[to].gold += amount;
                ev.push(RunEvent::Gold { player, delta: -amount });
                ev.push(RunEvent::Gold { player: to as u8, delta: *amount });
            }
            (RunAction::BuyCard(i) | RunAction::BuyRelic(i) | RunAction::BuyPotion(i), Stage::Shop { stock, done }) => {
                if done[p] {
                    return Err(RunError::AlreadyDone);
                }
                let shelf = match action {
                    RunAction::BuyCard(_) => &mut stock[p].cards,
                    RunAction::BuyRelic(_) => &mut stock[p].relics,
                    _ => &mut stock[p].potions,
                };
                let slot = shelf.get_mut(*i as usize).ok_or(RunError::BadChoice)?;
                let (id, price) = slot.clone().ok_or(RunError::SoldOut)?;
                let pl = &mut self.players[p];
                if pl.gold < price {
                    return Err(RunError::NotEnoughGold);
                }
                if matches!(action, RunAction::BuyPotion(_)) && pl.potions.len() >= POTION_SLOTS {
                    return Err(RunError::NoRoom);
                }
                pl.gold -= price;
                *slot = None;
                ev.push(RunEvent::Gold { player, delta: -price });
                match action {
                    RunAction::BuyCard(_) => {
                        pl.deck.push((id.clone(), false));
                        ev.push(RunEvent::CardGained { player, card: id });
                    }
                    RunAction::BuyRelic(_) => {
                        pl.relics.push(id.clone());
                        ev.push(RunEvent::RelicGained { player, relic: id });
                    }
                    _ => {
                        pl.potions.push(id.clone());
                        ev.push(RunEvent::PotionGained { player, potion: id });
                    }
                }
            }
            (RunAction::RemoveCard(i), Stage::Shop { done, .. }) => {
                if done[p] {
                    return Err(RunError::AlreadyDone);
                }
                let pl = &mut self.players[p];
                // 75 gold, then 25 more each time
                let price = 75 + 25 * pl.removals as i32;
                if *i as usize >= pl.deck.len() {
                    return Err(RunError::BadChoice);
                }
                if pl.gold < price {
                    return Err(RunError::NotEnoughGold);
                }
                pl.gold -= price;
                pl.removals += 1;
                let (card, _) = pl.deck.remove(*i as usize);
                ev.push(RunEvent::Gold { player, delta: -price });
                ev.push(RunEvent::CardRemoved { player, card });
            }
            (RunAction::LeaveShop, Stage::Shop { done, .. }) => {
                done[p] = true;
                if done.iter().all(|d| *d) {
                    self.to_map(c, &mut ev);
                }
            }
            (RunAction::Choose(choice), Stage::Chest(ch)) => {
                if *choice as usize >= ch.offers.len() {
                    return Err(RunError::BadChoice);
                }
                if ch.picks.iter().all(|v| v.is_some()) {
                    return Err(RunError::AlreadyDone);
                }
                // a claim may be changed until the last one is made
                ch.picks[p] = Some(*choice);
                ev.push(RunEvent::Voted { player, choice: *choice });
                if ch.picks.iter().all(|v| v.is_some()) {
                    self.share_chest(&mut ev);
                }
            }
            (RunAction::Throw(shape), Stage::Chest(ch)) => {
                if *shape > 2 || !ch.contest.contains(&player) {
                    return Err(RunError::BadChoice);
                }
                if ch.throws[p].is_some() {
                    return Err(RunError::AlreadyDone);
                }
                ch.throws[p] = Some(*shape);
                if ch.contest.iter().all(|q| ch.throws[*q as usize].is_some()) {
                    let round: Vec<(u8, u8)> = ch.contest.iter().map(|q| (*q, ch.throws[*q as usize].unwrap_or(0))).collect();
                    ch.rounds += 1;
                    ch.last = round.clone();
                    ch.throws = vec![None; ch.throws.len()];
                    let mut shapes: Vec<u8> = round.iter().map(|(_, s)| *s).collect();
                    shapes.sort_unstable();
                    shapes.dedup();
                    // one shape or all three: nobody wins, throw again
                    if let [a, b] = shapes[..] {
                        let best = if beats(a, b) { a } else { b };
                        ch.contest = round.iter().filter(|(_, s)| *s == best).map(|(q, _)| *q).collect();
                        for (q, s) in &round {
                            if *s != best {
                                ch.picks[*q as usize] = Some(Chest::LOST);
                            }
                        }
                        if let [w] = ch.contest[..] {
                            ch.won[w as usize] = ch.prize;
                            ch.contest.clear();
                        }
                    }
                    ev.push(RunEvent::Thrown { throws: round });
                    self.share_chest(&mut ev);
                }
            }
            (RunAction::Onward, Stage::Chest(ch)) => {
                if !ch.settled {
                    return Err(RunError::BadChoice);
                }
                ch.done[p] = true;
                if ch.done.iter().all(|d| *d) {
                    self.to_map(c, &mut ev);
                }
            }
            (RunAction::Choose(choice), Stage::Trophy { offers, picks }) => {
                if *choice as usize >= offers[p].len() {
                    return Err(RunError::BadChoice);
                }
                // each chooses from their own; one may change their mind until the last has chosen
                picks[p] = Some(*choice);
                if picks.iter().all(|v| v.is_some()) {
                    let won: Vec<Id> = offers.iter().zip(picks.iter()).map(|(own, pick)| own[pick.unwrap_or(0) as usize].clone()).collect();
                    for (q, relic) in won.into_iter().enumerate() {
                        self.players[q].relics.push(relic.clone());
                        ev.push(RunEvent::RelicGained { player: q as u8, relic });
                    }
                    self.stage = Stage::Map { votes: vec![None; self.players.len()] };
                }
            }
            (RunAction::Choose(choice), Stage::Omen { id, picks }) => {
                let def = c.events.get(id.as_str()).ok_or(RunError::BadChoice)?.clone();
                let option = def.options.get(*choice as usize).ok_or(RunError::BadChoice)?;
                if picks[p].is_some() && def.kind != EventKind::Vote {
                    return Err(RunError::AlreadyDone);
                }
                if def.kind == EventKind::Schism && schism_closed(picks, p) == Some(*choice) {
                    return Err(RunError::MustSplit);
                }
                if self.players[p].gold < option.price {
                    return Err(RunError::NotEnoughGold);
                }
                if !seam_open(&self.players, &def, option, p, picks) {
                    return Err(RunError::BadChoice);
                }
                picks[p] = Some(*choice);
                let all = picks.iter().all(|v| v.is_some());
                let cast: Vec<u8> = picks.iter().flatten().copied().collect();
                match def.kind {
                    // one pays, all gain; those who refuse just wait to see if anyone will
                    EventKind::Sacrifice => {
                        self.apply_option(c, p, option, &mut ev);
                        if !option.all.is_empty() {
                            ev.push(RunEvent::OmenResolved { option: *choice });
                            let gift = EventOption { id: option.id.clone(), price: 0, fx: option.all.clone(), all: vec![] };
                            for q in 0..self.players.len() {
                                self.apply_option(c, q, &gift, &mut ev);
                            }
                            self.to_map(c, &mut ev);
                        } else if all {
                            self.to_map(c, &mut ev);
                        }
                    }
                    // everyone decides for themselves, at once
                    EventKind::Personal | EventKind::Schism => {
                        self.apply_option(c, p, option, &mut ev);
                        if all {
                            self.to_map(c, &mut ev);
                        }
                    }
                    // one choice for the party; a tie goes to the lot
                    EventKind::Vote => {
                        ev.push(RunEvent::Voted { player, choice: *choice });
                        if all {
                            let mut counts = vec![0u32; def.options.len()];
                            for v in &cast {
                                counts[*v as usize] += 1;
                            }
                            let best = counts.iter().copied().max().unwrap_or(0);
                            let tied: Vec<u8> = (0..def.options.len() as u8).filter(|i| counts[*i as usize] == best).collect();
                            let winner = if tied.len() == 1 {
                                tied[0]
                            } else {
                                let chosen = *self.rng.pick(&tied).unwrap_or(&tied[0]);
                                ev.push(RunEvent::Lot { tied: tied.clone(), chosen });
                                chosen
                            };
                            ev.push(RunEvent::OmenResolved { option: winner });
                            let option = &def.options[winner as usize];
                            for q in 0..self.players.len() {
                                self.apply_option(c, q, option, &mut ev);
                            }
                            self.to_map(c, &mut ev);
                        }
                    }
                }
            }
            (RunAction::DropPotion(i), _) => {
                // not mid-fight: there the potions live in the combat state
                if self.stage == Stage::Combat {
                    return Err(RunError::WrongStage);
                }
                let pl = &mut self.players[p];
                if *i as usize >= pl.potions.len() {
                    return Err(RunError::BadChoice);
                }
                pl.potions.remove(*i as usize);
            }
            (RunAction::DrinkPotion(i), _) => {
                if self.stage == Stage::Combat {
                    return Err(RunError::WrongStage);
                }
                let id = self.players[p].potions.get(*i as usize).ok_or(RunError::BadChoice)?.clone();
                let heals = c.potions.get(&id).map(|d| out_of_fight_heals(&d.effects)).unwrap_or_default();
                if heals.is_empty() {
                    return Err(RunError::BadChoice);
                }
                self.players[p].potions.remove(*i as usize);
                for (n, all) in heals {
                    for q in 0..self.players.len() {
                        if q != p && !all {
                            continue;
                        }
                        let pl = &mut self.players[q];
                        let healed = n.min(pl.max_hp - pl.hp).max(0);
                        if pl.hp > 0 && healed > 0 {
                            pl.hp += healed;
                            ev.push(RunEvent::Hp { player: q as u8, delta: healed });
                        }
                    }
                }
            }
            (RunAction::PeltPedlar(i), Stage::Shop { stock, .. }) => {
                if stock[p].pelted {
                    return Err(RunError::AlreadyDone);
                }
                if *i as usize >= self.players[p].potions.len() {
                    return Err(RunError::BadChoice);
                }
                stock[p].pelted = true;
                let pl = &mut self.players[p];
                pl.potions.remove(*i as usize);
                pl.gold += PELT_GOLD;
                ev.push(RunEvent::Gold { player, delta: PELT_GOLD });
            }
            (RunAction::FindSecret { act }, _) => {
                if self.stage == Stage::Combat {
                    return Err(RunError::WrongStage);
                }
                let Some(relic) = act.checked_sub(1).and_then(|a| SECRET_RELICS.get(a as usize)) else { return Err(RunError::BadChoice) };
                if self.secrets.contains(act) {
                    return Err(RunError::AlreadyDone);
                }
                self.secrets.push(*act);
                let pl = &mut self.players[p];
                if !pl.relics.iter().any(|r| r == relic) {
                    pl.relics.push((*relic).into());
                    ev.push(RunEvent::RelicGained { player, relic: (*relic).into() });
                }
            }
            (RunAction::SkipTale, _) => {
                if self.stage == Stage::Combat {
                    return Err(RunError::WrongStage);
                }
                let pl = &mut self.players[p];
                if !pl.relics.iter().any(|r| r == SECRET_RELICS[3]) {
                    return Err(RunError::BadChoice);
                }
                // who did not hear the tale out is not held by it
                pl.gold += UNHEARD_GOLD;
                ev.push(RunEvent::Gold { player, delta: UNHEARD_GOLD });
                let curse = pl.deck.iter().position(|(id, _)| c.cards.get(id).is_some_and(|d| d.kind == CardType::Curse));
                if let Some(i) = curse {
                    let (card, _) = pl.deck.remove(i);
                    ev.push(RunEvent::CardRemoved { player, card });
                }
            }
            (RunAction::TearPage { lost }, _) => {
                // the book is read between fights, never in one; a nick that frees no paper costs nothing
                if self.stage == Stage::Combat {
                    return Err(RunError::WrongStage);
                }
                if *lost == 0 || *lost > 100 {
                    return Err(RunError::BadChoice);
                }
                let pl = &mut self.players[p];
                pl.deck.push((BOOK_SPOIL.into(), false));
                ev.push(RunEvent::CardGained { player, card: BOOK_SPOIL.into() });
                if *lost > BIG_TEAR {
                    pl.max_hp = (pl.max_hp - BIG_TEAR_HP).max(1);
                    let delta = pl.hp.min(pl.max_hp) - pl.hp;
                    pl.hp += delta;
                    if delta != 0 {
                        ev.push(RunEvent::Hp { player, delta });
                    }
                }
            }
            _ => return Err(RunError::WrongStage),
        }
        Ok(ev)
    }

    /// Puts the party straight into a shop or an omen, wherever it stands.
    /// For tests and the dev build's screen shortcuts; never reachable by a player action.
    pub fn force_stage(&mut self, c: &Content, kind: NodeKind) {
        match kind {
            NodeKind::Shop => {
                let stock = (0..self.players.len()).map(|p| self.roll_stock(c, p)).collect();
                self.stage = Stage::Shop { stock, done: vec![false; self.players.len()] };
            }
            NodeKind::Event => {
                let pool: Vec<&EventDef> = c.events.values().filter(|e| !e.for_the_maimed).collect();
                if let Some(e) = self.rng.pick(&pool) {
                    self.stage = Stage::Omen { id: e.id.clone(), picks: vec![None; self.players.len()] };
                }
            }
            NodeKind::Rest => self.stage = Stage::Rest { done: vec![false; self.players.len()] },
            NodeKind::Chest => self.open_chest(c),
            _ => {}
        }
    }

    fn open_chest(&mut self, c: &Content) {
        let n = self.players.len();
        let mut offers: Vec<Id> = Vec::new();
        for p in 0..n {
            // a relic for every seat, no two alike
            for _ in 0..8 {
                if let Some(r) = self.roll_relic(c, p).filter(|r| !offers.contains(r)) {
                    offers.push(r);
                    break;
                }
            }
        }
        if offers.is_empty() {
            self.stage = Stage::Map { votes: vec![None; n] };
            return;
        }
        self.stage = Stage::Chest(Chest {
            offers,
            picks: vec![None; n],
            won: vec![None; n],
            contest: vec![],
            prize: None,
            throws: vec![None; n],
            last: vec![],
            rounds: 0,
            settled: false,
            done: vec![false; n],
        });
    }

    /// Shares out what can be shared: lone claims are granted, the next contested offer is thrown
    /// for, and when nothing is left to throw for, the losers take the unclaimed relics by lot.
    fn share_chest(&mut self, ev: &mut Vec<RunEvent>) {
        let Stage::Chest(ch) = &mut self.stage else { return };
        if !ch.contest.is_empty() {
            return;
        }
        let m = ch.offers.len() as u8;
        let claimants = |ch: &Chest, i: u8| (0..ch.picks.len() as u8).filter(|q| ch.picks[*q as usize] == Some(i)).collect::<Vec<u8>>();
        for i in 0..m {
            if let [q] = claimants(ch, i)[..] {
                ch.won[q as usize] = Some(i);
            }
        }
        if let Some(i) = (0..m).find(|i| claimants(ch, *i).len() > 1 && !ch.won.contains(&Some(*i))) {
            ch.prize = Some(i);
            ch.contest = claimants(ch, i);
            return;
        }
        ch.prize = None;
        let mut left: Vec<u8> = (0..m).filter(|i| !ch.won.contains(&Some(*i))).collect();
        self.rng.shuffle(&mut left);
        let Stage::Chest(ch) = &mut self.stage else { return };
        // with fewer relics than seats, the last losers go without
        for w in ch.won.iter_mut().filter(|w| w.is_none()) {
            *w = left.pop();
        }
        ch.settled = true;
        let gains: Vec<(usize, Id)> = ch.won.iter().enumerate().filter_map(|(q, w)| w.map(|i| (q, ch.offers[i as usize].clone()))).collect();
        for (q, relic) in gains {
            self.players[q].relics.push(relic.clone());
            ev.push(RunEvent::RelicGained { player: q as u8, relic });
        }
    }

    fn to_map(&mut self, c: &Content, ev: &mut Vec<RunEvent>) {
        // past the boss there is no map left in this act
        let boss_done = self.fighting == Some(NodeKind::Boss);
        self.fighting = None;
        self.combat = None;
        if boss_done {
            ev.push(RunEvent::ActCleared { act: self.act });
            // act 4 is the island of Buyan: Koschei himself
            if self.act >= 4 {
                self.stage = Stage::Victory;
                ev.push(RunEvent::Ended { won: true });
                return;
            }
            self.act += 1;
            self.at = None;
            self.trail.clear();
            // a new land, rested: everyone still on their feet gets back most of what they lost (as
            // Slay the Spire 2's ancients heal, four fifths of it). Now that Act 4 Buyan Island has
            // its own floors and encounters before Koschei, the party heals after Mara as well.
            if self.act <= 4 {
                for q in 0..self.players.len() {
                    let pl = &mut self.players[q];
                    if !pl.beheaded && pl.hp < pl.max_hp {
                        let delta = ((pl.max_hp - pl.hp) * ACT_HEAL_PCT + 99) / 100;
                        pl.hp += delta;
                        ev.push(RunEvent::Hp { player: q as u8, delta });
                    }
                }
            }
            // the traveller's satchel: there is always something still in it
            for q in 0..self.players.len() {
                if self.players[q].relics.iter().any(|r| r == SECRET_RELICS[0]) {
                    self.give_potion(c, q, ev);
                }
            }
            self.map = if self.act >= 4 { generate_buyan_map(&mut self.rng, Circle(self.circle)) } else { generate_map(&mut self.rng, Circle(self.circle)) };
            // the boss leaves a trophy: three of their own for every player, to take one
            let offers = self.roll_trophies(c);
            if offers.iter().all(|own| !own.is_empty()) {
                self.stage = Stage::Trophy { offers, picks: vec![None; self.players.len()] };
                return;
            }
        }
        self.stage = Stage::Map { votes: vec![None; self.players.len()] };
    }

    fn travel(&mut self, c: &Content, choice: u8, ev: &mut Vec<RunEvent>) {
        let options = self.choices();
        let (node, kind) = options[choice as usize];
        let floor = match self.at {
            None => 0,
            Some((f, _)) => f + 1,
        };
        if kind != NodeKind::Boss && !self.map.floors.is_empty() {
            self.at = Some((floor, node));
            self.trail.push((floor, node));
        }
        ev.push(RunEvent::Moved { floor, node, kind });
        match kind {
            NodeKind::Fight | NodeKind::Elite | NodeKind::Boss => self.start_fight(c, kind, ev),
            NodeKind::Rest => self.stage = Stage::Rest { done: vec![false; self.players.len()] },
            NodeKind::Chest if self.players.len() > 1 => {
                self.floors_cleared += 1;
                self.open_chest(c);
            }
            NodeKind::Chest => {
                self.floors_cleared += 1;
                let rewards = (0..self.players.len()).map(|p| Reward { gold: 0, cards: vec![], relic: self.roll_relic(c, p), done: false }).collect();
                self.grant(c, rewards, ev);
            }
            NodeKind::Shop => {
                self.floors_cleared += 1;
                let stock = (0..self.players.len()).map(|p| self.roll_stock(c, p)).collect();
                self.stage = Stage::Shop { stock, done: vec![false; self.players.len()] };
            }
            NodeKind::Event => {
                self.floors_cleared += 1;
                let act = self.act;
                let open = 20 + 15 * self.unlocks.omen_tier as usize;
                let fits: Vec<&EventDef> = c.events.values().filter(|e| !e.for_the_maimed).take(open).filter(|e| e.act == 0 || e.act == act).collect();
                let fresh: Vec<&EventDef> = fits.iter().copied().filter(|e| !self.omens_seen.contains(&e.id)).collect();
                // a long road may run out of new omens; only then does an old one return
                let pool = if fresh.is_empty() { fits } else { fresh };
                // while someone is missing a part, she who sews them may be waiting on the road
                let someone_to_ask = self.players.iter().any(|pl| !pl.beheaded);
                let missing = self.players.iter().any(|pl| pl.beheaded || pl.walks_headless || pl.lost_limbs > 0);
                let waiting = if someone_to_ask && missing && self.rng.chance(SEAMSTRESS_CHANCE) { c.events.values().find(|e| e.for_the_maimed) } else { None };
                match waiting.or_else(|| self.rng.pick(&pool).copied()) {
                    Some(e) => {
                        self.omens_seen.push(e.id.clone());
                        self.stage = Stage::Omen { id: e.id.clone(), picks: vec![None; self.players.len()] };
                    }
                    // no omens written yet: the road is simply quiet
                    None => self.stage = Stage::Map { votes: vec![None; self.players.len()] },
                }
            }
        }
    }

    fn start_fight(&mut self, c: &Content, kind: NodeKind, ev: &mut Vec<RunEvent>) {
        let want = match kind {
            NodeKind::Elite => EncounterKind::Elite,
            NodeKind::Boss => EncounterKind::Boss,
            _ => EncounterKind::Normal,
        };
        let pick = |rng: &mut Rng, act: u8, kind: EncounterKind| -> Option<EncounterDef> {
            let pool: Vec<&EncounterDef> = c.encounters.values().filter(|e| e.act == act && e.kind == kind).collect();
            rng.pick(&pool).map(|e| (*e).clone())
        };
        // fall back so a thin content set never strands the party
        let enc = pick(&mut self.rng, self.act, want)
            .or_else(|| pick(&mut self.rng, self.act, EncounterKind::Normal))
            .or_else(|| pick(&mut self.rng, 1, EncounterKind::Normal));
        let Some(enc) = enc else {
            self.stage = Stage::Map { votes: vec![None; self.players.len()] };
            return;
        };
        let n = self.players.len();
        let mut enemies = if n == 1 && !enc.solo.is_empty() { enc.solo.clone() } else { enc.enemies.clone() };
        if n == 3 && self.rng.chance(50) {
            enemies.extend(enc.extra3.iter().cloned());
        }
        if n >= 4 {
            enemies.extend(enc.extra4.iter().cloned());
        }
        let party: Vec<HeroSetup> = self
            .players
            .iter()
            .map(|p| HeroSetup { def: p.hero.clone(), hp: p.hp, max_hp: p.max_hp, deck: p.deck.clone(), relics: p.relics.clone(), falls: p.falls, lost_limbs: p.lost_limbs, lost_arms: p.lost_arms, beheaded: p.beheaded, walks_headless: p.walks_headless, potions: p.potions.clone() })
            .collect();
        let rules = Circle(self.circle);
        let opts = CombatOpts { raise_cost: rules.raise_cost(), enemy_hp: rules.enemy_hp(), enemy_dmg: rules.enemy_dmg(kind), shadow: rules.koschei_shadow(), stall: match want {
            EncounterKind::Boss => combat::STALL_BOSS,
            EncounterKind::Elite => combat::STALL_ELITE,
            _ => combat::STALL_ROUND,
        } };
        let seed = ((self.rng.next_u32() as u64) << 32) | self.rng.next_u32() as u64;
        let (mut combat, events) = combat::start_with(c, &party, &enemies, seed, opts);
        for (h, p) in combat.heroes.iter_mut().zip(&self.players) {
            h.gold = p.gold;
        }
        self.combat = Some(combat);
        self.fighting = Some(kind);
        self.stage = Stage::Combat;
        ev.push(RunEvent::CombatStarted);
        ev.extend(events.into_iter().map(RunEvent::Combat));
        // a fight can be over before anyone acts: a curse or a relic that bites at the start
        // may fell the last hero standing, and then nobody is left to make the move that ends it
        match self.combat.as_ref().map(|k| k.phase) {
            Some(Phase::Lost) => {
                self.stage = Stage::Defeat;
                ev.push(RunEvent::Ended { won: false });
            }
            Some(Phase::Won) => self.after_fight(c, ev),
            _ => {}
        }
    }

    /// Puts the party straight into a fight with exactly these enemies.
    /// For tests and the dev build's screen shortcuts; never reachable by a player action.
    pub fn force_fight(&mut self, c: &Content, enemies: &[Id]) {
        let party: Vec<HeroSetup> = self
            .players
            .iter()
            .map(|p| HeroSetup { def: p.hero.clone(), hp: p.hp, max_hp: p.max_hp, deck: p.deck.clone(), relics: p.relics.clone(), falls: p.falls, lost_limbs: p.lost_limbs, lost_arms: p.lost_arms, beheaded: p.beheaded, walks_headless: p.walks_headless, potions: p.potions.clone() })
            .collect();
        let seed = ((self.rng.next_u32() as u64) << 32) | self.rng.next_u32() as u64;
        let (mut combat, _) = combat::start_with(c, &party, enemies, seed, CombatOpts::default());
        for (h, p) in combat.heroes.iter_mut().zip(&self.players) {
            h.gold = p.gold;
        }
        self.combat = Some(combat);
        self.fighting = Some(NodeKind::Fight);
        self.stage = Stage::Combat;
    }

    fn after_fight(&mut self, c: &Content, ev: &mut Vec<RunEvent>) {
        // the finished fight stays readable until the party walks on
        let Some(combat) = self.combat.clone() else { return };
        let kind = self.fighting.unwrap_or(NodeKind::Fight);
        for (pl, h) in self.players.iter_mut().zip(&combat.heroes) {
            pl.hp = if h.beheaded { 0 } else { h.hp.max(1) };
            pl.max_hp = h.max_hp;
            pl.falls = h.falls;
            pl.lost_limbs = h.lost_limbs;
            pl.lost_arms = h.lost_arms;
            pl.beheaded = h.beheaded;
            pl.potions = h.potions.clone();
            pl.gold = (pl.gold - h.gold_lost).max(0);
            // what was cut off is carried from now on; what was thrown away this fight is gone
            for part in &h.new_parts {
                pl.deck.push((part.clone(), false));
            }
            for thrown in h.exhaust.iter().filter(|card| card.def.starts_with("own_")) {
                take_from_deck(pl, &thrown.def);
            }
            for _ in 0..h.new_curses {
                pl.deck.push((NAV_MARK.into(), false));
            }
        }
        self.floors_cleared += 1;
        match kind {
            NodeKind::Elite => self.elites_slain += 1,
            NodeKind::Boss => self.bosses_slain += 1,
            _ => {}
        }
        let gold_pct = Circle(self.circle).gold_percent();
        let mut rewards = Vec::new();
        for p in 0..self.players.len() {
            let base = match kind {
                NodeKind::Elite => self.rng.range(25, 35),
                NodeKind::Boss => self.rng.range(95, 105),
                _ => self.rng.range(10, 20),
            };
            let cards = self.roll_cards(c, p, kind);
            let relic = if kind == NodeKind::Elite { self.roll_relic(c, p) } else { None };
            rewards.push(Reward { gold: base * gold_pct / 100, cards, relic, done: false });
        }
        // four fights in ten leave a draught behind
        for p in 0..self.players.len() {
            if self.rng.chance(40) {
                self.give_potion(c, p, ev);
            }
        }
        self.grant(c, rewards, ev);
    }

    /// Hands out gold and relics at once and leaves the card choice to the players.
    fn grant(&mut self, c: &Content, mut rewards: Vec<Reward>, ev: &mut Vec<RunEvent>) {
        for (p, r) in rewards.iter_mut().enumerate() {
            if r.gold != 0 {
                self.players[p].gold += r.gold;
                ev.push(RunEvent::Gold { player: p as u8, delta: r.gold });
            }
            if let Some(relic) = &r.relic {
                self.players[p].relics.push(relic.clone());
                ev.push(RunEvent::RelicGained { player: p as u8, relic: relic.clone() });
            }
            // nothing to choose: this player is finished already
            if r.cards.is_empty() {
                r.done = true;
            }
        }
        ev.push(RunEvent::Rewards);
        if rewards.iter().all(|r| r.done) {
            self.stage = Stage::Reward { rewards };
            self.to_map(c, ev);
        } else {
            self.stage = Stage::Reward { rewards };
        }
    }

    /// Three different cards of the player's own hero; co-op cards only in a party.
    fn roll_cards(&mut self, c: &Content, p: usize, kind: NodeKind) -> Vec<Id> {
        let hero = &self.players[p].hero;
        let coop = self.players.len() > 1;
        let pool = |rarity: Rarity| -> Vec<&CardDef> {
            c.cards
                .values()
                .filter(|d| &d.hero == hero && d.rarity == rarity && (coop || !d.keywords.contains(&Keyword::Coop)))
                .filter(|d| !matches!(d.kind, CardType::Curse | CardType::Status))
                .filter(|d| self.unlocks.card_ok(p, d))
                .collect()
        };
        let (common, uncommon, rare) = (pool(Rarity::Common), pool(Rarity::Uncommon), pool(Rarity::Rare));
        // elites and bosses lean toward the better shelves
        let (w_unc, w_rare) = match kind {
            NodeKind::Boss => (40, 60),
            NodeKind::Elite => (45, 12),
            _ => (37, 3),
        };
        let mut out: Vec<Id> = Vec::new();
        for _ in 0..40 {
            if out.len() == 3 {
                break;
            }
            let roll = self.rng.below(100) as i32;
            let shelf = if roll < w_rare { &rare } else if roll < w_rare + w_unc { &uncommon } else { &common };
            // an empty shelf falls through to whatever exists
            let shelf = [shelf, &uncommon, &common, &rare].into_iter().find(|s| !s.is_empty());
            let Some(shelf) = shelf else { break };
            if let Some(card) = self.rng.pick(shelf) {
                if !out.contains(&card.id) {
                    out.push(card.id.clone());
                }
            }
        }
        out
    }

    /// An omen's option applied to one player, for tests.
    #[doc(hidden)]
    pub fn test_apply_option(&mut self, c: &Content, p: usize, option: &EventOption, ev: &mut Vec<RunEvent>) {
        self.apply_option(c, p, option, ev);
    }

    /// Dead water and living water get a body up, but water sews nothing: it walks with no head,
    /// on a tenth of its health, until the seamstress sees to it.
    fn pour_water(&mut self, who: usize, by: u8, ev: &mut Vec<RunEvent>) {
        let pl = &mut self.players[who];
        pl.beheaded = false;
        pl.walks_headless = true;
        if pl.whole_max_hp == 0 {
            pl.whole_max_hp = pl.max_hp;
        }
        pl.max_hp = (pl.whole_max_hp * WATER_PCT / 100).max(1);
        pl.hp = pl.max_hp;
        ev.push(RunEvent::Raised { player: by, who: who as u8 });
    }

    /// A head sewn back on, whether its owner lay dead or walked without it: far weaker for good,
    /// with a quarter of what is left. What they lost of their limbs stays lost.
    fn sew_head(&mut self, who: usize, by: u8, ev: &mut Vec<RunEvent>) {
        let pl = &mut self.players[who];
        let whole = if pl.whole_max_hp > 0 { pl.whole_max_hp } else { pl.max_hp };
        pl.beheaded = false;
        pl.walks_headless = false;
        pl.whole_max_hp = 0;
        pl.max_hp = (whole - whole * HEAD_SEAM_PCT / 100).max(LEAST_MAX_HP);
        pl.hp = (pl.max_hp * RAISED_PCT / 100).max(1);
        ev.push(RunEvent::Raised { player: by, who: who as u8 });
    }

    fn apply_option(&mut self, c: &Content, p: usize, option: &EventOption, ev: &mut Vec<RunEvent>) {
        let player = p as u8;
        // an omen does nothing to a corpse
        if self.players[p].beheaded {
            return;
        }
        if option.price > 0 {
            self.players[p].gold = (self.players[p].gold - option.price).max(0);
            ev.push(RunEvent::Gold { player, delta: -option.price });
        }
        for fx in &option.fx {
            match fx {
                RunFx::Gold(n) => {
                    let pl = &mut self.players[p];
                    let delta = (*n).max(-pl.gold);
                    pl.gold += delta;
                    ev.push(RunEvent::Gold { player, delta });
                }
                RunFx::Hp(n) | RunFx::HpPct(n) => {
                    let pl = &mut self.players[p];
                    let n = if matches!(fx, RunFx::HpPct(_)) { pl.max_hp * n / 100 } else { *n };
                    // an omen may wound, but it never kills outright
                    let delta = n.clamp(1 - pl.hp, pl.max_hp - pl.hp);
                    pl.hp += delta;
                    ev.push(RunEvent::Hp { player, delta });
                }
                RunFx::MaxHp(n) => {
                    let pl = &mut self.players[p];
                    pl.max_hp = (pl.max_hp + n).max(1);
                    pl.hp = (pl.hp + (*n).max(0)).min(pl.max_hp).max(1);
                }
                RunFx::SewHeadOn => {
                    if let Some(who) = self.players.iter().position(head_to_sew) {
                        let card = format!("own_head_{}", self.players[who].hero);
                        take_from_deck(&mut self.players[who], &card);
                        self.sew_head(who, player, ev);
                    }
                }
                RunFx::SewLimbOn => {
                    let pl = &mut self.players[p];
                    if let Some((back, card)) = arm_to_sew(pl) {
                        take_from_deck(pl, &card);
                        // with the arm back the makeshifts go; those for no arms at all go too
                        for gone in crate::combat::stump_cards(back).iter().chain(crate::combat::BOTH_ARMS_CARDS.iter()) {
                            take_from_deck(pl, gone);
                        }
                        pl.lost_limbs -= 1;
                        pl.lost_arms &= !back;
                        pl.max_hp = (pl.max_hp - pl.max_hp * LIMB_SEAM_PCT / 100).max(LEAST_MAX_HP);
                        pl.hp = pl.hp.min(pl.max_hp);
                        ev.push(RunEvent::LimbSewn { player });
                    }
                }
                RunFx::Card(card) => {
                    self.players[p].deck.push((card.clone(), false));
                    ev.push(RunEvent::CardGained { player, card: card.clone() });
                }
                RunFx::RandomCard(rarity) => {
                    let hero = self.players[p].hero.clone();
                    let coop = self.players.len() > 1;
                    let fits = |d: &&CardDef| d.hero == hero && d.rarity == *rarity && (coop || !d.keywords.contains(&Keyword::Coop));
                    let mut pool: Vec<&CardDef> = c.cards.values().filter(fits).filter(|d| self.unlocks.card_ok(p, d)).collect();
                    // the omen promised a card of this rarity: it keeps its word even before the
                    // hero's level opens that rarity to the rewards
                    if pool.is_empty() {
                        pool = c.cards.values().filter(fits).collect();
                    }
                    if let Some(card) = self.rng.pick(&pool).map(|d| d.id.clone()) {
                        self.players[p].deck.push((card.clone(), false));
                        ev.push(RunEvent::CardGained { player, card });
                    }
                }
                RunFx::Relic => {
                    if let Some(relic) = self.roll_relic(c, p) {
                        self.players[p].relics.push(relic.clone());
                        ev.push(RunEvent::RelicGained { player, relic });
                    }
                }
                RunFx::Potion => self.give_potion(c, p, ev),
                RunFx::RemoveRandomCard => {
                    let n = self.players[p].deck.len();
                    if n > 1 {
                        let i = self.rng.below(n as u32) as usize;
                        let (card, _) = self.players[p].deck.remove(i);
                        ev.push(RunEvent::CardRemoved { player, card });
                    }
                }
                RunFx::UpgradeRandomCard => {
                    let able: Vec<usize> = self.players[p].deck.iter().enumerate().filter(|(_, (id, up))| !up && c.cards.get(id).is_some_and(|d| d.up.is_some())).map(|(i, _)| i).collect();
                    if let Some(i) = self.rng.pick(&able).copied() {
                        self.players[p].deck[i].1 = true;
                        ev.push(RunEvent::Upgraded { player, card: self.players[p].deck[i].0.clone() });
                    }
                }
            }
        }
    }

    /// A random potion, if the player has a free slot.
    fn give_potion(&mut self, c: &Content, p: usize, ev: &mut Vec<RunEvent>) {
        if self.players[p].potions.len() >= POTION_SLOTS {
            return;
        }
        let open = 10 * (self.unlocks.potion_tier as usize + 1);
        let pool: Vec<&Id> = c.potions.keys().take(open).collect();
        if let Some(potion) = self.rng.pick(&pool).map(|id| (*id).clone()) {
            self.players[p].potions.push(potion.clone());
            ev.push(RunEvent::PotionGained { player: p as u8, potion });
        }
    }

    /// The pedlar's shelves for one player: 5 cards of their hero, 2 neutral, 3 relics, 3 potions.
    fn roll_stock(&mut self, c: &Content, p: usize) -> Stock {
        let hero = self.players[p].hero.clone();
        let coop = self.players.len() > 1;
        let price = |rng: &mut Rng, rarity: Rarity| match rarity {
            Rarity::Rare => rng.range(135, 165),
            Rarity::Uncommon => rng.range(68, 82),
            _ => rng.range(45, 55),
        };
        let mut stock = Stock::default();
        let mine: Vec<&CardDef> = c
            .cards
            .values()
            .filter(|d| d.hero == hero && !matches!(d.rarity, Rarity::Starter | Rarity::Special) && (coop || !d.keywords.contains(&Keyword::Coop)))
            .filter(|d| self.unlocks.card_ok(p, d))
            .collect();
        let neutral: Vec<&CardDef> = c.cards.values().filter(|d| d.hero.is_empty() && !matches!(d.kind, CardType::Curse | CardType::Status) && d.rarity != Rarity::Special).collect();
        for (pool, want) in [(&mine, 5), (&neutral, 2)] {
            let mut picked: Vec<&Id> = Vec::new();
            for _ in 0..40 {
                if picked.len() == want.min(pool.len()) {
                    break;
                }
                if let Some(d) = self.rng.pick(pool) {
                    if !picked.contains(&&d.id) {
                        picked.push(&d.id);
                        let cost = price(&mut self.rng, d.rarity);
                        stock.cards.push(Some((d.id.clone(), cost)));
                    }
                }
            }
        }
        let owned = self.players[p].relics.clone();
        let relics: Vec<&RelicDef> =
            c.relics.values().filter(|r| r.rarity != Rarity::Starter && r.pool != RelicPool::Boss && (r.hero.is_empty() || r.hero == hero) && !owned.contains(&r.id) && self.unlocks.relic_ok(r.rarity)).collect();
        // the first shelf is the pedlar's own: something found nowhere else
        let wares: Vec<&RelicDef> = relics.iter().copied().filter(|r| r.pool == RelicPool::Shop).collect();
        if let Some(r) = self.rng.pick(&wares) {
            let cost = price(&mut self.rng, r.rarity) + 80;
            stock.relics.push(Some((r.id.clone(), cost)));
        }
        for _ in 0..40 {
            if stock.relics.len() == 3.min(relics.len()) {
                break;
            }
            if let Some(r) = self.rng.pick(&relics) {
                if !stock.relics.iter().flatten().any(|(id, _)| *id == r.id) {
                    let cost = price(&mut self.rng, r.rarity) + 80;
                    stock.relics.push(Some((r.id.clone(), cost)));
                }
            }
        }
        let potions: Vec<&Id> = c.potions.keys().take(10 * (self.unlocks.potion_tier as usize + 1)).collect();
        for _ in 0..3.min(potions.len()) {
            if let Some(id) = self.rng.pick(&potions) {
                let cost = self.rng.range(45, 60);
                stock.potions.push(Some(((*id).clone(), cost)));
            }
        }
        stock
    }

    /// A relic this player doesn't own yet: common pool, or their hero's own.
    /// Three boss relics for each player: none they hold, and none another is offered or holds
    /// while the pool lasts.
    pub fn roll_trophies(&mut self, c: &Content) -> Vec<Vec<Id>> {
        let mut pool: Vec<Id> = c.relics.values().filter(|r| r.pool == RelicPool::Boss).map(|r| r.id.clone()).collect();
        self.rng.shuffle(&mut pool);
        let mut offers: Vec<Vec<Id>> = vec![Vec::new(); self.players.len()];
        let held = |run: &Run, id: &Id| run.players.iter().any(|pl| pl.relics.contains(id));
        for own in offers.iter_mut() {
            for id in pool.iter().filter(|id| !held(self, id)) {
                if own.len() < 3 {
                    own.push(id.clone());
                }
            }
            pool.retain(|id| !own.contains(id));
        }
        // a pool run dry: fill up with what others have, never what the player has
        let all: Vec<Id> = c.relics.values().filter(|r| r.pool == RelicPool::Boss).map(|r| r.id.clone()).collect();
        for (q, own) in offers.iter_mut().enumerate() {
            for id in &all {
                if own.len() < 3 && !own.contains(id) && !self.players[q].relics.contains(id) {
                    own.push(id.clone());
                }
            }
        }
        offers
    }

    fn roll_relic(&mut self, c: &Content, p: usize) -> Option<Id> {
        let pl = &self.players[p];
        let pool: Vec<&RelicDef> = c
            .relics
            .values()
            .filter(|r| r.rarity != Rarity::Starter && (r.pool == RelicPool::Any || (r.pool == RelicPool::Book && self.unlocks.secrets.contains(&r.id))) && (r.hero.is_empty() || r.hero == pl.hero))
            .filter(|r| !pl.relics.contains(&r.id) && self.unlocks.relic_ok(r.rarity))
            .collect();
        // what another player has is found only when nothing else is left
        let unclaimed: Vec<&RelicDef> = pool.iter().copied().filter(|r| !self.players.iter().any(|o| o.relics.contains(&r.id))).collect();
        self.rng.pick(if unclaimed.is_empty() { &pool } else { &unclaimed }).map(|r| r.id.clone())
    }
}

/// A sensible action for a seat nobody is steering.
pub fn run_bot(c: &Content, run: &Run, player: u8) -> Option<RunAction> {
    let p = player as usize;
    match &run.stage {
        Stage::Map { votes } => {
            if votes.get(p)?.is_some() || run.players.get(p).is_some_and(|pl| pl.beheaded) {
                return None;
            }
            // the road is chosen by how much blood is left: the hurt look for a fire,
            // the hale for a fight worth a trophy
            let pl = run.players.get(p)?;
            let hale = pl.hp * 4 >= pl.max_hp * 3;
            let hurt = pl.hp * 2 < pl.max_hp;
            let worth = |kind: NodeKind| match kind {
                NodeKind::Chest => 6,
                NodeKind::Rest => if hurt { 7 } else if hale { 3 } else { 5 },
                NodeKind::Shop => if pl.gold >= 80 { 4 } else { 2 },
                NodeKind::Event => 3,
                NodeKind::Fight => if hurt { 1 } else { 3 },
                NodeKind::Elite => if hale { 4 } else { 0 },
                NodeKind::Boss => 0,
            };
            let choices = run.choices();
            // the first of the best, so that every seat of a party of autopilots votes alike
            let best = choices.iter().enumerate().max_by_key(|(i, (_, kind))| (worth(*kind), std::cmp::Reverse(*i))).map(|(i, _)| i as u8).unwrap_or(0);
            Some(RunAction::Vote(best))
        }
        Stage::Combat => {
            let combat = run.combat.as_ref()?;
            let h = combat.hero(player)?;
            (combat.phase == Phase::Heroes && !h.downed && !h.ready).then(|| RunAction::Combat(crate::bot::choose(c, combat, player)))
        }
        Stage::Reward { rewards } => {
            let r = rewards.get(p)?;
            (!r.done).then(|| RunAction::TakeCard(if r.cards.is_empty() { None } else { Some(0) }))
        }
        Stage::Rest { done } => {
            let pl = run.players.get(p)?;
            if *done.get(p)? {
                return None;
            }
            // nobody is left lying while there is water
            if let Some(who) = run.players.iter().position(|o| o.beheaded) {
                if !pl.beheaded {
                    return Some(RunAction::Raise { who: who as u8 });
                }
            }
            // hurt heroes sleep, healthy ones sharpen a card
            let upgrade = pl.deck.iter().position(|(id, up)| !up && c.cards.get(id).is_some_and(|d| d.up.is_some() && d.kind != CardType::Curse));
            Some(match upgrade {
                Some(i) if pl.hp * 10 >= pl.max_hp * 7 => RunAction::Upgrade(i as u16),
                _ => RunAction::Sleep,
            })
        }
        Stage::Shop { stock, done } => {
            if *done.get(p)? {
                return None;
            }
            // the autopilot buys the first thing it can afford, once, and moves on
            let pl = run.players.get(p)?;
            let afford = |shelf: &Vec<Option<(Id, i32)>>| shelf.iter().position(|s| s.as_ref().is_some_and(|(_, price)| *price <= pl.gold)).map(|i| i as u8);
            let s = stock.get(p)?;
            if let Some(i) = afford(&s.relics) {
                return Some(RunAction::BuyRelic(i));
            }
            if let Some(i) = afford(&s.cards) {
                return Some(RunAction::BuyCard(i));
            }
            Some(RunAction::LeaveShop)
        }
        Stage::Omen { id, picks } => {
            let def = c.events.get(id.as_str())?;
            if picks.get(p)?.is_some() {
                return None;
            }
            let pl = run.players.get(p)?;
            // the first option it can pay for (and, at a schism, that is still open to it)
            let closed = if def.kind == EventKind::Schism { schism_closed(picks, p) } else { None };
            let i = (0..def.options.len()).find(|i| def.options[*i].price <= pl.gold && closed != Some(*i as u8) && seam_open(&run.players, def, &def.options[*i], p, picks)).unwrap_or(def.options.len() - 1);
            Some(RunAction::Choose(i as u8))
        }
        Stage::Trophy { picks, .. } => picks.get(p)?.is_none().then_some(RunAction::Choose(0)),
        Stage::Chest(ch) => {
            if ch.settled {
                return (!*ch.done.get(p)?).then_some(RunAction::Onward);
            }
            if ch.picks.iter().any(|v| v.is_none()) {
                // the autopilot reaches for the first relic, and will fight for it
                return ch.picks.get(p)?.is_none().then_some(RunAction::Choose(0));
            }
            if !ch.contest.contains(&player) || ch.throws.get(p)?.is_some() {
                return None;
            }
            // a throw that looks random but is the same on every machine
            let mut x = (p as u32 + 1).wrapping_mul(0x9E37_79B1) ^ (ch.rounds + 1).wrapping_mul(0x85EB_CA6B);
            x ^= x >> 15;
            x = x.wrapping_mul(0x2C1B_3C6D);
            Some(RunAction::Throw(((x >> 16) % 3) as u8))
        }
        Stage::Victory | Stage::Defeat => None,
    }
}
