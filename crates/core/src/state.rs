//! Mutable combat state. Everything here is hashed for desync checks,
//! so no floats and no unordered maps.

use crate::data::*;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const HAND_LIMIT: usize = 10;
pub const BASE_ENERGY: i32 = 3;
pub const BASE_DRAW: i32 = 5;
pub const NAV_MARK: &str = "nav_mark";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum UnitRef {
    Hero(u8),
    Enemy(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub uid: u32,
    pub def: Id,
    pub upgraded: bool,
    /// An Echo copy: costs nothing and is gone once played.
    #[serde(default)]
    pub free: bool,
}

pub type Statuses = BTreeMap<StatusKind, i32>;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hero {
    pub slot: u8,
    pub def: Id,
    pub mechanic: Mechanic,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub energy: i32,
    pub statuses: Statuses,
    pub draw: Vec<Card>,
    pub hand: Vec<Card>,
    pub discard: Vec<Card>,
    pub exhaust: Vec<Card>,
    pub relics: Vec<Id>,
    pub powers: Vec<Passive>,
    pub ready: bool,
    pub downed: bool,
    /// Falls over the whole run, carried in and out of combat.
    pub falls: u32,
    /// Limbs struck off by heavy blows, gone for the rest of the run.
    #[serde(default)]
    pub lost_limbs: u8,
    /// Which: [`WEAPON_ARM`] and [`SHIELD_ARM`] as bits.
    #[serde(default)]
    pub lost_arms: u8,
    /// The head struck off: gone for good, and the player only watches from here on.
    #[serde(default)]
    pub beheaded: bool,
    /// Raised by water alone: up and fighting, with no head.
    #[serde(default)]
    pub walks_headless: bool,
    /// Curses earned this combat, to be added to the run deck afterwards.
    pub new_curses: u32,
    /// Parts cut off this fight, to be carried in the deck from now on.
    #[serde(default)]
    pub new_parts: Vec<Id>,
    /// Bogatyr slot soaking this hero's overflow damage until the round closes.
    pub guard_by: Option<u8>,
    /// Absorbed damage not yet converted into Valor.
    pub valor_rem: i32,
    pub runes: Vec<RuneKind>,
    pub form: Form,
    pub moon: i32,
    pub mask: Option<MaskKind>,
    /// Herbs waiting to be brewed.
    pub cauldron: Vec<Id>,
    pub potions: Vec<Id>,
    pub gold_lost: i32,
    /// The gold this hero came into the fight with (what a thief can take a share of).
    #[serde(default)]
    pub gold: i32,
    pub flags: BTreeMap<String, i32>,
    /// A card has asked this player to pick cards from the hand to throw away; nothing else is
    /// done until they have.
    #[serde(default)]
    pub choosing: Option<Choosing>,
}

/// What a card that says "discard" waits on: how many more cards the player is to pick, and what
/// the card goes on to do once they have.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Choosing {
    pub left: u8,
    /// How many have been picked for this card so far.
    pub picked: u8,
    pub rest: Vec<Effect>,
    pub target: Option<UnitRef>,
    pub x: i32,
    pub attack: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Intent {
    pub mv: usize,
    pub target: Option<u8>,
}

/// The flag on an enemy that fell once and lies as bones until its next turn.
pub const BONES: &str = "bones";

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Enemy {
    pub uid: u32,
    pub def: Id,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub statuses: Statuses,
    pub alive: bool,
    pub fled: bool,
    /// Moves taken so far; indexes `opening` then cycles `pattern`.
    pub step: usize,
    pub intent: Option<Intent>,
    /// Gold taken per hero slot, handed back if this enemy dies.
    pub stolen: BTreeMap<u8, i32>,
    pub flags: BTreeMap<String, i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    Heroes,
    Won,
    Lost,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Played {
    pub player: u8,
    pub card: Id,
    pub upgraded: bool,
    pub kind: CardType,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Combat {
    pub round: u32,
    pub phase: Phase,
    pub heroes: Vec<Hero>,
    pub enemies: Vec<Enemy>,
    pub rng: Rng,
    pub next_uid: u32,
    /// Cards played this round by anyone, oldest first.
    pub log: Vec<Played>,
    /// Energy for the "Raise" action; Circle 9 bumps it to 3.
    pub raise_cost: i32,
    pub hp_scale: i32,
    /// Enemy attack damage in percent (Circles of Nav raise it).
    pub enemy_dmg: i32,
    /// Circle 10: breaking the needle also lets Koschei's shadow loose.
    #[serde(default)]
    pub shadow: bool,
    /// Rounds this fight may run before the dark gathers (longer for elites and bosses).
    #[serde(default = "stall_default")]
    pub stall: u32,
    /// A copy played out to foretell a move: what falls on a random foe is left out, with no roll.
    #[serde(skip)]
    pub foretelling: bool,
}

/// Per-fight rule tweaks, set by the Circle of Nav and the kind of encounter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CombatOpts {
    pub raise_cost: i32,
    /// Enemy HP in percent on top of the party-size scaling.
    pub enemy_hp: i32,
    pub enemy_dmg: i32,
    pub shadow: bool,
    /// Rounds before the dark gathers on the heroes.
    pub stall: u32,
}

fn stall_default() -> u32 {
    crate::combat::STALL_ROUND
}

impl Default for CombatOpts {
    fn default() -> Self {
        CombatOpts { raise_cost: 2, enemy_hp: 100, enemy_dmg: 100, shadow: false, stall: crate::combat::STALL_ROUND }
    }
}

/// What a hero brings into a fight from the run.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HeroSetup {
    pub def: Id,
    pub hp: i32,
    pub max_hp: i32,
    /// `(card id, upgraded)`
    pub deck: Vec<(Id, bool)>,
    pub relics: Vec<Id>,
    pub falls: u32,
    #[serde(default)]
    pub potions: Vec<Id>,
    #[serde(default)]
    pub lost_limbs: u8,
    /// Which: [`WEAPON_ARM`] and [`SHIELD_ARM`] as bits.
    #[serde(default)]
    pub lost_arms: u8,
    #[serde(default)]
    pub beheaded: bool,
    /// Raised by water alone: up and fighting, with no head.
    #[serde(default)]
    pub walks_headless: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CombatAction {
    Play { card: u32, target: Option<UnitRef> },
    Ready(bool),
    /// Spend energy and discard a card to raise a downed ally.
    Raise { ally: u8, discard: u32 },
    /// Hand a passable card (Koschei's needle) to a standing ally for 1 Energy.
    Pass { card: u32, to: u8 },
    /// Drink the potion in this slot, or throw it at the target.
    UsePotion { slot: u8, target: Option<UnitRef> },
    /// Throw this card from the hand, as a card that says "discard" asked.
    Discard { card: u32 },
    /// Pour the potion in this slot out on the ground: nothing comes of it.
    DropPotion { slot: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionError {
    NotHeroPhase,
    NoSuchPlayer,
    Downed,
    NoSuchCard,
    Unplayable,
    NotEnoughEnergy,
    BadTarget,
    NotDowned,
    /// Only some cards can be handed over, and only to a standing ally.
    CannotPass,
    NoSuchPotion,
    /// Wrapped in the serpent's coils: no attacks until it lets go.
    Coiled,
    /// The arm this card needs has been cut off.
    NoArm,
    /// Sunk in the mire or put to sleep: no cards this turn.
    Held,
    /// This card is frozen in hand for the turn.
    Frozen,
    /// The target hides behind its fellows.
    Hidden,
    /// Cards are to be picked for a discard first.
    Choosing,
    /// No discard is asked for now.
    NotChoosing,
}

/// Things that happened, in order, for the renderer and the log. Never read back by rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    RoundStart(u32),
    Intent { enemy: u32, mv: usize, target: Option<u8> },
    Drawn { player: u8, count: u32 },
    Shuffled { player: u8 },
    CardPlayed { player: u8, card: Id, upgraded: bool, target: Option<UnitRef> },
    /// `hp` and `block` are the target's values after the hit.
    Damage { src: Option<UnitRef>, dst: UnitRef, blocked: i32, lost: i32, attack: bool, hp: i32, block: i32 },
    Redirected { from: u8, to: u8, amount: i32 },
    Block { unit: UnitRef, amount: i32, total: i32 },
    Status { unit: UnitRef, status: StatusKind, delta: i32, total: i32 },
    /// An enemy fell apart and will get up on its next turn.
    Collapsed { enemy: u32 },
    Heal { unit: UnitRef, amount: i32, hp: i32 },
    Energy { player: u8, delta: i32 },
    Downed { player: u8 },
    Raised { player: u8, by: Option<u8>, hp: i32 },
    /// A heavy blow struck off a limb: this is how many are gone now.
    Maimed { player: u8, limbs: u8, arm: u8 },
    /// A blow far past what the hero had left struck off the head.
    Beheaded { player: u8 },
    /// A blow found only air: the unit dodged it, or it was wasted on nobody.
    Missed { unit: UnitRef },
    CardAdded { player: u8, card: Id, pile: Pile },
    Exhausted { player: u8, card: Id },
    /// Thrown out of the hand by a card or a power.
    Discarded { player: u8, card: Id },
    /// This player is to pick `count` cards from the hand to throw away.
    Choose { player: u8, count: u8 },
    EnemyMove { enemy: u32, mv: usize },
    EnemySkipped { enemy: u32, fear: bool },
    EnemyDied { enemy: u32 },
    EnemyFled { enemy: u32 },
    Summoned { enemy: u32, def: Id },
    Ready { player: u8, ready: bool },
    EnemyPhase,
    RuneCarved { player: u8, rune: RuneKind },
    RuneFired { player: u8, rune: RuneKind },
    /// The oldest rune gave way to a new one, and fires one last time.
    RuneDischarged { player: u8, rune: RuneKind },
    /// The oldest herb was thrown out of a full cauldron.
    HerbSpilled { player: u8, herb: Id },
    HerbAdded { player: u8, herb: Id },
    Brewed { player: u8, recipe: Id },
    PotionUsed { player: u8, potion: Id, target: Option<UnitRef> },
    /// Poured out, to no effect.
    PotionDropped { player: u8, potion: Id },
    Passed { from: u8, to: u8, card: Id },
    /// An enemy passed into its second phase.
    Phase2 { enemy: u32 },
    Shifted { player: u8, form: Form },
    MaskChanged { player: u8, mask: MaskKind },
    Taunt { player: u8 },
    GoldStolen { player: u8, amount: i32 },
    /// A thief killed drops what it took back to its owner.
    GoldReturned { player: u8, amount: i32 },
    /// A foe threw the gold it held: it is lost to its owners.
    GoldScattered { enemy: u32, amount: i32 },
    Ended { won: bool },
}

impl Combat {
    pub fn hero(&self, slot: u8) -> Option<&Hero> {
        self.heroes.get(slot as usize)
    }
    pub fn enemy(&self, uid: u32) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.uid == uid)
    }
    /// Enemies that can act and be struck: a heap of bones getting up is neither.
    pub fn living_enemies(&self) -> impl Iterator<Item = &Enemy> {
        self.enemies.iter().filter(|e| e.alive && !e.flags.contains_key(BONES))
    }
    pub fn standing_heroes(&self) -> impl Iterator<Item = &Hero> {
        self.heroes.iter().filter(|h| !h.downed)
    }
    pub fn status(&self, u: UnitRef, s: StatusKind) -> i32 {
        let map = match u {
            UnitRef::Hero(i) => self.hero(i).map(|h| &h.statuses),
            UnitRef::Enemy(i) => self.enemy(i).map(|e| &e.statuses),
        };
        map.and_then(|m| m.get(&s).copied()).unwrap_or(0)
    }
    pub fn is_coop(&self) -> bool {
        self.heroes.len() > 1
    }

    /// Stable 64-bit FNV-1a digest of the whole state, compared across clients.
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
}
