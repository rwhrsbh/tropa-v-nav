//! Static game content: cards, heroes, relics, enemies. Loaded from RON.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub type Id = String;

macro_rules! data_enum {
    ($(#[$m:meta])* $name:ident { $($v:ident),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum $name { $($v),* }
    };
}

data_enum!(CardType { Attack, Skill, Power, Herb, Curse, Status });
data_enum!(Rarity { Starter, Common, Uncommon, Rare, Special });
data_enum!(
    /// `Coop` cards only show up in rewards with 2+ players.
    /// `Passable` cards can be handed to an ally for 1 Energy.
    Keyword { Exhaust, Retain, Ethereal, Unplayable, Coop, Passable, Armless }
);
data_enum!(
    /// What the player has to point a card at.
    CardTarget { None, Enemy, Ally, AnyHero, Downed }
);
data_enum!(
    /// Who an effect lands on, relative to its source.
    Tgt { Me, Target, AllFoes, RandomFoe, AllFriends, OtherFriends, WeakestFoe }
);
data_enum!(Pile { Hand, Draw, Discard });
data_enum!(RelicPool { Any, Shop, Boss, Book });
data_enum!(Form { Human, Beast });
data_enum!(MaskKind { Bear, Goat, Mara });
data_enum!(RuneKind { Thunder, Frost, Mist, Flame, Zhiva });
data_enum!(Mechanic { Valor, Curse, Runes, Moon, Brew, Masks, Sloth });
data_enum!(EncounterKind { Normal, Elite, Boss });
data_enum!(IntentKind { Attack, AttackAll, Defend, Buff, Debuff, Summon, Unknown, Special });

data_enum!(StatusKind {
    Strength,
    Drowse,
    TempStrength,
    Fortitude,
    Vulnerable,
    Weak,
    Rot,
    Hex,
    Bleed,
    Fear,
    Thorns,
    HuntMark,
    Valor,
    Stun,
    Mist,
    Chained,
    // the gathering dark of a fight drawn out too long: less block and healing, and HP lost
    // at each turn's end; laid on the heroes by the fight itself, never cleansed
    Gloom,
});

impl StatusKind {
    /// Counted in rounds and ticked down when the round closes.
    pub fn timed(self) -> bool {
        matches!(self, StatusKind::Vulnerable | StatusKind::Weak | StatusKind::Chained)
    }
    /// Gone when the round closes.
    pub fn round_only(self) -> bool {
        matches!(self, StatusKind::TempStrength | StatusKind::HuntMark)
    }
    pub fn debuff(self) -> bool {
        use StatusKind::*;
        matches!(self, Vulnerable | Weak | Rot | Hex | Bleed | Fear | HuntMark | Stun)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Amount {
    N(i32),
    /// Energy spent on an X-cost card, times the multiplier.
    X(i32),
    /// Stacks of a status on a unit, times the multiplier.
    Stat(StatusKind, Tgt, i32),
    /// Stacks swallowed by the last `Consume`, times the multiplier.
    Consumed(i32),
    /// Cards played by all heroes this round, times the multiplier.
    PlayedThisRound(i32),
    /// Runes of a kind across all heroes, times the multiplier.
    Runes(RuneKind, i32),
    /// Rot, Hex and Weak stacks together on a unit, times the multiplier.
    Curses(Tgt, i32),
    /// `.0` and `.1` for every other foe still standing beside the source: a pack bites harder.
    Pack(i32, i32),
    /// This percent of the health the source has lost.
    MissingHp(i32),
    /// Cards the last discard of the source hero threw away, times the multiplier.
    Discarded(i32),
    /// Cards the source hero has discarded this round, times the multiplier.
    DiscardedThisTurn(i32),
    /// Cards in the source hero's hand, times the multiplier.
    HandSize(i32),
    /// Cards the source hero has exhausted this fight, times the multiplier.
    ExhaustPile(i32),
    /// `.0` and `.1` percent of the gold the source foe holds: the more it throws, the harder it hits.
    Hoard(i32, i32),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cond {
    Has(StatusKind, Tgt, i32),
    /// An ally already played a card of this type this round.
    Link(CardType),
    /// The last damage effect killed its target.
    Fatal,
    Form(Form),
    Mask(MaskKind),
    Coop,
    Not(Box<Cond>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Effect {
    Damage(Amount, Tgt),
    /// Damage repeated `.1` times.
    Hits(Amount, i32, Tgt),
    Block(Amount, Tgt),
    Apply(StatusKind, Amount, Tgt),
    Remove(StatusKind, Tgt),
    Draw(Amount),
    Energy(Amount, Tgt),
    Heal(Amount, Tgt),
    LoseHp(Amount, Tgt),
    /// Raise a downed hero with this percent of max HP.
    Revive(i32, Tgt),
    AddCard { card: Id, pile: Pile, to: Tgt, #[serde(default = "one")] count: i32 },
    /// Block for a friend plus redirection of the overflow to the source.
    Guard(Amount, Tgt),
    /// Every enemy intent now points at the source.
    Taunt,
    /// Strip all stacks of a status and remember how many there were.
    Consume(StatusKind, Tgt),
    /// Strip Rot, Hex and Weak together and remember the total.
    ConsumeCurses(Tgt),
    /// Strip every debuff.
    Cleanse(Tgt),
    If(Cond, Vec<Effect>, #[serde(default)] Vec<Effect>),
    Repeat(Amount, Vec<Effect>),
    Rune(RuneKind, Tgt),
    Invoke,
    Shift,
    Mask(MaskKind),
    Summon(Id, #[serde(default = "one")] i32),
    StealGold(i32),
    /// Takes this percent of the gold the targeted hero still has.
    StealGoldPct(i32),
    /// The source throws away all the gold it holds: what it took is gone for good.
    ScatterHoard,
    Flee,
    /// Adds to a named counter on the source hero (`turn_*` counters reset every round).
    Flag(String, i32),
    /// Runs once per combat for this hero, keyed by name.
    Once(String, Vec<Effect>),
    /// Runs once per round for this hero, keyed by name.
    OncePerTurn(String, Vec<Effect>),
    /// Counts up each time it is reached, for this hero and by name, and runs on every `.1`-th
    /// time; the count then starts again. Kept through the fight.
    Every(String, i32, Vec<Effect>),
    /// As `Every`, but the count starts again with every round.
    EveryThisTurn(String, i32, Vec<Effect>),
    MaxHp(Amount, Tgt),
    /// A free, exhausting copy of the last card an ally played this round
    /// (your own last card when alone) into the hands of these heroes.
    Echo(Tgt),
    /// Drops this herb into the source hero's cauldron.
    Herb(Id),
    /// Empties the cauldron and resolves its recipe.
    Brew,
    /// The targeted enemy now aims its move at the source hero.
    Provoke,
    /// Throws this many random cards from the source hero's hand onto the discard pile.
    Discard(Amount),
    /// Throws the whole hand onto the discard pile.
    DiscardHand,
    /// Burns this many random cards from the source hero's hand.
    ExhaustCards(Amount),
    /// Brings this many random cards back from the discard pile into the hand.
    Recall(Amount),
    /// The pike's will: the card on top of the source hero's draw pile plays itself, for nothing.
    PikeWill,
    /// Escape hatch into named Rust code for one-off mechanics.
    Hook(String),
}

fn one() -> i32 {
    1
}

/// The name a hero's flags keep the count of an `Every` (or an `EveryThisTurn`) under.
pub fn counter_key(name: &str, this_turn: bool) -> String {
    // per-round keys live under `turn_`, which the round start wipes
    if this_turn { format!("turn_count_{name}") } else { format!("count_{name}") }
}

data_enum!(Trigger {
    CombatStart,
    TurnStart,
    TurnEnd,
    CardPlayed,
    AttackPlayed,
    AllyHpLoss,
    Kill,
    SkillPlayed,
    // an enemy's blow cost you HP, and you are still standing
    Hurt,
    Shifted,
    MaskChanged,
    AllyRaised,
    // a card of yours was thrown away by a card or a power, not by the end of the turn
    Discarded,
    Exhausted,
});

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Passive {
    pub on: Trigger,
    pub effects: Vec<Effect>,
}

/// Overrides layered on a card: its upgrade, or the Beast side of a werewolf card.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct CardPatch {
    pub cost: Option<i32>,
    pub drowse: Option<i32>,
    pub kind: Option<CardType>,
    pub target: Option<CardTarget>,
    pub effects: Option<Vec<Effect>>,
    pub passives: Option<Vec<Passive>>,
    pub keywords: Option<Vec<Keyword>>,
    pub on_discard: Option<Vec<Effect>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardDef {
    pub id: Id,
    /// Owning hero id, or empty for neutral, curse and status cards.
    #[serde(default)]
    pub hero: Id,
    pub kind: CardType,
    #[serde(default = "starter")]
    pub rarity: Rarity,
    #[serde(default)]
    pub cost: i32,
    /// Drowse the card is paid for with, beside (or instead of) energy.
    #[serde(default)]
    pub drowse: i32,
    #[serde(default)]
    pub x_cost: bool,
    #[serde(default = "no_target")]
    pub target: CardTarget,
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// Attached to the hero for the rest of combat when a Power is played.
    #[serde(default)]
    pub passives: Vec<Passive>,
    #[serde(default)]
    pub keywords: Vec<Keyword>,
    /// Runs at end of turn while the card sits in hand (curses).
    #[serde(default)]
    pub in_hand_end: Vec<Effect>,
    /// Runs when a card or a power throws this card out of the hand.
    #[serde(default)]
    pub on_discard: Vec<Effect>,
    /// Hero level that opens this card in rewards; 0 means by its rarity.
    #[serde(default)]
    pub level: u8,
    #[serde(default)]
    pub up: Option<CardPatch>,
    #[serde(default)]
    pub beast: Option<CardPatch>,
    #[serde(default)]
    pub beast_up: Option<CardPatch>,
}

fn starter() -> Rarity {
    Rarity::Starter
}
fn no_target() -> CardTarget {
    CardTarget::None
}

/// A card as it plays right now, with upgrade and form applied.
#[derive(Clone, Debug)]
pub struct CardView {
    pub cost: i32,
    pub drowse: i32,
    pub x_cost: bool,
    pub kind: CardType,
    pub target: CardTarget,
    pub effects: Vec<Effect>,
    pub passives: Vec<Passive>,
    pub keywords: Vec<Keyword>,
    pub on_discard: Vec<Effect>,
}

impl CardView {
    pub fn has(&self, k: Keyword) -> bool {
        self.keywords.contains(&k)
    }
}

impl CardDef {
    pub fn view(&self, upgraded: bool, form: Form) -> CardView {
        let mut v = CardView {
            cost: self.cost,
            drowse: self.drowse,
            x_cost: self.x_cost,
            kind: self.kind,
            target: self.target,
            effects: self.effects.clone(),
            passives: self.passives.clone(),
            keywords: self.keywords.clone(),
            on_discard: self.on_discard.clone(),
        };
        let mut layer = |p: &Option<CardPatch>| {
            let Some(p) = p else { return };
            if let Some(c) = p.cost {
                v.cost = c;
            }
            if let Some(d) = p.drowse {
                v.drowse = d;
            }
            if let Some(k) = p.kind {
                v.kind = k;
            }
            if let Some(t) = p.target {
                v.target = t;
            }
            if let Some(e) = &p.effects {
                v.effects = e.clone();
            }
            if let Some(e) = &p.passives {
                v.passives = e.clone();
            }
            if let Some(k) = &p.keywords {
                v.keywords = k.clone();
            }
            if let Some(e) = &p.on_discard {
                v.on_discard = e.clone();
            }
        };
        let beast = form == Form::Beast && self.beast.is_some();
        if upgraded {
            layer(&self.up);
        }
        if beast {
            layer(&self.beast);
            if upgraded {
                layer(&self.beast_up);
            }
        } else if form == Form::Beast && self.hero == "volkolak" {
            // every card of the werewolf has a beast side: one written for it, or this one
            let mut fx = beastly(&v.effects, v.cost);
            // a blow that bleeds no one in particular is struck the harder instead
            if v.kind == CardType::Attack && fx == v.effects {
                fx.insert(0, Effect::Apply(StatusKind::TempStrength, Amount::N(2), Tgt::Me));
            }
            v.effects = fx;
        }
        v
    }

    /// Whether this card reads differently in the two forms, so both sides are worth telling.
    pub fn two_sided(&self, upgraded: bool) -> bool {
        let (a, b) = (self.view(upgraded, Form::Human), self.view(upgraded, Form::Beast));
        a.effects != b.effects || a.cost != b.cost || a.kind != b.kind || a.target != b.target || a.keywords != b.keywords
    }
}

/// The beast side of a werewolf card with none of its own: its blows tear and leave the wound
/// bleeding, and the block it would give itself (the beast gets none from its own cards)
/// bristles into thorns instead. Help for others, draws and powers stay as they are.
fn beastly(effects: &[Effect], cost: i32) -> Vec<Effect> {
    let bleed = if cost >= 2 { 2 } else { 1 };
    let mut out = Vec::new();
    let mut bled = false;
    for e in effects {
        match e {
            Effect::Block(a, Tgt::Me) => {
                let n = match a {
                    Amount::N(n) => (*n / 3).max(1),
                    _ => 2,
                };
                out.push(Effect::Apply(StatusKind::Thorns, Amount::N(n), Tgt::Me));
            }
            Effect::Damage(_, t) | Effect::Hits(_, _, t) => {
                out.push(e.clone());
                if !bled && matches!(t, Tgt::Target | Tgt::AllFoes | Tgt::WeakestFoe) {
                    bled = true;
                    out.push(Effect::Apply(StatusKind::Bleed, Amount::N(bleed), *t));
                }
            }
            Effect::If(c, a, b) => out.push(Effect::If(c.clone(), beastly(a, cost), beastly(b, cost))),
            Effect::Repeat(n, body) => out.push(Effect::Repeat(n.clone(), beastly(body, cost))),
            other => out.push(other.clone()),
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HeroDef {
    pub id: Id,
    pub max_hp: i32,
    pub mechanic: Mechanic,
    pub deck: Vec<Id>,
    pub relic: Id,
    /// Other relics the hero may start with instead, once the village shrine stands.
    #[serde(default)]
    pub alt_relics: Vec<Id>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelicDef {
    pub id: Id,
    /// Where it turns up: anywhere, only on the pedlar's shelves, or as a boss trophy.
    #[serde(default = "any_pool")]
    pub pool: RelicPool,
    #[serde(default = "common")]
    pub rarity: Rarity,
    #[serde(default)]
    pub hero: Id,
    #[serde(default)]
    pub passives: Vec<Passive>,
}

impl RelicDef {
    /// The count this relic keeps, if it keeps one: the flag it is under, and what it runs to.
    pub fn counter(&self) -> Option<(String, i32)> {
        fn find(effects: &[Effect]) -> Option<(String, i32)> {
            effects.iter().find_map(|e| match e {
                Effect::Every(name, n, _) => Some((counter_key(name, false), *n)),
                Effect::EveryThisTurn(name, n, _) => Some((counter_key(name, true), *n)),
                Effect::If(_, a, b) => find(a).or_else(|| find(b)),
                Effect::Repeat(_, a) | Effect::Once(_, a) | Effect::OncePerTurn(_, a) => find(a),
                _ => None,
            })
        }
        self.passives.iter().find_map(|p| find(&p.effects))
    }
}

fn any_pool() -> RelicPool {
    RelicPool::Any
}

fn common() -> Rarity {
    Rarity::Common
}

/// A one-use draught. Three slots per hero; drunk on your turn or thrown to an ally.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PotionDef {
    pub id: Id,
    #[serde(default = "no_target")]
    pub target: CardTarget,
    pub effects: Vec<Effect>,
}

data_enum!(
    /// `Sacrifice`: one pays an option's `fx` and everyone gets its `all`.
    /// `Schism`: each goes their own way, but the party may not all go the same one.
    EventKind { Personal, Vote, Sacrifice, Schism }
);

/// Something that happens to a player outside of combat.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunFx {
    Gold(i32),
    Hp(i32),
    /// Percent of max HP, negative to hurt.
    HpPct(i32),
    MaxHp(i32),
    Card(Id),
    /// A random card of the player's own hero of this rarity.
    RandomCard(Rarity),
    Relic,
    Potion,
    RemoveRandomCard,
    UpgradeRandomCard,
    /// The first of the party without a head gets one back, and much of their health goes with it.
    SewHeadOn,
    /// The chooser gets a lost limb back, and much of their health goes with it.
    SewLimbOn,
    /// The chooser sits down to play with the hollow: nothing happens yet, the stakes come after.
    Gamble,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventOption {
    pub id: Id,
    /// Gold the player must have (and pays) to take this option.
    #[serde(default)]
    pub price: i32,
    #[serde(default)]
    pub fx: Vec<RunFx>,
    /// What the whole party gets when one of them takes this option (sacrifice omens).
    #[serde(default)]
    pub all: Vec<RunFx>,
}

/// An omen on the path: a short eerie tale with a choice.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventDef {
    pub id: Id,
    /// 0 = any act.
    #[serde(default)]
    pub act: u8,
    pub kind: EventKind,
    pub options: Vec<EventOption>,
    /// Comes only while someone of a party has lost a limb or their head, and someone is left to ask.
    #[serde(default)]
    pub for_the_maimed: bool,
}

/// What a set of herbs brews into. `herbs` is a multiset; `"*"` matches any herb.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecipeDef {
    pub id: Id,
    pub herbs: Vec<Id>,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MoveDef {
    pub id: Id,
    pub intent: IntentKind,
    /// Aimed at one hero, picked when the intent is shown.
    #[serde(default)]
    pub targeted: bool,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EnemyDef {
    pub id: Id,
    /// Inclusive HP range for one player.
    pub hp: (i32, i32),
    pub moves: Vec<MoveDef>,
    /// Played once, before the cycle starts.
    #[serde(default)]
    pub opening: Vec<usize>,
    /// Indices into `moves`, repeated forever.
    pub pattern: Vec<usize>,
    /// Runs when the enemy dies.
    #[serde(default)]
    pub on_death: Vec<Effect>,
    /// Named traits the rules check for: `immortal`, `dodge`, `flying`.
    #[serde(default)]
    pub flags: Vec<Id>,
    /// Block it already has when it appears.
    #[serde(default)]
    pub start_block: i32,
    /// Second phase: at or below this percent of HP the enemy switches to `phase2`.
    #[serde(default)]
    pub phase2_at: i32,
    /// Pattern of the second phase (indices into `moves`).
    #[serde(default)]
    pub phase2: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EncounterDef {
    pub id: Id,
    pub act: u8,
    pub kind: EncounterKind,
    pub enemies: Vec<Id>,
    /// Faced instead of `enemies` by a lone hero.
    #[serde(default)]
    pub solo: Vec<Id>,
    /// Joins with 3 players (doc: +0-1), rolled 50/50.
    #[serde(default)]
    pub extra3: Vec<Id>,
    /// Joins with 4 players.
    #[serde(default)]
    pub extra4: Vec<Id>,
}

#[derive(Clone, Debug, Default)]
pub struct Content {
    pub cards: BTreeMap<Id, CardDef>,
    pub heroes: BTreeMap<Id, HeroDef>,
    pub relics: BTreeMap<Id, RelicDef>,
    pub enemies: BTreeMap<Id, EnemyDef>,
    pub encounters: BTreeMap<Id, EncounterDef>,
    /// Most specific first: the first recipe that matches the cauldron wins.
    pub recipes: Vec<RecipeDef>,
    pub potions: BTreeMap<Id, PotionDef>,
    pub events: BTreeMap<Id, EventDef>,
}

#[derive(Debug)]
pub struct LoadError(pub String);

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for LoadError {}

fn parse<T: serde::de::DeserializeOwned>(src: &str, what: &str) -> Result<Vec<T>, LoadError> {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(src)
        .map_err(|e| LoadError(format!("{what}: {e}")))
}

/// Enemy HP multiplier in permille by player count (design doc, section 2).
pub fn hp_scale(players: usize) -> i32 {
    match players {
        0 | 1 => 1000,
        2 => 1800,
        3 => 2500,
        _ => 3200,
    }
}

impl Content {
    pub fn add_cards(&mut self, src: &str) -> Result<(), LoadError> {
        for c in parse::<CardDef>(src, "cards")? {
            self.cards.insert(c.id.clone(), c);
        }
        Ok(())
    }
    pub fn add_heroes(&mut self, src: &str) -> Result<(), LoadError> {
        for c in parse::<HeroDef>(src, "heroes")? {
            self.heroes.insert(c.id.clone(), c);
        }
        Ok(())
    }
    pub fn add_relics(&mut self, src: &str) -> Result<(), LoadError> {
        for c in parse::<RelicDef>(src, "relics")? {
            self.relics.insert(c.id.clone(), c);
        }
        Ok(())
    }
    pub fn add_enemies(&mut self, src: &str) -> Result<(), LoadError> {
        for c in parse::<EnemyDef>(src, "enemies")? {
            self.enemies.insert(c.id.clone(), c);
        }
        Ok(())
    }
    pub fn add_encounters(&mut self, src: &str) -> Result<(), LoadError> {
        for c in parse::<EncounterDef>(src, "encounters")? {
            self.encounters.insert(c.id.clone(), c);
        }
        Ok(())
    }

    pub fn add_recipes(&mut self, src: &str) -> Result<(), LoadError> {
        self.recipes.extend(parse::<RecipeDef>(src, "recipes")?);
        // exact recipes before wildcard ones, bigger before smaller
        self.recipes.sort_by_key(|r| (r.herbs.iter().filter(|h| *h == "*").count(), std::cmp::Reverse(r.herbs.len())));
        Ok(())
    }

    pub fn add_potions(&mut self, src: &str) -> Result<(), LoadError> {
        for p in parse::<PotionDef>(src, "potions")? {
            self.potions.insert(p.id.clone(), p);
        }
        Ok(())
    }
    pub fn add_events(&mut self, src: &str) -> Result<(), LoadError> {
        for e in parse::<EventDef>(src, "events")? {
            self.events.insert(e.id.clone(), e);
        }
        Ok(())
    }

    /// Loads every `.ron` under `dir/{cards,heroes,relics,enemies,encounters,recipes}`.
    pub fn load_dir(dir: &Path) -> Result<Content, LoadError> {
        Content::load_with(&|sub| {
            let Ok(rd) = std::fs::read_dir(dir.join(sub)) else { return Ok(Vec::new()) };
            let mut files: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron")).collect();
            files.sort();
            files
                .into_iter()
                .map(|file| std::fs::read_to_string(&file).map(|src| (file.display().to_string(), src)).map_err(|e| LoadError(format!("{}: {e}", file.display()))))
                .collect()
        })
    }

    /// Loads the content from wherever `files` finds it: for each folder (`cards`, `heroes`, ...)
    /// the name and text of every `.ron` in it, in order. A phone reads them out of its package.
    pub fn load_with(files: &dyn Fn(&str) -> Result<Vec<(String, String)>, LoadError>) -> Result<Content, LoadError> {
        let mut c = Content::default();
        let each = |sub: &str, f: &mut dyn FnMut(&str) -> Result<(), LoadError>| {
            for (name, src) in files(sub)? {
                f(&src).map_err(|e| LoadError(format!("{name}: {}", e.0)))?;
            }
            Ok(())
        };
        each("cards", &mut |s| c.add_cards(s))?;
        each("heroes", &mut |s| c.add_heroes(s))?;
        each("relics", &mut |s| c.add_relics(s))?;
        each("enemies", &mut |s| c.add_enemies(s))?;
        each("encounters", &mut |s| c.add_encounters(s))?;
        each("recipes", &mut |s| c.add_recipes(s))?;
        each("potions", &mut |s| c.add_potions(s))?;
        each("events", &mut |s| c.add_events(s))?;
        c.validate()?;
        Ok(c)
    }

    /// Catches dangling ids at load time instead of mid-combat.
    pub fn validate(&self) -> Result<(), LoadError> {
        let mut bad = Vec::new();
        fn walk(effects: &[Effect], f: &mut dyn FnMut(&Effect)) {
            for e in effects {
                f(e);
                match e {
                    Effect::If(_, a, b) => {
                        walk(a, f);
                        walk(b, f);
                    }
                    Effect::Repeat(_, a) | Effect::Once(_, a) | Effect::OncePerTurn(_, a) | Effect::Every(_, _, a) | Effect::EveryThisTurn(_, _, a) => walk(a, f),
                    _ => {}
                }
            }
        }
        let check = |bad: &mut Vec<String>, owner: &str, effects: &[Effect]| {
            walk(effects, &mut |e| match e {
                Effect::AddCard { card, .. } if !self.cards.contains_key(card) => {
                    bad.push(format!("{owner}: unknown card `{card}`"))
                }
                Effect::Summon(id, _) if !self.enemies.contains_key(id) => {
                    bad.push(format!("{owner}: unknown enemy `{id}`"))
                }
                _ => {}
            });
        };
        for c in self.cards.values() {
            check(&mut bad, &c.id, &c.effects);
            check(&mut bad, &c.id, &c.in_hand_end);
            for p in [&c.up, &c.beast, &c.beast_up].into_iter().flatten() {
                if let Some(e) = &p.effects {
                    check(&mut bad, &c.id, e);
                }
            }
        }
        for r in self.relics.values() {
            for p in &r.passives {
                check(&mut bad, &r.id, &p.effects);
            }
        }
        for e in self.enemies.values() {
            check(&mut bad, &e.id, &e.on_death);
            for m in &e.moves {
                check(&mut bad, &e.id, &m.effects);
            }
            for &i in e.opening.iter().chain(&e.pattern).chain(&e.phase2) {
                if i >= e.moves.len() {
                    bad.push(format!("{}: pattern index {i} out of range", e.id));
                }
            }
            if e.pattern.is_empty() {
                bad.push(format!("{}: empty pattern", e.id));
            }
        }
        for r in &self.recipes {
            check(&mut bad, &r.id, &r.effects);
        }
        for p in self.potions.values() {
            check(&mut bad, &p.id, &p.effects);
        }
        for e in self.events.values() {
            if e.options.is_empty() {
                bad.push(format!("{}: an omen needs at least one option", e.id));
            }
            for o in &e.options {
                for fx in &o.fx {
                    if let RunFx::Card(card) = fx {
                        if !self.cards.contains_key(card) {
                            bad.push(format!("{}: unknown card `{card}`", e.id));
                        }
                    }
                }
            }
        }
        for h in self.heroes.values() {
            for c in &h.deck {
                if !self.cards.contains_key(c) {
                    bad.push(format!("{}: unknown deck card `{c}`", h.id));
                }
            }
            if !self.relics.contains_key(&h.relic) {
                bad.push(format!("{}: unknown relic `{}`", h.id, h.relic));
            }
        }
        for enc in self.encounters.values() {
            for e in enc.enemies.iter().chain(&enc.solo).chain(&enc.extra3).chain(&enc.extra4) {
                if !self.enemies.contains_key(e) {
                    bad.push(format!("{}: unknown enemy `{e}`", enc.id));
                }
            }
        }
        if bad.is_empty() {
            Ok(())
        } else {
            Err(LoadError(bad.join("\n")))
        }
    }
}
