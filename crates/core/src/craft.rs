//! The rules of the three later heroes: Kuzma's forge and plate, Ivanushka's chance and luck,
//! Lisa's copies of others' cards, her dodge and her charms.

use crate::combat::{Ctx, Exec};
use crate::data::*;
use crate::state::*;
use StatusKind as S;

/// Forging it takes for Kuzma to put his plate on.
pub const FORGE_TO_PLATE: i32 = 15;
/// What the plate holds, and with Heavy Plate, and with Svyatogor's.
pub const PLATE: i32 = 40;
pub const PLATE_HEAVY: i32 = 50;
pub const PLATE_SVYATOGOR: i32 = 60;
/// Cards in Kuzma's hand the forge marks at the start of each of his turns.
pub const MARKS_A_TURN: i32 = 2;
/// Cards Lisa draws of her own each turn; one more comes from someone else.
pub const LISA_OWN_DRAW: i32 = 4;
/// Of the gold a fight leaves her, the share Lisa lays by, and what it comes back with.
pub const STASH_SHARE: i32 = 30;
pub const STASH_BACK: i32 = 150;
pub const STASH_CAP: i32 = 60;
/// On the last floor every so much of the stash is a Strength for the fight.
/// The least Strength «Позаимствовать» brings, even from a target with none.
pub const BORROW_MIN: i32 = 2;
pub const STASH_TO_STRENGTH: i32 = 15;
/// The card a foe's move is copied onto.
pub const STOLEN_MOVE: &str = "li_echo_move";

/// Every range (`Amount::Roll`) of these effects, in the order they are written.
pub fn ranges(effects: &[Effect]) -> Vec<(i32, i32)> {
    fn amount(a: &Amount, out: &mut Vec<(i32, i32)>) {
        match a {
            Amount::Roll(lo, hi) => out.push((*lo, *hi)),
            Amount::Sum(list) => list.iter().for_each(|a| amount(a, out)),
            Amount::AtLeast(a, _) => amount(a, out),
            _ => {}
        }
    }
    fn walk(list: &[Effect], out: &mut Vec<(i32, i32)>) {
        for e in list {
            match e {
                Effect::Damage(a, _)
                | Effect::Hits(a, _, _)
                | Effect::Block(a, _)
                | Effect::Apply(_, a, _)
                | Effect::Draw(a)
                | Effect::Energy(a, _)
                | Effect::Heal(a, _)
                | Effect::Discard(a)
                | Effect::DiscardRandom(a)
                | Effect::DrawFor(a, _)
                | Effect::Forge(a)
                | Effect::Upgrade(a, _) => amount(a, out),
                Effect::Repeat(a, body) => {
                    amount(a, out);
                    walk(body, out);
                }
                Effect::If(_, a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                Effect::Once(_, a) | Effect::OncePerTurn(_, a) | Effect::Every(_, _, a) | Effect::EveryThisTurn(_, _, a) => walk(a, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(effects, &mut out);
    out
}

/// The same effects with each range, in order, replaced by the number thrown for it.
pub fn settle_rolls(effects: &[Effect], rolls: &[i32]) -> Vec<Effect> {
    fn amount(a: &Amount, rolls: &[i32], at: &mut usize) -> Amount {
        match a {
            Amount::Roll(lo, _) => {
                let v = rolls.get(*at).copied().unwrap_or(*lo);
                *at += 1;
                Amount::N(v)
            }
            Amount::Sum(list) => Amount::Sum(list.iter().map(|a| amount(a, rolls, at)).collect()),
            Amount::AtLeast(a, n) => Amount::AtLeast(Box::new(amount(a, rolls, at)), *n),
            other => other.clone(),
        }
    }
    fn walk(list: &[Effect], rolls: &[i32], at: &mut usize) -> Vec<Effect> {
        list.iter()
            .map(|e| match e {
                Effect::Damage(a, t) => Effect::Damage(amount(a, rolls, at), *t),
                Effect::Hits(a, k, t) => Effect::Hits(amount(a, rolls, at), *k, *t),
                Effect::Block(a, t) => Effect::Block(amount(a, rolls, at), *t),
                Effect::Apply(s, a, t) => Effect::Apply(*s, amount(a, rolls, at), *t),
                Effect::Draw(a) => Effect::Draw(amount(a, rolls, at)),
                Effect::Energy(a, t) => Effect::Energy(amount(a, rolls, at), *t),
                Effect::Heal(a, t) => Effect::Heal(amount(a, rolls, at), *t),
                Effect::Discard(a) => Effect::Discard(amount(a, rolls, at)),
                Effect::DiscardRandom(a) => Effect::DiscardRandom(amount(a, rolls, at)),
                Effect::DrawFor(a, t) => Effect::DrawFor(amount(a, rolls, at), *t),
                Effect::Forge(a) => Effect::Forge(amount(a, rolls, at)),
                Effect::Upgrade(a, t) => Effect::Upgrade(amount(a, rolls, at), *t),
                Effect::Repeat(a, body) => {
                    let a = amount(a, rolls, at);
                    Effect::Repeat(a, walk(body, rolls, at))
                }
                Effect::If(c, a, b) => {
                    let a = walk(a, rolls, at);
                    Effect::If(c.clone(), a, walk(b, rolls, at))
                }
                Effect::Once(n, a) => Effect::Once(n.clone(), walk(a, rolls, at)),
                Effect::OncePerTurn(n, a) => Effect::OncePerTurn(n.clone(), walk(a, rolls, at)),
                Effect::Every(n, k, a) => Effect::Every(n.clone(), *k, walk(a, rolls, at)),
                Effect::EveryThisTurn(n, k, a) => Effect::EveryThisTurn(n.clone(), *k, walk(a, rolls, at)),
                other => other.clone(),
            })
            .collect()
    }
    let mut at = 0;
    walk(effects, rolls, &mut at)
}

/// The same effects with both ends of every range moved by `by`.
pub fn bump_rolls(effects: &[Effect], by: i32) -> Vec<Effect> {
    if by == 0 {
        return effects.to_vec();
    }
    let n = ranges(effects).len();
    // settle with a marker the walk will not meet, then put the bumped ranges back
    fn amount(a: &Amount, by: i32) -> Amount {
        match a {
            Amount::Roll(lo, hi) => Amount::Roll((lo + by).max(0), (hi + by).max(0)),
            Amount::Sum(list) => Amount::Sum(list.iter().map(|a| amount(a, by)).collect()),
            Amount::AtLeast(a, k) => Amount::AtLeast(Box::new(amount(a, by)), *k),
            other => other.clone(),
        }
    }
    fn walk(list: &[Effect], by: i32) -> Vec<Effect> {
        list.iter()
            .map(|e| match e {
                Effect::Damage(a, t) => Effect::Damage(amount(a, by), *t),
                Effect::Hits(a, k, t) => Effect::Hits(amount(a, by), *k, *t),
                Effect::Block(a, t) => Effect::Block(amount(a, by), *t),
                Effect::Apply(s, a, t) => Effect::Apply(*s, amount(a, by), *t),
                Effect::Draw(a) => Effect::Draw(amount(a, by)),
                Effect::Energy(a, t) => Effect::Energy(amount(a, by), *t),
                Effect::Heal(a, t) => Effect::Heal(amount(a, by), *t),
                Effect::Discard(a) => Effect::Discard(amount(a, by)),
                Effect::DiscardRandom(a) => Effect::DiscardRandom(amount(a, by)),
                Effect::DrawFor(a, t) => Effect::DrawFor(amount(a, by), *t),
                Effect::Repeat(a, body) => Effect::Repeat(amount(a, by), walk(body, by)),
                Effect::If(c, a, b) => Effect::If(c.clone(), walk(a, by), walk(b, by)),
                other => other.clone(),
            })
            .collect()
    }
    let _ = n;
    walk(effects, by)
}

/// The cards a pick may be made from, for the screen and the autopilot.
pub fn pickable_cards(c: &Content, s: &Combat, p: &Picking) -> Vec<u32> {
    Exec::pickable(c, s, p, 0)
}

/// Is this card one of Ivanushka's cards of chance: a number on it falls only when it is played or drawn.
pub fn ranged(view: &CardView) -> bool {
    !ranges(&view.effects).is_empty()
}

/// What a card of luck still has to say: is it an evil one, falling on whoever it finds.
pub fn evil_luck(effects: &[Effect]) -> bool {
    effects.iter().any(|e| matches!(e, Effect::Luck { friend, .. } if friend.is_empty()))
}

pub fn luck_card(effects: &[Effect]) -> bool {
    effects.iter().any(|e| matches!(e, Effect::Luck { .. }))
}

impl<'a> Exec<'a> {
    pub(crate) fn hero_mut(&mut self, slot: u8) -> &mut Hero {
        &mut self.s.heroes[slot as usize]
    }

    pub(crate) fn flag(&self, slot: u8, key: &str) -> i32 {
        self.s.heroes[slot as usize].flags.get(key).copied().unwrap_or(0)
    }

    pub(crate) fn in_plate(&self, slot: u8) -> bool {
        self.s.heroes[slot as usize].mechanic == Mechanic::Forge && self.s.heroes[slot as usize].form == Form::Beast
    }

    // ---- Kuzma ----

    /// The forge rises by `n`; at fifteen the plate goes on. In the plate there is nothing to forge.
    pub(crate) fn forge(&mut self, slot: u8, n: i32) {
        let h = &self.s.heroes[slot as usize];
        if n <= 0 || h.downed || h.mechanic != Mechanic::Forge || self.in_plate(slot) {
            return;
        }
        let u = UnitRef::Hero(slot);
        self.add_status(u, S::Forge, n);
        *self.hero_mut(slot).flags.entry("turn_forged".into()).or_insert(0) += n;
        let total = self.s.status(u, S::Forge);
        self.ev.push(Event::Forged { player: slot, total });
        if total >= FORGE_TO_PLATE {
            self.don_plate(slot);
        }
    }

    pub(crate) fn don_plate(&mut self, slot: u8) {
        if self.s.heroes[slot as usize].mechanic != Mechanic::Forge || self.in_plate(slot) {
            return;
        }
        let u = UnitRef::Hero(slot);
        let forge = self.s.status(u, S::Forge);
        self.add_status(u, S::Forge, -forge);
        let plate = if self.flag(slot, "svyatogor") > 0 {
            PLATE_SVYATOGOR
        } else if self.flag(slot, "heavy_plate") > 0 {
            PLATE_HEAVY
        } else {
            PLATE
        };
        let h = self.hero_mut(slot);
        h.form = Form::Beast;
        h.block = 0;
        self.add_status(u, S::Plate, plate);
        self.ev.push(Event::Plated { player: slot, on: true });
        self.ev.push(Event::Shifted { player: slot, form: Form::Beast });
        self.fire(slot, Trigger::PlateOn);
    }

    pub(crate) fn doff_plate(&mut self, slot: u8) {
        if !self.in_plate(slot) {
            return;
        }
        let u = UnitRef::Hero(slot);
        let left = self.s.status(u, S::Plate);
        self.add_status(u, S::Plate, -left);
        let forge = self.s.status(u, S::Forge);
        self.add_status(u, S::Forge, -forge);
        self.hero_mut(slot).form = Form::Human;
        self.ev.push(Event::Plated { player: slot, on: false });
        self.ev.push(Event::Shifted { player: slot, form: Form::Human });
        self.fire(slot, Trigger::PlateOff);
    }

    /// Marks up to `n` unmarked cards in the hand with the forge.
    pub(crate) fn mark_cards(&mut self, slot: u8, n: i32) {
        for _ in 0..n.max(0) {
            let h = &self.s.heroes[slot as usize];
            let free: Vec<usize> = (0..h.hand.len()).filter(|i| !h.hand[*i].forged && !h.hand[*i].alien).collect();
            let Some(&i) = self.s.rng.pick(&free) else { return };
            self.hero_mut(slot).hand[i].forged = true;
        }
    }

    /// What a marked card adds to the forge as it is played.
    pub(crate) fn forge_mark(&mut self, slot: u8) {
        let mut n = 1;
        if self.hero_mut(slot).flags.remove("temper_next").is_some() {
            n += 1;
        }
        n += self.flag(slot, "turn_warm_forge");
        *self.hero_mut(slot).flags.entry("turn_forge_cards".into()).or_insert(0) += 1;
        self.forge(slot, n);
        self.fire(slot, Trigger::ForgePlayed);
    }

    /// The plate is mended by `n`, or spent; spent to nothing, it comes off.
    pub(crate) fn mend_plate(&mut self, slot: u8, n: i32) {
        if !self.in_plate(slot) {
            return;
        }
        let u = UnitRef::Hero(slot);
        let now = self.s.status(u, S::Plate);
        let delta = n.max(-now);
        self.add_status(u, S::Plate, delta);
        if self.s.status(u, S::Plate) <= 0 {
            self.doff_plate(slot);
        }
    }

    /// The share of a blow the plate takes: what it holds, and it comes off when it holds no more.
    pub(crate) fn plate_takes(&mut self, slot: u8, d: i32) -> i32 {
        if d <= 0 || !self.in_plate(slot) {
            return 0;
        }
        let u = UnitRef::Hero(slot);
        let took = d.min(self.s.status(u, S::Plate));
        self.add_status(u, S::Plate, -took);
        *self.hero_mut(slot).flags.entry("plate_hit".into()).or_insert(0) += took;
        if self.s.status(u, S::Plate) <= 0 {
            self.doff_plate(slot);
        }
        took
    }

    pub(crate) fn reforge(&mut self, ctx: &Ctx, a: &Amount, from: Tgt, to: Tgt) {
        let UnitRef::Hero(me) = ctx.src else { return };
        let n = self.amount(ctx, a) + self.flag(me, "reforge_plus");
        if n <= 0 {
            return;
        }
        // re-forged strength is kept for the rest of the fight, on both sides
        for u in self.targets(ctx, from) {
            self.add_status(u, S::Strength, -n);
        }
        let gain = if self.flag(me, "reforge_all") > 0 { self.standing().into_iter().map(UnitRef::Hero).collect() } else { self.targets(ctx, to) };
        for u in gain {
            self.add_status(u, S::Strength, n);
        }
    }

    pub(crate) fn upgrade_hand(&mut self, ctx: &Ctx, n: i32, who: Tgt) {
        for u in self.targets(ctx, who) {
            let UnitRef::Hero(slot) = u else { continue };
            for _ in 0..n.max(0) {
                let h = &self.s.heroes[slot as usize];
                let can: Vec<usize> = (0..h.hand.len()).filter(|i| !h.hand[*i].upgraded && self.c.cards.get(&h.hand[*i].def).is_some_and(|d| d.up.is_some())).collect();
                let Some(&i) = self.s.rng.pick(&can) else { break };
                self.hero_mut(slot).hand[i].upgraded = true;
                if let UnitRef::Hero(me) = ctx.src {
                    self.fire(me, Trigger::Upgraded);
                }
            }
        }
    }

    /// A random card of this hero's own, to stand where a curse was.
    fn own_card(&mut self, slot: u8) -> Option<Id> {
        let hero = self.s.heroes[slot as usize].def.clone();
        let pool: Vec<Id> = self.c.cards.values().filter(|d| d.hero == hero && matches!(d.rarity, Rarity::Common | Rarity::Uncommon | Rarity::Rare) && !matches!(d.kind, CardType::Curse | CardType::Status)).map(|d| d.id.clone()).collect();
        self.s.rng.pick(&pool).cloned()
    }

    fn transmutable(&self, card: &Card) -> bool {
        !card.def.starts_with("own_")
            && self.c.cards.get(&card.def).is_some_and(|d| matches!(d.kind, CardType::Curse | CardType::Status) && !d.keywords.contains(&Keyword::Passable))
    }

    pub(crate) fn transmute(&mut self, ctx: &Ctx, all: bool, who: Tgt) {
        for u in self.targets(ctx, who) {
            let UnitRef::Hero(slot) = u else { continue };
            for pile in [Pile::Hand, Pile::Draw, Pile::Discard] {
                if !all && pile != Pile::Hand {
                    continue;
                }
                let len = self.pile(slot, pile).len();
                for i in 0..len {
                    let card = self.pile(slot, pile)[i].clone();
                    if self.transmutable(&card) {
                        if let Some(id) = self.own_card(slot) {
                            let c = &mut self.pile_mut(slot, pile)[i];
                            c.def = id.clone();
                            c.upgraded = false;
                            self.ev.push(Event::CardAdded { player: slot, card: id, pile });
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn pile(&self, slot: u8, pile: Pile) -> &Vec<Card> {
        let h = &self.s.heroes[slot as usize];
        match pile {
            Pile::Hand => &h.hand,
            Pile::Draw => &h.draw,
            Pile::Discard => &h.discard,
        }
    }

    pub(crate) fn pile_mut(&mut self, slot: u8, pile: Pile) -> &mut Vec<Card> {
        let h = &mut self.s.heroes[slot as usize];
        match pile {
            Pile::Hand => &mut h.hand,
            Pile::Draw => &mut h.draw,
            Pile::Discard => &mut h.discard,
        }
    }

    /// The cards a pick may be made from.
    pub fn pickable(c: &Content, s: &Combat, p: &Picking, _picker: u8) -> Vec<u32> {
        let Some(h) = s.hero(p.owner) else { return Vec::new() };
        let pile = match p.pile {
            Pile::Hand => &h.hand,
            Pile::Draw => &h.draw,
            Pile::Discard => &h.discard,
        };
        let from = if p.top > 0 && p.pile == Pile::Draw { pile.len().saturating_sub(p.top as usize) } else { 0 };
        pile[from..]
            .iter()
            .filter(|card| !card.alien)
            .filter(|card| {
                let Some(d) = c.cards.get(&card.def) else { return false };
                let bad = matches!(d.kind, CardType::Curse | CardType::Status);
                match p.then {
                    PickThen::Transmute => bad && !card.def.starts_with("own_") && !d.keywords.contains(&Keyword::Passable),
                    // nobody copies a curse on purpose, and one's own card in hand is not picked from it
                    PickThen::Alien => !bad,
                    _ => true,
                }
            })
            .map(|card| card.uid)
            .collect()
    }

    /// A card that asks the player to pick from a pile stops here until they have; `false` when
    /// there is nothing to pick and the card simply goes on.
    pub(crate) fn begin_pick(&mut self, ctx: &Ctx, pile: Pile, from: Tgt, then: PickThen, top: u8, rest: Vec<Effect>) -> bool {
        let UnitRef::Hero(me) = ctx.src else { return false };
        if self.s.foretelling {
            return false;
        }
        let owner = match self.targets(ctx, from).first() {
            Some(UnitRef::Hero(o)) => *o,
            _ => me,
        };
        let p = Picking { owner, pile, top, then, rest, target: ctx.target, x: ctx.x, attack: ctx.attack };
        if Self::pickable(self.c, self.s, &p, me).is_empty() {
            return false;
        }
        self.hero_mut(me).picking = Some(p);
        self.ev.push(Event::Pick { player: me, owner, pile });
        true
    }

    /// A pick whose pile ran dry meanwhile (an ally played or drew the cards it offered) is let
    /// go, and the card goes on without it; otherwise the picker could never end the turn.
    pub(crate) fn drop_stale_picks(&mut self) {
        for me in 0..self.s.heroes.len() as u8 {
            let Some(p) = self.s.heroes[me as usize].picking.clone() else { continue };
            if !Self::pickable(self.c, self.s, &p, me).is_empty() {
                continue;
            }
            self.hero_mut(me).picking = None;
            let mut ctx = Ctx { src: UnitRef::Hero(me), target: p.target.filter(|t| self.exists(*t)), x: p.x, consumed: 0, fatal: false, attack: p.attack, card: true, bled: false };
            self.run(&p.rest, &mut ctx);
        }
        // likewise a discard asked of a hand that someone has emptied meanwhile
        for me in 0..self.s.heroes.len() as u8 {
            if self.s.heroes[me as usize].choosing.is_none() || !self.s.heroes[me as usize].hand.is_empty() {
                continue;
            }
            let Some(ch) = self.hero_mut(me).choosing.take() else { continue };
            self.hero_mut(me).flags.insert("last_discarded".into(), ch.picked as i32);
            let mut ctx = Ctx { src: UnitRef::Hero(me), target: ch.target.filter(|t| self.exists(*t)), x: ch.x, consumed: 0, fatal: false, attack: ch.attack, card: true, bled: false };
            self.run(&ch.rest, &mut ctx);
        }
    }

    pub(crate) fn pick(&mut self, me: u8, uid: u32) -> Result<(), ActionError> {
        let Some(p) = self.s.heroes[me as usize].picking.clone() else { return Err(ActionError::NotChoosing) };
        if !Self::pickable(self.c, self.s, &p, me).contains(&uid) {
            return Err(ActionError::NoSuchCard);
        }
        self.hero_mut(me).picking = None;
        let pos = self.pile(p.owner, p.pile).iter().position(|c| c.uid == uid).ok_or(ActionError::NoSuchCard)?;
        let card = self.pile(p.owner, p.pile)[pos].clone();
        match p.then {
            PickThen::Alien => self.give_alien(me, &card.def, card.upgraded, None),
            PickThen::Take | PickThen::TakeMarked => {
                let mut card = self.pile_mut(p.owner, p.pile).remove(pos);
                card.forged = p.then == PickThen::TakeMarked;
                let def = card.def.clone();
                if self.s.heroes[me as usize].hand.len() < HAND_LIMIT {
                    self.hero_mut(me).hand.push(card);
                    self.ev.push(Event::CardAdded { player: me, card: def, pile: Pile::Hand });
                } else {
                    self.hero_mut(me).discard.push(card);
                }
                self.roll_marked(me);
            }
            PickThen::Burn => {
                let card = self.pile_mut(p.owner, p.pile).remove(pos);
                self.exhaust_card(p.owner, card);
            }
            PickThen::Transmute => {
                if let Some(id) = self.own_card(p.owner) {
                    let c = &mut self.pile_mut(p.owner, p.pile)[pos];
                    c.def = id.clone();
                    c.upgraded = false;
                    self.ev.push(Event::CardAdded { player: p.owner, card: id, pile: p.pile });
                }
            }
        }
        let mut ctx = Ctx { src: UnitRef::Hero(me), target: p.target.filter(|t| self.exists(*t)), x: p.x, consumed: 0, fatal: false, attack: p.attack, card: true, bled: false };
        self.run(&p.rest, &mut ctx);
        Ok(())
    }

    // ---- Lisa ----

    /// A copy of someone's card comes into the hand: it costs what the original costs, and is
    /// gone once played or when the turn ends.
    pub(crate) fn give_alien(&mut self, me: u8, def: &str, upgraded: bool, cost: Option<i32>) {
        if self.s.heroes[me as usize].hand.len() >= HAND_LIMIT {
            return;
        }
        self.s.next_uid += 1;
        let card = Card { uid: self.s.next_uid, def: def.to_string(), upgraded, alien: true, cost_set: cost, ..Default::default() };
        self.hero_mut(me).hand.push(card);
        self.ev.push(Event::CardAdded { player: me, card: def.to_string(), pile: Pile::Hand });
        self.roll_marked(me);
    }

    /// A random card of another hero's pool, for one who has no ally to copy from.
    fn pool_card(&mut self, me: u8) -> Option<Id> {
        let mine = self.s.heroes[me as usize].def.clone();
        let pool: Vec<Id> = self
            .c
            .cards
            .values()
            .filter(|d| !d.hero.is_empty() && d.hero != mine && matches!(d.rarity, Rarity::Starter | Rarity::Common | Rarity::Uncommon) && !matches!(d.kind, CardType::Curse | CardType::Status | CardType::Herb))
            .filter(|d| !d.keywords.contains(&Keyword::Coop) && !d.keywords.contains(&Keyword::Unplayable))
            .map(|d| d.id.clone())
            .collect();
        self.s.rng.pick(&pool).cloned()
    }

    pub(crate) fn alien_from(&mut self, ctx: &Ctx, pile: Pile, from: Tgt, count: i32) {
        let UnitRef::Hero(me) = ctx.src else { return };
        if self.s.foretelling {
            return;
        }
        // aimed to pick the next one by hand
        if self.hero_mut(me).flags.remove("aim_next").is_some() && self.begin_pick(ctx, pile, from, PickThen::Alien, 0, Vec::new()) {
            return;
        }
        for _ in 0..count {
            let owner = match self.targets(ctx, from).first() {
                Some(UnitRef::Hero(o)) => *o,
                _ => me,
            };
            let can: Vec<(Id, bool)> = self.pile(owner, pile).iter().filter(|c| !c.alien && self.c.cards.get(&c.def).is_some_and(|d| !matches!(d.kind, CardType::Curse | CardType::Status))).map(|c| (c.def.clone(), c.upgraded)).collect();
            match self.s.rng.pick(&can).cloned() {
                Some((def, up)) => self.give_alien(me, &def, up, None),
                None => {
                    if let Some(def) = self.pool_card(me) {
                        self.give_alien(me, &def, false, None);
                    }
                }
            }
        }
    }

    /// Lisa's own turn: a copy from a random ally's hand, or from the other heroes' cards when alone.
    pub(crate) fn turn_aliens(&mut self, me: u8) {
        let extra = self.flag(me, "alien_more") + self.hero_mut(me).flags.remove("alien_next").unwrap_or(0);
        let others: Vec<u8> = self.standing().into_iter().filter(|s| *s != me).collect();
        // the fox's nose: the copy is hers to choose
        if self.flag(me, "fox_nose") > 0 && !others.is_empty() {
            let owner = others[self.s.rng.below(others.len() as u32) as usize];
            let ctx = Ctx { src: UnitRef::Hero(me), target: Some(UnitRef::Hero(owner)), x: 0, consumed: 0, fatal: false, attack: false, card: false, bled: false };
            if self.begin_pick(&ctx, Pile::Hand, Tgt::Target, PickThen::Alien, 0, Vec::new()) {
                for _ in 0..extra {
                    self.one_alien(me, &others);
                }
                return;
            }
        }
        for _ in 0..1 + extra {
            self.one_alien(me, &others);
        }
    }

    fn one_alien(&mut self, me: u8, others: &[u8]) {
        let mut can: Vec<(Id, bool)> = Vec::new();
        for o in others {
            can.extend(self.s.heroes[*o as usize].hand.iter().filter(|c| !c.alien && self.c.cards.get(&c.def).is_some_and(|d| !matches!(d.kind, CardType::Curse | CardType::Status))).map(|c| (c.def.clone(), c.upgraded)));
        }
        match self.s.rng.pick(&can).cloned() {
            Some((def, up)) => self.give_alien(me, &def, up, None),
            None => {
                if let Some(def) = self.pool_card(me) {
                    self.give_alien(me, &def, false, None);
                }
            }
        }
    }

    /// A blow aimed at Lisa: does she slip it.
    pub(crate) fn dodges(&mut self, i: u8, src: Option<UnitRef>) -> bool {
        let dead = self.flag(i, "play_dead") > 0;
        let u = UnitRef::Hero(i);
        if !dead && self.s.status(u, S::Evade) <= 0 {
            return false;
        }
        if !dead {
            self.add_status(u, S::Evade, -1);
        }
        *self.hero_mut(i).flags.entry("dodged".into()).or_insert(0) += 1;
        self.hero_mut(i).flags.insert("dodged_last".into(), 1);
        if let Some(UnitRef::Enemy(uid)) = src {
            self.hero_mut(i).flags.insert("last_striker".into(), uid as i32);
        }
        self.ev.push(Event::Dodged { player: i });
        self.ev.push(Event::Missed { unit: u });
        self.fire(i, Trigger::Dodged);
        true
    }

    /// Half of the target's Strength (for the round) and half its block come to the source.
    pub(crate) fn borrow(&mut self, ctx: &Ctx, from: UnitRef) {
        let UnitRef::Hero(me) = ctx.src else { return };
        let full = self.flag(me, "mirror") > 0;
        let str = self.s.status(from, S::Strength) + self.s.status(from, S::TempStrength);
        let block = match from {
            UnitRef::Hero(i) => self.s.heroes[i as usize].block,
            UnitRef::Enemy(uid) => self.s.enemy(uid).map_or(0, |e| e.block),
        };
        let (str, block) = if full { (str, block) } else { ((str + 1) / 2, block / 2) };
        // a foe that never trained still has something to lend: at least BORROW_MIN Strength
        let str = str.max(BORROW_MIN);
        if str > 0 {
            self.add_status(ctx.src, S::TempStrength, str);
        }
        self.gain_block(ctx.src, block);
        if full {
            // what else it holds that helps it, lent for the round
            for st in [S::Thorns, S::Fortitude, S::Mist] {
                let n = self.s.status(from, st);
                if n > 0 {
                    self.add_status(ctx.src, st, n);
                    *self.hero_mut(me).flags.entry(format!("lent_{st:?}")).or_insert(0) += n;
                }
            }
        }
    }

    /// The lent statuses go back as the round closes.
    pub(crate) fn return_lent(&mut self, slot: u8) {
        for st in [S::Thorns, S::Fortitude, S::Mist] {
            if let Some(n) = self.hero_mut(slot).flags.remove(&format!("lent_{st:?}")) {
                let has = self.s.status(UnitRef::Hero(slot), st);
                self.add_status(UnitRef::Hero(slot), st, -n.min(has));
            }
        }
    }

    // ---- Ivanushka ----

    /// Throws the numbers of every marked card of chance in the hand that has none yet.
    pub(crate) fn roll_marked(&mut self, slot: u8) {
        for i in 0..self.s.heroes[slot as usize].hand.len() {
            let card = self.s.heroes[slot as usize].hand[i].clone();
            if card.blind.is_some() || !card.rolls.is_empty() {
                continue;
            }
            let Some(def) = self.c.cards.get(&card.def) else { continue };
            if !def.keywords.contains(&Keyword::Marked) {
                continue;
            }
            let view = def.view(card.upgraded, self.s.heroes[slot as usize].form);
            let rolls = self.throw_all(slot, &ranges(&view.effects));
            self.hero_mut(slot).hand[i].rolls = rolls;
        }
    }

    pub(crate) fn throw_all(&mut self, slot: u8, ranges: &[(i32, i32)]) -> Vec<i32> {
        ranges.iter().map(|(lo, hi)| self.throw(slot, *lo, *hi)).collect()
    }

    /// One number between the two; at either end the hero's powers hear of it.
    pub(crate) fn throw(&mut self, slot: u8, lo: i32, hi: i32) -> i32 {
        if self.s.foretelling {
            return lo;
        }
        let (lo, hi) = (lo.min(hi), hi.max(lo));
        let v = self.s.rng.range(lo, hi);
        self.hero_mut(slot).flags.insert("last_roll".into(), v);
        if lo < hi {
            if v == lo {
                self.fire(slot, Trigger::RollMin);
            } else if v == hi {
                self.fire(slot, Trigger::RollMax);
            }
        }
        v
    }

    pub(crate) fn luck(&mut self, ctx: &mut Ctx, foe: &[Effect], friend: &[Effect], keep: bool) {
        let UnitRef::Hero(me) = ctx.src else { return };
        if self.s.foretelling {
            return;
        }
        let evil = friend.is_empty();
        let times = if self.flag(me, "fool_fool") > 0 { 2 } else { 1 };
        for _ in 0..times {
            let mut pool: Vec<UnitRef> = self.living().into_iter().map(UnitRef::Enemy).collect();
            pool.extend(self.standing().into_iter().map(UnitRef::Hero));
            if self.flag(me, "own_mind") > 0 {
                pool.retain(|u| *u != UnitRef::Hero(me));
            }
            let Some(mut aim) = self.s.rng.pick(&pool).copied() else { break };
            // an eye on it: of two it falls on, the foe
            if self.flag(me, "eye_eye") > 0 {
                if let Some(other) = self.s.rng.pick(&pool).copied() {
                    if matches!(other, UnitRef::Enemy(_)) {
                        aim = other;
                    }
                }
            }
            let was = ctx.target;
            ctx.target = Some(aim);
            match aim {
                UnitRef::Enemy(_) => self.run(foe, ctx),
                UnitRef::Hero(h) => {
                    self.run(if evil { foe } else { friend }, ctx);
                    let _ = h;
                    self.fire(me, Trigger::LuckHitAlly);
                }
            }
            ctx.target = was;
        }
        *self.hero_mut(me).flags.entry("luck_cards".into()).or_insert(0) += 1;
        let spared = keep || self.hero_mut(me).flags.remove("lucky_next").is_some() || self.flag(me, "lucky_one") > 0;
        if !spared {
            let l = self.s.status(ctx.src, S::Luck);
            self.add_status(ctx.src, S::Luck, -l);
        }
    }

    /// Ivanushka's hooks, Kuzma's and Lisa's; `false` for a name none of them knows.
    pub(crate) fn hook3(&mut self, ctx: &mut Ctx, name: &str) -> bool {
        let me = match ctx.src {
            UnitRef::Hero(i) => Some(i),
            _ => None,
        };
        match name {
            "don_plate" => {
                if let Some(me) = me {
                    self.don_plate(me);
                }
            }
            "doff_plate" => {
                if let Some(me) = me {
                    self.doff_plate(me);
                }
            }
            // the plate goes back into the forge, and forging starts from a quarter of it
            "remelt" => {
                if let Some(me) = me {
                    let left = self.s.status(UnitRef::Hero(me), S::Plate);
                    self.doff_plate(me);
                    self.forge(me, left / 4);
                }
            }
            // every helpful status the foe holds is struck off it
            "unforge" => {
                if let Some(t) = ctx.target {
                    let all: Vec<(StatusKind, i32)> = match t {
                        UnitRef::Enemy(uid) => self.s.enemy(uid).map(|e| e.statuses.iter().map(|(k, v)| (*k, *v)).collect()).unwrap_or_default(),
                        UnitRef::Hero(i) => self.s.heroes[i as usize].statuses.iter().map(|(k, v)| (*k, *v)).collect(),
                    };
                    for (st, n) in all {
                        if !st.debuff() && n > 0 {
                            self.add_status(t, st, -n);
                        }
                    }
                }
            }
            // one debuff off yourself
            "cleanse_one" => {
                let all: Vec<StatusKind> = match ctx.src {
                    UnitRef::Hero(i) => self.s.heroes[i as usize].statuses.keys().copied().filter(|s| s.debuff()).collect(),
                    UnitRef::Enemy(_) => Vec::new(),
                };
                if let Some(st) = self.s.rng.pick(&all).copied() {
                    let n = self.s.status(ctx.src, st);
                    self.add_status(ctx.src, st, -n);
                }
            }
            // the foe loses all its block
            "strip_target_block" => {
                if let Some(UnitRef::Enemy(uid)) = ctx.target {
                    if let Some(idx) = self.enemy_idx(uid) {
                        self.s.enemies[idx].block = 0;
                    }
                }
            }
            "reroll" => {
                if let Some(me) = me {
                    for i in 0..self.s.heroes[me as usize].hand.len() {
                        if self.s.heroes[me as usize].hand[i].blind.is_none() {
                            self.hero_mut(me).hand[i].rolls.clear();
                        }
                    }
                    self.roll_marked(me);
                }
            }
            "all_blind" => {
                if let Some(me) = me {
                    for i in 0..self.s.heroes[me as usize].hand.len() {
                        let card = self.s.heroes[me as usize].hand[i].clone();
                        let ranged = self.c.cards.get(&card.def).is_some_and(|d| ranged(&d.view(card.upgraded, Form::Human)));
                        if ranged {
                            let c = &mut self.hero_mut(me).hand[i];
                            c.blind = Some(c.blind.unwrap_or(0) + 2);
                            c.rolls.clear();
                        }
                    }
                }
            }
            "cat_in_bag" => {
                if let Some(me) = me {
                    let hero = self.s.heroes[me as usize].def.clone();
                    let pool: Vec<Id> = self.c.cards.values().filter(|d| d.hero == hero && d.rarity == Rarity::Rare).map(|d| d.id.clone()).collect();
                    if let Some(id) = self.s.rng.pick(&pool).cloned() {
                        let cost = self.s.rng.range(0, 2);
                        self.add_card(me, &id, Pile::Hand);
                        if let Some(c) = self.hero_mut(me).hand.last_mut() {
                            if c.def == id {
                                c.cost_set = Some(cost);
                            }
                        }
                        self.roll_marked(me);
                    }
                }
            }
            "reshuffle" => {
                if let Some(me) = me {
                    let mut pile = std::mem::take(&mut self.hero_mut(me).discard);
                    pile.append(&mut self.hero_mut(me).draw);
                    self.s.rng.shuffle(&mut pile);
                    self.hero_mut(me).draw = pile;
                    self.ev.push(Event::Shuffled { player: me });
                }
            }
            "humpback" => {
                if let Some(me) = me {
                    let luck = self.s.status(UnitRef::Hero(me), S::Luck);
                    match self.s.rng.below(3) {
                        0 => {
                            let foes = self.living();
                            if let Some(t) = self.s.rng.pick(&foes).copied() {
                                self.hurt(Some(ctx.src), UnitRef::Enemy(t), luck, false);
                            }
                        }
                        1 => self.gain_block(ctx.src, luck),
                        _ => self.add_status(ctx.src, S::Luck, 2),
                    }
                }
            }
            "halve_luck" => {
                let l = self.s.status(ctx.src, S::Luck);
                self.add_status(ctx.src, S::Luck, -(l - l / 2));
            }
            // the prices in the hand are shuffled among the cards, none above 3
            "shuffle_costs" => {
                if let Some(me) = me {
                    let form = self.s.heroes[me as usize].form;
                    let mut costs: Vec<i32> = self.s.heroes[me as usize].hand.iter().map(|c| c.cost_set.unwrap_or_else(|| self.c.cards.get(&c.def).map_or(0, |d| d.view(c.upgraded, form).cost)).clamp(0, 3)).collect();
                    self.s.rng.shuffle(&mut costs);
                    for (c, cost) in self.hero_mut(me).hand.iter_mut().zip(costs) {
                        c.cost_set = Some(cost);
                    }
                }
            }
            "borrow" => {
                if let Some(t) = ctx.target {
                    let c = ctx.clone();
                    self.borrow(&c, t);
                }
            }
            "borrow_block_all" => {
                let c = ctx.clone();
                for uid in self.living() {
                    let b = self.s.enemy(uid).map_or(0, |e| e.block) / 2;
                    if self.flag_of(c.src, "mirror") > 0 {
                        self.borrow(&c, UnitRef::Enemy(uid));
                    } else {
                        self.gain_block(c.src, b);
                    }
                }
            }
            // half the blow the crane means to deal comes back at it
            "crane" => {
                if let Some(UnitRef::Enemy(uid)) = ctx.target {
                    let dmg = self.s.enemy(uid).and_then(|e| crate::bot::intent_damage(self.c, self.s, e)).map_or(0, |(d, k)| d * k);
                    self.strike(Some(ctx.src), UnitRef::Enemy(uid), dmg / 2, true);
                }
            }
            // the wolf: the foe strikes another foe at once, with all its intent
            "fox_wolf" => {
                if let Some(UnitRef::Enemy(uid)) = ctx.target {
                    let dmg = self.s.enemy(uid).and_then(|e| crate::bot::intent_damage(self.c, self.s, e)).map_or(0, |(d, k)| d * k);
                    let others: Vec<u32> = self.living().into_iter().filter(|u| *u != uid).collect();
                    if let Some(o) = self.s.rng.pick(&others).copied() {
                        self.hurt(Some(UnitRef::Enemy(uid)), UnitRef::Enemy(o), dmg, false);
                    }
                }
            }
            // the foe is finished at once
            "execute" => {
                if let Some(t @ UnitRef::Enemy(uid)) = ctx.target {
                    let hp = self.s.enemy(uid).map_or(0, |e| e.hp);
                    self.lose_hp(t, hp);
                    ctx.fatal = true;
                }
            }
            // a copy of the foe's move at half its strength
            "steal_move" => {
                if let (Some(me), Some(UnitRef::Enemy(uid))) = (me, ctx.target) {
                    if let Some(mv) = self.s.enemy(uid).and_then(|e| e.intent.as_ref()).map(|i| i.mv) {
    
                        if self.s.heroes[me as usize].hand.len() < HAND_LIMIT {
                            self.s.next_uid += 1;
                            let card = Card { uid: self.s.next_uid, def: STOLEN_MOVE.into(), alien: true, rolls: vec![uid as i32, mv as i32], ..Default::default() };
                            self.hero_mut(me).hand.push(card);
                            self.ev.push(Event::CardAdded { player: me, card: STOLEN_MOVE.into(), pile: Pile::Hand });
                        }
                    }
                }
            }
            "play_move" => {
                if let Some(me) = me {
                    let foe = self.flag(me, "this_move_foe") as u32;
                    let mv = self.flag(me, "this_move_idx") as usize;
                    let def = self.s.enemy(foe).map(|e| e.def.clone());
                    let effects = def.and_then(|d| self.c.enemies.get(&d)).and_then(|d| d.moves.get(mv)).map(|m| half_move(&m.effects)).unwrap_or_default();
                    let mut c = ctx.clone();
                    c.attack = true;
                    self.run(&effects, &mut c);
                }
            }
            // four to the foe that struck at her and missed
            "hit_striker" => {
                if let Some(me) = me {
                    let uid = self.flag(me, "last_striker") as u32;
                    if self.s.enemy(uid).is_some_and(|e| e.alive) {
                        self.hurt(Some(ctx.src), UnitRef::Enemy(uid), 4, false);
                    }
                }
            }
            "charm_random" => {
                let foes = self.living();
                if let Some(t) = self.s.rng.pick(&foes).copied() {
                    self.add_status(UnitRef::Enemy(t), S::Charm, 1);
                }
            }
            // the alien card in the hand is swapped for a chosen copy from another's hand
            "drop_alien" => {
                if let Some(me) = me {
                    if let Some(pos) = self.s.heroes[me as usize].hand.iter().position(|c| c.alien) {
                        self.hero_mut(me).hand.remove(pos);
                    }
                }
            }
            "discard_alien" => {
                if let Some(me) = me {
                    if let Some(pos) = self.s.heroes[me as usize].hand.iter().position(|c| c.alien) {
                        let card = self.hero_mut(me).hand.remove(pos);
                        self.ev.push(Event::Discarded { player: me, card: card.def });
                        *self.hero_mut(me).flags.entry("turn_discarded".into()).or_insert(0) += 1;
                        self.fire(me, Trigger::Discarded);
                    }
                }
            }
            // the ally draws, and a copy of what came comes to Lisa
            "gossip" | "promise" => {
                if let Some(me) = me {
                    let n = if name == "promise" { 2 } else { 1 };
                    let ally = match ctx.target {
                        Some(UnitRef::Hero(a)) => a,
                        _ => me,
                    };
                    let before = self.s.heroes[ally as usize].hand.len();
                    self.draw(ally, n);
                    let drawn: Vec<(Id, bool)> = self.s.heroes[ally as usize].hand[before..].iter().map(|c| (c.def.clone(), c.upgraded)).collect();
                    if let Some((def, up)) = self.s.rng.pick(&drawn).cloned() {
                        self.give_alien(me, &def, up, None);
                    }
                    self.roll_marked(ally);
                }
            }
            // a copy of the top of the ally's draw pile, a little cheaper
            "swipe_top" => {
                if let Some(me) = me {
                    let ally = match ctx.target {
                        Some(UnitRef::Hero(a)) => a,
                        _ => me,
                    };
                    if let Some(top) = self.s.heroes[ally as usize].draw.last().cloned() {
                        let cost = self.c.cards.get(&top.def).map_or(0, |d| d.view(top.upgraded, Form::Human).cost - 1).max(0);
                        self.give_alien(me, &top.def, top.upgraded, Some(cost));
                    }
                }
            }
            // copies of the whole of an ally's hand
            "copy_hand" => {
                if let Some(me) = me {
                    let ally = match ctx.target {
                        Some(UnitRef::Hero(a)) => a,
                        _ => me,
                    };
                    let all: Vec<(Id, bool)> = self.s.heroes[ally as usize].hand.iter().filter(|c| !c.alien).map(|c| (c.def.clone(), c.upgraded)).collect();
                    for (def, up) in all {
                        self.give_alien(me, &def, up, None);
                    }
                }
            }
            // one copy from each of two different allies' hands
            "gifts" => {
                if let Some(me) = me {
                    let mut others: Vec<u8> = self.standing().into_iter().filter(|s| *s != me).collect();
                    self.s.rng.shuffle(&mut others);
                    if others.is_empty() {
                        for _ in 0..2 {
                            if let Some(def) = self.pool_card(me) {
                                self.give_alien(me, &def, false, None);
                            }
                        }
                    }
                    for o in others.into_iter().take(2) {
                        let can: Vec<(Id, bool)> = self.s.heroes[o as usize].hand.iter().filter(|c| !c.alien).map(|c| (c.def.clone(), c.upgraded)).collect();
                        if let Some((def, up)) = self.s.rng.pick(&can).cloned() {
                            self.give_alien(me, &def, up, None);
                        }
                    }
                }
            }
            // the workshop: a random ally gets bracers and a little block
            "workshop" => {
                let c = ctx.clone();
                let pick = self.targets(&c, Tgt::RandomAlly);
                if let Some(UnitRef::Hero(a)) = pick.first().copied() {
                    *self.hero_mut(a).flags.entry("bracers".into()).or_insert(0) += 1;
                    self.gain_block(UnitRef::Hero(a), 3);
                }
            }
            _ => return false,
        }
        true
    }

    fn flag_of(&self, u: UnitRef, key: &str) -> i32 {
        match u {
            UnitRef::Hero(i) => self.flag(i, key),
            UnitRef::Enemy(_) => 0,
        }
    }
}

/// A foe's move as Lisa plays it back: its blows and shields at half, aimed the other way.
fn half_move(effects: &[Effect]) -> Vec<Effect> {
    let half = |a: &Amount| match a {
        Amount::N(n) => Amount::N((*n / 2).max(1)),
        _ => Amount::N(3),
    };
    let mut out = Vec::new();
    for e in effects {
        match e {
            Effect::Damage(a, t) => out.push(Effect::Damage(half(a), if *t == Tgt::AllFoes { Tgt::AllFoes } else { Tgt::Target })),
            Effect::Hits(a, k, t) => out.push(Effect::Hits(half(a), *k, if *t == Tgt::AllFoes { Tgt::AllFoes } else { Tgt::Target })),
            Effect::Block(a, _) => out.push(Effect::Block(half(a), Tgt::Me)),
            Effect::Heal(a, _) => out.push(Effect::Heal(half(a), Tgt::Me)),
            Effect::Apply(st, a, t) if st.debuff() => out.push(Effect::Apply(*st, half(a), if *t == Tgt::AllFoes { Tgt::AllFoes } else { Tgt::Target })),
            Effect::Apply(st @ (StatusKind::Strength | StatusKind::Thorns), a, _) => out.push(Effect::Apply(if *st == StatusKind::Strength { StatusKind::TempStrength } else { *st }, half(a), Tgt::Me)),
            Effect::If(_, then, _) => out.extend(half_move(then)),
            Effect::Repeat(_, body) => out.extend(half_move(body)),
            _ => {}
        }
    }
    out
}
