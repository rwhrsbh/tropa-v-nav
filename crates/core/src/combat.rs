//! Combat rules: the only code allowed to mutate a `Combat`.

use crate::data::*;
use crate::rng::Rng;
use crate::state::*;
use std::collections::BTreeMap;
use StatusKind as S;

/// Where an effect list is running from.
#[derive(Clone, Debug)]
pub(crate) struct Ctx {
    pub src: UnitRef,
    pub target: Option<UnitRef>,
    pub x: i32,
    pub consumed: i32,
    pub fatal: bool,
    /// Damage counts as an attack: Strength, Weak, Hex and friends apply.
    pub attack: bool,
    /// Comes from a card the source played (Fortitude, Beast block ban).
    pub card: bool,
    pub bled: bool,
}

impl Ctx {
    fn passive(src: UnitRef) -> Ctx {
        Ctx { src, target: None, x: 0, consumed: 0, fatal: false, attack: false, card: false, bled: false }
    }
}

/// A blow taking this share (percent) of the health a hero had before it may take a limb too.
const MAIM_FROM: i32 = 30;
const MOST_LIMBS: u8 = 2;
/// A killing blow with this much (percent of whole health) still to spend takes the head.
const BEHEAD_PAST: i32 = 40;

/// Rounds a fight may run before the dark gathers on the heroes (see `StatusKind::Gloom`); an
/// elite and a boss take longer to bring down, so they are given more.
pub const STALL_ROUND: u32 = 8;
pub const STALL_ELITE: u32 = 12;
pub const STALL_BOSS: u32 = 16;
/// Percent of block and healing each stack of the dark takes away, and the most it can.
const GLOOM_PCT: i32 = 10;
const GLOOM_MAX: i32 = 60;

/// Whether a card held by `slot` would meet the condition of a bonus it carries, played now: what
/// the hand lights up. A condition on the target counts when some unit it may land on meets it; one
/// that only the playing can tell (a killing blow) never lights.
pub fn bonus_ready(c: &Content, s: &Combat, slot: u8, effects: &[Effect]) -> bool {
    fn conds(list: &[Effect], out: &mut Vec<Cond>) {
        for e in list {
            match e {
                Effect::If(c, then, other) => {
                    out.push(c.clone());
                    conds(then, out);
                    conds(other, out);
                }
                Effect::Repeat(_, body) | Effect::Once(_, body) | Effect::OncePerTurn(_, body) => conds(body, out),
                _ => {}
            }
        }
    }
    fn knowable(c: &Cond) -> bool {
        match c {
            Cond::Fatal => false,
            Cond::Not(inner) => knowable(inner),
            _ => true,
        }
    }
    let mut found = Vec::new();
    conds(effects, &mut found);
    found.retain(knowable);
    if found.is_empty() {
        return false;
    }
    let mut copy = s.clone();
    copy.foretelling = true;
    let mut targets: Vec<Option<UnitRef>> = vec![None];
    targets.extend(copy.living_enemies().map(|e| Some(UnitRef::Enemy(e.uid))));
    targets.extend(copy.standing_heroes().map(|h| Some(UnitRef::Hero(h.slot))));
    let mut ex = Exec { s: &mut copy, c, ev: Vec::new(), depth: 0 };
    found.iter().any(|cond| {
        targets.iter().any(|t| {
            let ctx = Ctx { src: UnitRef::Hero(slot), target: *t, x: 0, consumed: 0, fatal: false, attack: false, card: true, bled: false };
            ex.cond(&ctx, cond)
        })
    })
}

/// One blow from no one in particular, for tests.
#[doc(hidden)]
pub fn test_hit_hero(c: &Content, s: &mut Combat, slot: u8, n: i32) -> Vec<Event> {
    let mut ex = Exec { s, c, ev: Vec::new(), depth: 0 };
    let foe = ex.s.enemies.first().map(|e| UnitRef::Enemy(e.uid));
    ex.hero_take(foe, slot, n, true);
    ex.ev
}

/// The most one blow can do to a foe of straw.
const HOLLOW_MOST: i32 = 6;

pub struct Exec<'a> {
    pub s: &'a mut Combat,
    pub c: &'a Content,
    pub ev: Vec<Event>,
    depth: u8,
}

/// Starts a fight. `enemies` are definition ids, left to right.
pub fn start(c: &Content, heroes: &[HeroSetup], enemies: &[Id], seed: u64) -> (Combat, Vec<Event>) {
    start_with(c, heroes, enemies, seed, CombatOpts::default())
}

/// The arm that strikes, and the arm that shields.
pub const WEAPON_ARM: u8 = 1;
pub const SHIELD_ARM: u8 = 2;

/// Fear is kept in tenths of a percent, so the Mara mask can add half a percent.
pub const FEAR_CAP: i32 = 750;
/// What each blow struck in the Mara mask adds: 7.5 percent.
pub const MARA_FEAR: i32 = 75;

/// What a body has left to fight with once this arm is gone.
pub fn stump_cards(arm: u8) -> &'static [&'static str] {
    if arm == WEAPON_ARM {
        &["stump_bash", "stump_kick"]
    } else {
        &["stump_shoulder", "stump_spray"]
    }
}

/// And once neither arm is left.
pub const BOTH_ARMS_CARDS: [&str; 2] = ["stump_bite", "stump_spite"];

/// Does any of these effects, however deep in a condition or a repeat, answer to this test.
fn any_effect(effects: &[Effect], test: &dyn Fn(&Effect) -> bool) -> bool {
    effects.iter().any(|e| {
        test(e)
            || match e {
                Effect::If(_, then, other) => any_effect(then, test) || any_effect(other, test),
                Effect::Repeat(_, inner) | Effect::Once(_, inner) | Effect::OncePerTurn(_, inner) => any_effect(inner, test),
                _ => false,
            }
    })
}

/// Does this card take an arm that is gone. Every hero strikes with the weapon arm and shields with
/// the other, and besides each arm carries what that hero holds in it: the herb woman's basket and
/// jug, the volhv's staff, the witch's doll and bones, the jester's free hand for the masks.
/// In beast form a werewolf bites: no arm is needed for an attack.
pub fn needs_lost_arm(view: &CardView, lost_arms: u8, hero: &str, form: Form) -> bool {
    if view.has(Keyword::Armless) {
        return false;
    }
    let weapon_gone = lost_arms & WEAPON_ARM != 0;
    let shield_gone = lost_arms & SHIELD_ARM != 0;
    let any = |test: &dyn Fn(&Effect) -> bool| any_effect(&view.effects, test);
    if weapon_gone && view.kind == CardType::Attack && form != Form::Beast {
        return true;
    }
    if shield_gone && any(&|e| matches!(e, Effect::Block(..) | Effect::Guard(..))) {
        return true;
    }
    match hero {
        // the basket holds the herbs, the jug pours what is brewed
        "znaharka" => (shield_gone && any(&|e| matches!(e, Effect::Herb(_)))) || (weapon_gone && any(&|e| matches!(e, Effect::Brew))),
        // runes are cut by hand, and called down through the staff
        "volhv" => (shield_gone && any(&|e| matches!(e, Effect::Rune(..)))) || (weapon_gone && any(&|e| matches!(e, Effect::Invoke))),
        // the doll takes the curse, the bones read it and cash it in
        "vorozheya" => (shield_gone && any(&|e| matches!(e, Effect::Apply(StatusKind::Hex, ..)))) || (weapon_gone && any(&|e| matches!(e, Effect::Consume(StatusKind::Hex, _) | Effect::ConsumeCurses(_)))),
        // a mask is changed with the free hand
        "skomoroh" => shield_gone && any(&|e| matches!(e, Effect::Mask(_))),
        _ => false,
    }
}

pub fn start_with(c: &Content, heroes: &[HeroSetup], enemies: &[Id], seed: u64, opts: CombatOpts) -> (Combat, Vec<Event>) {
    let mut rng = Rng::new(seed);
    let mut next_uid = 1u32;
    let mut hs = Vec::new();
    for (slot, h) in heroes.iter().enumerate() {
        let mut draw: Vec<Card> = h
            .deck
            .iter()
            .map(|(def, upgraded)| {
                next_uid += 1;
                Card { uid: next_uid, def: def.clone(), upgraded: *upgraded, free: false }
            })
            .collect();
        rng.shuffle(&mut draw);
        hs.push(Hero {
            slot: slot as u8,
            def: h.def.clone(),
            mechanic: c.heroes.get(&h.def).map(|d| d.mechanic).unwrap_or(Mechanic::Valor),
            hp: h.hp,
            max_hp: h.max_hp,
            block: 0,
            energy: 0,
            statuses: BTreeMap::new(),
            draw,
            hand: Vec::new(),
            discard: Vec::new(),
            exhaust: Vec::new(),
            relics: h.relics.clone(),
            powers: Vec::new(),
            ready: false,
            downed: h.hp <= 0 || h.beheaded,
            falls: h.falls,
            lost_limbs: h.lost_limbs,
            lost_arms: h.lost_arms,
            beheaded: h.beheaded,
            walks_headless: h.walks_headless,
            new_curses: 0,
            new_parts: Vec::new(),
            guard_by: None,
            valor_rem: 0,
            runes: Vec::new(),
            form: Form::Human,
            moon: 0,
            mask: None,
            cauldron: Vec::new(),
            potions: h.potions.clone(),
            gold_lost: 0,
            gold: 0,
            flags: BTreeMap::new(),
            choosing: None,
        });
    }
    let mut s = Combat {
        round: 0,
        phase: Phase::Heroes,
        hp_scale: hp_scale(hs.len()) * opts.enemy_hp / 100,
        enemy_dmg: opts.enemy_dmg,
        shadow: opts.shadow,
        stall: opts.stall,
        heroes: hs,
        enemies: Vec::new(),
        rng,
        next_uid,
        log: Vec::new(),
        raise_cost: opts.raise_cost,
        foretelling: false,
    };
    let mut ex = Exec { s: &mut s, c, ev: Vec::new(), depth: 0 };
    for id in enemies {
        ex.spawn(id);
    }
    for slot in 0..ex.s.heroes.len() as u8 {
        ex.fire(slot, Trigger::CombatStart);
    }
    ex.start_round();
    let ev = ex.ev;
    (s, ev)
}

/// Applies one player action. On `Err` the state is untouched.
pub fn apply(c: &Content, s: &mut Combat, player: u8, action: &CombatAction) -> Result<Vec<Event>, ActionError> {
    if s.phase != Phase::Heroes {
        return Err(ActionError::NotHeroPhase);
    }
    let h = s.hero(player).ok_or(ActionError::NoSuchPlayer)?;
    if h.downed {
        return Err(ActionError::Downed);
    }
    let choosing = h.choosing.is_some();
    if choosing != matches!(action, CombatAction::Discard { .. }) {
        return Err(if choosing { ActionError::Choosing } else { ActionError::NotChoosing });
    }
    let mut ex = Exec { s, c, ev: Vec::new(), depth: 0 };
    match action {
        CombatAction::Discard { card } => ex.pick_discard(player, *card)?,
        CombatAction::Play { card, target } => ex.play(player, *card, *target)?,
        CombatAction::Ready(r) => {
            ex.s.heroes[player as usize].ready = *r;
            ex.ev.push(Event::Ready { player, ready: *r });
        }
        CombatAction::Raise { ally, discard } => ex.raise(player, *ally, *discard)?,
        CombatAction::Pass { card, to } => ex.pass(player, *card, *to)?,
        CombatAction::UsePotion { slot, target } => ex.use_potion(player, *slot, *target)?,
    }
    ex.check_end();
    ex.maybe_close();
    Ok(ex.ev)
}

/// Pure form of [`apply`] for callers that want `reduce(state, action) -> state`.
pub fn reduce(c: &Content, s: &Combat, player: u8, action: &CombatAction) -> Result<(Combat, Vec<Event>), ActionError> {
    let mut next = s.clone();
    let ev = apply(c, &mut next, player, action)?;
    Ok((next, ev))
}

impl<'a> Exec<'a> {
    fn hero(&mut self, slot: u8) -> &mut Hero {
        &mut self.s.heroes[slot as usize]
    }
    fn enemy_idx(&self, uid: u32) -> Option<usize> {
        self.s.enemies.iter().position(|e| e.uid == uid)
    }
    pub(crate) fn standing(&self) -> Vec<u8> {
        self.s.standing_heroes().map(|h| h.slot).collect()
    }
    pub(crate) fn living(&self) -> Vec<u32> {
        self.s.living_enemies().map(|e| e.uid).collect()
    }
    fn exists(&self, u: UnitRef) -> bool {
        match u {
            UnitRef::Hero(i) => self.s.hero(i).is_some_and(|h| !h.downed),
            UnitRef::Enemy(i) => self.s.enemy(i).is_some_and(|e| e.alive && !e.flags.contains_key(crate::state::BONES)),
        }
    }

    pub(crate) fn spawn(&mut self, def: &Id) -> Option<u32> {
        let d = self.c.enemies.get(def)?;
        self.s.next_uid += 1;
        let uid = self.s.next_uid;
        let hp = (self.s.rng.range(d.hp.0, d.hp.1) * self.s.hp_scale + 500) / 1000;
        self.s.enemies.push(Enemy {
            uid,
            def: def.clone(),
            hp,
            max_hp: hp,
            block: d.start_block,
            statuses: BTreeMap::new(),
            alive: true,
            fled: false,
            step: 0,
            intent: None,
            stolen: BTreeMap::new(),
            flags: d.flags.iter().map(|f| (f.clone(), 1)).collect(),
        });
        Some(uid)
    }

    // ---- statuses, block, healing ----

    pub(crate) fn add_status(&mut self, u: UnitRef, st: StatusKind, n: i32) {
        if n == 0 || (n > 0 && !self.exists(u)) {
            return;
        }
        let map = match u {
            UnitRef::Hero(i) => &mut self.s.heroes[i as usize].statuses,
            UnitRef::Enemy(i) => match self.s.enemies.iter_mut().find(|e| e.uid == i) {
                Some(e) => &mut e.statuses,
                None => return,
            },
        };
        let cur = map.get(&st).copied().unwrap_or(0);
        let mut total = cur + n;
        // Strength is the only status that can go below zero (it can be drained)
        if st != S::Strength {
            total = total.max(0);
        }
        if st == S::Fear {
            total = total.min(FEAR_CAP);
        }
        if total == 0 {
            map.remove(&st);
        } else {
            map.insert(st, total);
        }
        if total != cur {
            self.ev.push(Event::Status { unit: u, status: st, delta: total - cur, total });
        }
    }

    fn clear_status(&mut self, u: UnitRef, st: StatusKind) -> i32 {
        let cur = self.s.status(u, st);
        self.add_status(u, st, -cur);
        cur
    }

    pub(crate) fn gain_block(&mut self, u: UnitRef, n: i32) {
        let n = self.gloomed(u, n);
        if n <= 0 || !self.exists(u) {
            return;
        }
        let total = match u {
            UnitRef::Hero(i) => {
                self.hero(i).block += n;
                self.hero(i).block
            }
            UnitRef::Enemy(i) => {
                let idx = self.enemy_idx(i).unwrap();
                self.s.enemies[idx].block += n;
                self.s.enemies[idx].block
            }
        };
        self.ev.push(Event::Block { unit: u, amount: n, total });
    }

    /// What the gathering dark leaves of block or healing a hero is given: a tenth less for each
    /// of its stacks, at most three fifths less.
    fn gloomed(&self, u: UnitRef, n: i32) -> i32 {
        let g = self.s.status(u, S::Gloom);
        if g <= 0 || n <= 0 {
            return n;
        }
        n * (100 - (g * GLOOM_PCT).min(GLOOM_MAX)) / 100
    }

    pub(crate) fn heal(&mut self, u: UnitRef, n: i32) {
        let n = self.gloomed(u, n);
        if n <= 0 || !self.exists(u) {
            return;
        }
        let (healed, hp) = match u {
            UnitRef::Hero(i) => {
                let h = self.hero(i);
                let v = n.min(h.max_hp - h.hp);
                h.hp += v;
                (v, h.hp)
            }
            UnitRef::Enemy(i) => {
                let idx = self.enemy_idx(i).unwrap();
                let e = &mut self.s.enemies[idx];
                let v = n.min(e.max_hp - e.hp);
                e.hp += v;
                (v, e.hp)
            }
        };
        if healed > 0 {
            self.ev.push(Event::Heal { unit: u, amount: healed, hp });
        }
    }

    // ---- damage ----

    /// Applies attack modifiers, then lands the hit. Returns true if the target died.
    pub(crate) fn strike(&mut self, src: Option<UnitRef>, dst: UnitRef, base: i32, attack: bool) -> bool {
        if !self.exists(dst) {
            return false;
        }
        let mut d = base;
        if attack {
            let mut beast = false;
            let mut mara = false;
            if let Some(s) = src {
                d += self.s.status(s, S::Strength) + self.s.status(s, S::TempStrength);
                if let UnitRef::Hero(i) = s {
                    let h = &self.s.heroes[i as usize];
                    beast = h.form == Form::Beast;
                    mara = h.mask == Some(MaskKind::Mara);
                    if h.mask == Some(MaskKind::Bear) {
                        d += 3;
                    }
                }
            }
            if self.s.status(dst, S::Hex) > 0 {
                d += 3;
                self.add_status(dst, S::Hex, -1);
            }
            d += self.s.status(dst, S::HuntMark);
            d = d.max(0);
            if beast {
                d = d * 3 / 2;
            }
            if matches!(src, Some(UnitRef::Enemy(_))) {
                d = d * self.s.enemy_dmg / 100;
            }
            if src.is_some_and(|s| self.s.status(s, S::Weak) > 0) {
                d = d * 3 / 4;
            }
            if self.s.status(dst, S::Vulnerable) > 0 {
                d = d * 3 / 2;
            }
            if self.s.status(dst, S::Mist) > 0 {
                d /= 2;
                self.add_status(dst, S::Mist, -1);
            }
            // every blow that lands, not every card played: a card of three hits gives three times
            if mara && matches!(dst, UnitRef::Enemy(_)) {
                self.add_status(dst, S::Fear, MARA_FEAR);
            }
        }
        self.hurt(src, dst, d.max(0), attack)
    }

    /// Lands already-computed damage on block and HP.
    pub(crate) fn hurt(&mut self, src: Option<UnitRef>, dst: UnitRef, d: i32, attack: bool) -> bool {
        let mut killed = false;
        match dst {
            UnitRef::Hero(i) => {
                if self.s.heroes[i as usize].downed {
                    return false;
                }
                let guard = self.s.heroes[i as usize]
                    .guard_by
                    .filter(|g| *g != i && !self.s.heroes[*g as usize].downed);
                match guard {
                    Some(g) => {
                        let h = self.hero(i);
                        let blocked = d.min(h.block);
                        h.block -= blocked;
                        let (hp, block) = (h.hp, h.block);
                        self.ev.push(Event::Damage { src, dst, blocked, lost: 0, attack, hp, block });
                        let over = d - blocked;
                        if over > 0 {
                            self.ev.push(Event::Redirected { from: i, to: g, amount: over });
                            self.hero_take(src, g, over, attack);
                        }
                    }
                    None => self.hero_take(src, i, d, attack),
                }
            }
            UnitRef::Enemy(uid) => {
                let Some(idx) = self.enemy_idx(uid) else { return false };
                let e = &mut self.s.enemies[idx];
                if !e.alive {
                    return false;
                }
                // Koschei remembers who struck at the chain of his death
                let chain = e.flags.contains_key("chain");
                if let (true, Some(UnitRef::Hero(i))) = (chain, src) {
                    *self.s.heroes[i as usize].flags.entry("chain_dmg".into()).or_insert(0) += d;
                }
                let e = &mut self.s.enemies[idx];
                // the hare: the first attack against it each round finds only air
                let dodged = attack && e.flags.contains_key("dodge") && !e.flags.contains_key("turn_dodged");
                let d = if dodged {
                    e.flags.insert("turn_dodged".into(), 1);
                    0
                } else {
                    d
                };
                if dodged {
                    self.ev.push(Event::Missed { unit: dst });
                }
                let e = &mut self.s.enemies[idx];
                // a thing of straw: no one blow does it more than so much harm
                let d = if e.flags.contains_key("hollow") { d.min(HOLLOW_MOST) } else { d };
                // Nochnitsa flicker: on even rounds, incoming attack damage reduced to 1
                let d = if attack && e.flags.contains_key("flicker") && self.s.round % 2 == 0 { d.min(1) } else { d };
                // Mokhovik kinetic moss: attacks under 6 damage are absorbed into kinetic charge
                let d = if attack && e.flags.contains_key("kinetic_moss") && d < 6 {
                    *e.flags.entry("kinetic_charge".into()).or_insert(0) += 1;
                    d.min(1)
                } else {
                    d
                };
                // Baba Yaga mortar: reduces damage by 35% until 28+ damage taken in one turn
                let d = if e.flags.contains_key("yaga_mortar") {
                    let taken = e.flags.entry("mortar_damage".into()).or_insert(0);
                    *taken += d;
                    if *taken >= 28 {
                        e.flags.remove("yaga_mortar");
                    }
                    d * 65 / 100
                } else {
                    d
                };
                let blocked = d.min(e.block);
                e.block -= blocked;
                let mut lost = (d - blocked).min(e.hp);
                // Koschei cannot die while the needle is whole
                if e.flags.contains_key("immortal") {
                    lost = lost.min(e.hp - 1).max(0);
                }
                e.hp -= lost;
                let dead = e.hp <= 0;
                let (hp, block) = (e.hp, e.block);
                // the serpent loosens its coils once it has been hurt enough
                let mut uncoil = false;
                if lost > 0 {
                    if let Some(left) = e.flags.get_mut("coil_left") {
                        *left -= lost;
                        uncoil = *left <= 0;
                    }
                }
                let enrage = attack && lost > 0 && !dead && e.flags.contains_key("enrage");
                self.ev.push(Event::Damage { src, dst, blocked, lost, attack, hp, block });
                if uncoil || dead {
                    self.s.enemies[idx].flags.remove("coil_left");
                    for h in &mut self.s.heroes {
                        if h.flags.get("coiled").copied() == Some(uid as i32) {
                            h.flags.remove("coiled");
                        }
                    }
                }
                // the Shatun: every blow that lands feeds his rage
                if enrage {
                    self.add_status(dst, S::Strength, 2);
                }
                if dead {
                    self.kill(idx, src);
                    killed = true;
                }
            }
        }
        if attack {
            if let Some(s) = src {
                let thorns = self.s.status(dst, S::Thorns);
                let bayun_perch = matches!(dst, UnitRef::Enemy(uid) if self.s.enemy(uid).is_some_and(|e| e.flags.contains_key("bayun_perch")));
                let spell_mirror = matches!(dst, UnitRef::Enemy(uid) if self.s.enemy(uid).is_some_and(|e| e.flags.contains_key("spell_mirror")));
                let mut reflect = thorns;
                if bayun_perch {
                    reflect += 2;
                }
                if spell_mirror {
                    reflect += (d * 30 / 100).max(1);
                }
                if reflect > 0 && s != dst {
                    self.hurt(None, s, reflect, false);
                }
            }
        }
        killed
    }

    fn hero_take(&mut self, src: Option<UnitRef>, i: u8, d: i32, attack: bool) {
        let src_flag = |me: &Self, flag: &str| matches!(src, Some(UnitRef::Enemy(uid)) if me.s.enemy(uid).is_some_and(|e| e.flags.contains_key(flag)));
        // a soul of Nav: half of its blow passes through any shield
        let piercing = attack && src_flag(self, "piercing");
        let lifesteal = attack && src_flag(self, "lifesteal");
        let h = self.hero(i);
        let blocked = if piercing { (d / 2).min(h.block) } else { d.min(h.block) };
        h.block -= blocked;
        let lost = (d - blocked).min(h.hp);
        // what the blow had left over once there was no more hero to take it
        let past = d - blocked - lost;
        let max_hp = h.max_hp;
        let hp_before = h.hp;
        h.hp -= lost;
        let fell = h.hp <= 0;
        let mut valor = 0;
        if h.mechanic == Mechanic::Valor {
            h.valor_rem += blocked + lost;
            valor = h.valor_rem / 5;
            h.valor_rem %= 5;
        }
        let (hp, block) = (h.hp, h.block);
        self.ev.push(Event::Damage { src, dst: UnitRef::Hero(i), blocked, lost, attack, hp, block });
        if valor > 0 && !fell {
            self.add_status(UnitRef::Hero(i), S::Valor, valor);
        }
        if lost > 0 {
            for other in self.standing() {
                if other != i {
                    self.fire(other, Trigger::AllyHpLoss);
                }
            }
            if attack && !fell {
                self.fire(i, Trigger::Hurt);
            }
            // the upyr drinks: half of what it draws heals it
            if lifesteal {
                if let Some(s) = src {
                    self.heal(s, lost / 2);
                }
            }
        }
        if !fell && attack && !self.s.foretelling && self.hero(i).lost_limbs < MOST_LIMBS {
            // measured against the health there was, not the whole: worn down, a hero comes apart sooner
            let share = lost * 100 / hp_before.max(1);
            if share >= MAIM_FROM && self.s.rng.chance(((share - MAIM_FROM) * 2 + 25).min(70)) {
                // whichever arm is still there, by lot when both are
                let first = if self.s.rng.chance(50) { WEAPON_ARM } else { SHIELD_ARM };
                let h = self.hero(i);
                let arm = if h.lost_arms & first == 0 { first } else { first ^ (WEAPON_ARM | SHIELD_ARM) };
                h.lost_limbs += 1;
                h.lost_arms |= arm;
                let limbs = h.lost_limbs;
                let part = format!("own_{}_{}", if arm == WEAPON_ARM { "weapon" } else { "shield" }, h.def);
                self.ev.push(Event::Maimed { player: i, limbs, arm });
                // it stays with its owner, as a card that does nothing
                if self.c.cards.contains_key(part.as_str()) {
                    self.add_card(i, &part, Pile::Discard);
                    self.hero(i).new_parts.push(part.into());
                }
                // what is left to fight with
                let both = limbs >= 2;
                for card in stump_cards(arm).iter().chain(if both { BOTH_ARMS_CARDS.iter() } else { [].iter() }) {
                    if self.c.cards.contains_key(*card) {
                        self.add_card(i, card, Pile::Discard);
                        self.hero(i).new_parts.push((*card).into());
                    }
                }
            }
        }
        if fell && !self.unfound(i) {
            if attack && past * 100 >= max_hp.max(1) * BEHEAD_PAST {
                // a head already gone is not lost twice
                let first = !self.hero(i).walks_headless;
                self.hero(i).beheaded = true;
                self.ev.push(Event::Beheaded { player: i });
                let part = format!("own_head_{}", self.hero(i).def);
                if first && self.c.cards.contains_key(part.as_str()) {
                    self.hero(i).new_parts.push(part.into());
                }
            }
            self.down(i);
        }
    }

    /// The erased name: the first time in a fight a hero would fall, death, which looks for
    /// them by name, does not find them, and they stand on at 1 HP.
    fn unfound(&mut self, i: u8) -> bool {
        let h = self.hero(i);
        if !h.relics.iter().any(|r| r == crate::run::SECRET_RELICS[1]) || h.flags.contains_key("erased_name") {
            return false;
        }
        h.flags.insert("erased_name".into(), 1);
        h.hp = 1;
        true
    }

    fn lose_hp(&mut self, u: UnitRef, n: i32) {
        if n <= 0 || !self.exists(u) {
            return;
        }
        match u {
            UnitRef::Hero(i) => {
                let h = self.hero(i);
                let lost = n.min(h.hp);
                h.hp -= lost;
                let fell = h.hp <= 0;
                let (hp, block) = (h.hp, h.block);
                self.ev.push(Event::Damage { src: None, dst: u, blocked: 0, lost, attack: false, hp, block });
                if lost > 0 {
                    for other in self.standing() {
                        if other != i {
                            self.fire(other, Trigger::AllyHpLoss);
                        }
                    }
                }
                if fell && !self.unfound(i) {
                    self.down(i);
                }
            }
            UnitRef::Enemy(uid) => {
                let idx = self.enemy_idx(uid).unwrap();
                let e = &mut self.s.enemies[idx];
                let lost = n.min(e.hp);
                e.hp -= lost;
                let dead = e.hp <= 0;
                let (hp, block) = (e.hp, e.block);
                self.ev.push(Event::Damage { src: None, dst: u, blocked: 0, lost, attack: false, hp, block });
                if dead {
                    self.kill(idx, None);
                }
            }
        }
    }

    fn down(&mut self, i: u8) {
        let has_curse = self.c.cards.contains_key(NAV_MARK);
        let h = self.hero(i);
        h.hp = 0;
        h.downed = true;
        h.ready = false;
        h.block = 0;
        h.guard_by = None;
        h.falls += 1;
        let hand = std::mem::take(&mut h.hand);
        h.discard.extend(hand);
        let cursed = h.falls > 1 && has_curse;
        self.ev.push(Event::Downed { player: i });
        if cursed {
            self.hero(i).new_curses += 1;
            self.add_card(i, NAV_MARK, Pile::Discard);
            // one mark a fall: lying on until the fight ends does not mark them again
            self.hero(i).flags.insert("marked_fall".into(), 1);
        }
        for h in &mut self.s.heroes {
            if h.guard_by == Some(i) {
                h.guard_by = None;
            }
        }
    }

    fn revive(&mut self, i: u8, pct: i32, by: Option<u8>) {
        let h = self.hero(i);
        if !h.downed || h.beheaded {
            return;
        }
        h.downed = false;
        h.flags.remove("marked_fall");
        h.hp = (h.max_hp * pct / 100).max(1);
        let hp = h.hp;
        self.ev.push(Event::Raised { player: i, by, hp });
        for s in self.standing() {
            self.fire(s, Trigger::AllyRaised);
        }
    }

    fn kill(&mut self, idx: usize, src: Option<UnitRef>) {
        // the kostomakha falls apart once and lies as bones until its next turn, when it gets
        // up at half its strength: the heroes get a turn's grace, and see it coming
        if self.s.enemies[idx].flags.remove("rises").is_some() {
            let reassemble = self.c.enemies.get(&self.s.enemies[idx].def).and_then(|d| d.moves.iter().position(|m| m.id == "reassemble"));
            let e = &mut self.s.enemies[idx];
            e.hp = 0;
            e.block = 0;
            e.statuses.clear();
            e.flags.insert(crate::state::BONES.into(), 1);
            e.intent = reassemble.map(|mv| Intent { mv, target: None });
            let uid = e.uid;
            self.ev.push(Event::Collapsed { enemy: uid });
            if let Some(mv) = reassemble {
                self.ev.push(Event::Intent { enemy: uid, mv, target: None });
            }
            return;
        }
        let e = &mut self.s.enemies[idx];
        e.alive = false;
        e.hp = 0;
        e.intent = None;
        let uid = e.uid;
        let def = e.def.clone();
        // a dead thief drops what it took
        for (slot, gold) in std::mem::take(&mut e.stolen) {
            self.s.heroes[slot as usize].gold_lost -= gold;
            self.ev.push(Event::GoldReturned { player: slot, amount: gold });
        }
        self.ev.push(Event::EnemyDied { enemy: uid });
        if let Some(d) = self.c.enemies.get(&def) {
            if !d.on_death.is_empty() {
                let fx = d.on_death.clone();
                let mut ctx = Ctx::passive(UnitRef::Enemy(uid));
                self.run(&fx, &mut ctx);
            }
        }
        if let Some(UnitRef::Hero(i)) = src {
            // Devour: a kill in Beast form feeds the werewolf
            if self.s.heroes[i as usize].form == Form::Beast {
                self.heal(UnitRef::Hero(i), 3);
            }
            self.fire(i, Trigger::Kill);
        }
    }

    // ---- cards and piles ----

    pub(crate) fn draw(&mut self, slot: u8, n: i32) {
        let mut count = 0;
        for _ in 0..n.max(0) {
            if self.s.heroes[slot as usize].hand.len() >= HAND_LIMIT {
                break;
            }
            if self.s.heroes[slot as usize].draw.is_empty() {
                if self.s.heroes[slot as usize].discard.is_empty() {
                    break;
                }
                let mut pile = std::mem::take(&mut self.s.heroes[slot as usize].discard);
                self.s.rng.shuffle(&mut pile);
                self.s.heroes[slot as usize].draw = pile;
                self.ev.push(Event::Shuffled { player: slot });
            }
            let h = self.hero(slot);
            let card = h.draw.pop().unwrap();
            h.hand.push(card);
            count += 1;
        }
        if count > 0 {
            self.ev.push(Event::Drawn { player: slot, count });
        }
    }

    pub(crate) fn add_card(&mut self, slot: u8, def: &str, pile: Pile) {
        self.s.next_uid += 1;
        let card = Card { uid: self.s.next_uid, def: def.to_string(), upgraded: false, free: false };
        let h = self.hero(slot);
        let pile = if pile == Pile::Hand && h.hand.len() >= HAND_LIMIT { Pile::Discard } else { pile };
        match pile {
            Pile::Hand => h.hand.push(card),
            Pile::Discard => h.discard.push(card),
            Pile::Draw => {
                // shuffled in at a random depth
                let at = self.s.rng.below(self.s.heroes[slot as usize].draw.len() as u32 + 1) as usize;
                self.s.heroes[slot as usize].draw.insert(at, card);
            }
        }
        self.ev.push(Event::CardAdded { player: slot, card: def.to_string(), pile });
    }

    fn play(&mut self, p: u8, uid: u32, target: Option<UnitRef>) -> Result<(), ActionError> {
        let h = &self.s.heroes[p as usize];
        let pos = h.hand.iter().position(|c| c.uid == uid).ok_or(ActionError::NoSuchCard)?;
        let card = h.hand[pos].clone();
        let def = self.c.cards.get(&card.def).ok_or(ActionError::NoSuchCard)?;
        let mut view = def.view(card.upgraded, h.form);
        if view.has(Keyword::Unplayable) {
            // the icy tale: the dead listen to one who speaks so; a curse is told away for 1,
            // burns, and leaves 4 block
            let icy = view.kind == CardType::Curse && h.relics.iter().any(|r| r == crate::run::SECRET_RELICS[2]);
            if !icy {
                return Err(ActionError::Unplayable);
            }
            view.cost = 1;
            view.x_cost = false;
            view.target = CardTarget::None;
            view.effects = vec![Effect::Block(Amount::N(4), Tgt::Me)];
            view.keywords.retain(|k| *k != Keyword::Unplayable);
            view.keywords.push(Keyword::Exhaust);
        }
        if view.kind == CardType::Attack && h.flags.contains_key("coiled") {
            return Err(ActionError::Coiled);
        }
        if needs_lost_arm(&view, h.lost_arms, h.def.as_str(), h.form) {
            return Err(ActionError::NoArm);
        }
        if h.flags.contains_key("mired") || h.flags.contains_key("asleep") {
            return Err(ActionError::Held);
        }
        // Mara's frost: the leftmost cards in hand cannot be played this turn
        if (pos as i32) < h.flags.get("frozen").copied().unwrap_or(0) {
            return Err(ActionError::Frozen);
        }
        // the igosha cowers behind its fellows: aim at them first
        if let Some(UnitRef::Enemy(uid)) = target {
            let hiding = self.s.enemy(uid).is_some_and(|e| e.alive && e.flags.contains_key("hiding"));
            if hiding && self.s.living_enemies().any(|e| !e.flags.contains_key("hiding")) && view.target == CardTarget::Enemy {
                return Err(ActionError::Hidden);
            }
        }
        let carves = view.effects.iter().any(|e| matches!(e, Effect::Rune(..)));
        let free_rune = carves && h.flags.get("free_rune").copied().unwrap_or(0) > 0;
        let cost = if card.free || free_rune {
            0
        } else if view.x_cost {
            h.energy
        } else {
            view.cost
        };
        if h.energy < cost {
            return Err(ActionError::NotEnoughEnergy);
        }
        let target = match (view.target, target) {
            (CardTarget::None, _) => None,
            (CardTarget::Enemy, Some(t @ UnitRef::Enemy(_))) if self.exists(t) => Some(t),
            (CardTarget::Ally, Some(t @ UnitRef::Hero(j))) if j != p && self.exists(t) => Some(t),
            // alone, an ally card still plays for its base effect
            (CardTarget::Ally, None) if self.standing().len() == 1 => None,
            (CardTarget::AnyHero, Some(t @ UnitRef::Hero(_))) if self.exists(t) => Some(t),
            (CardTarget::Downed, Some(t @ UnitRef::Hero(j))) if self.s.hero(j).is_some_and(|a| a.downed) => Some(t),
            _ => return Err(ActionError::BadTarget),
        };

        // pulling a friend out of the mire: any card played on them does it
        if let Some(UnitRef::Hero(j)) = target {
            if j != p {
                self.hero(j).flags.remove("mired");
            }
        }
        // the mavka's charm: the next attack turns on a friend, or is wasted when alone
        let charmed = view.kind == CardType::Attack && matches!(target, Some(UnitRef::Enemy(_))) && self.s.heroes[p as usize].flags.remove("charmed").is_some();
        let target = if charmed {
            let others: Vec<u8> = self.standing().into_iter().filter(|s| *s != p).collect();
            self.s.rng.pick(&others).copied().map(UnitRef::Hero)
        } else {
            target
        };
        // the Leshy's tangled paths: an aimed blow goes astray
        let target = match target {
            Some(UnitRef::Enemy(_)) if self.s.heroes[p as usize].flags.contains_key("confused") => {
                let foes = self.living();
                self.s.rng.pick(&foes).copied().map(UnitRef::Enemy)
            }
            other => other,
        };
        let h = self.hero(p);
        h.hand.remove(pos);
        h.energy -= cost;
        if free_rune {
            *h.flags.entry("free_rune".into()).or_insert(1) -= 1;
        }
        self.ev.push(Event::CardPlayed { player: p, card: card.def.clone(), upgraded: card.upgraded, target });
        // charmed and alone: the blow has no friend to turn on, and is seen to go to waste
        if charmed && target.is_none() {
            self.ev.push(Event::Missed { unit: UnitRef::Hero(p) });
        }
        let mut ctx = Ctx {
            src: UnitRef::Hero(p),
            target,
            x: cost,
            consumed: 0,
            fatal: false,
            attack: view.kind == CardType::Attack,
            card: true,
            bled: false,
        };
        self.run(&view.effects, &mut ctx);

        let h = self.hero(p);
        if view.kind == CardType::Power {
            h.powers.extend(view.passives.iter().cloned());
        } else if view.has(Keyword::Exhaust) || card.free {
            self.exhaust_card(p, card.clone());
        } else {
            h.discard.push(card.clone());
        }
        self.s.log.push(Played { player: p, card: card.def.clone(), upgraded: card.upgraded, kind: view.kind });

        if self.s.heroes[p as usize].downed {
            return Ok(());
        }
        self.fire(p, Trigger::CardPlayed);
        if view.kind == CardType::Skill {
            self.fire(p, Trigger::SkillPlayed);
        }
        if view.kind == CardType::Attack {
            self.fire(p, Trigger::AttackPlayed);
            let h = self.hero(p);
            if h.mechanic == Mechanic::Moon && h.form == Form::Human {
                h.moon += 1;
                if h.moon >= 5 {
                    self.shift(p);
                }
            }
        }
        if self.s.heroes[p as usize].mask == Some(MaskKind::Goat) {
            let mut others = self.standing();
            others.retain(|s| *s != p);
            let to = self.s.rng.pick(&others).copied().unwrap_or(p);
            self.gain_block(UnitRef::Hero(to), 2);
        }
        Ok(())
    }

    fn use_potion(&mut self, p: u8, slot: u8, target: Option<UnitRef>) -> Result<(), ActionError> {
        let id = self.s.heroes[p as usize].potions.get(slot as usize).cloned().ok_or(ActionError::NoSuchPotion)?;
        let def = self.c.potions.get(&id).ok_or(ActionError::NoSuchPotion)?;
        let target = match (def.target, target) {
            (CardTarget::None, _) => None,
            (CardTarget::Enemy, Some(t @ UnitRef::Enemy(_))) if self.exists(t) => Some(t),
            // a draught for "an ally" may always be drunk yourself
            (CardTarget::Ally | CardTarget::AnyHero, Some(t @ UnitRef::Hero(_))) if self.exists(t) => Some(t),
            (CardTarget::Ally | CardTarget::AnyHero, None) => Some(UnitRef::Hero(p)),
            (CardTarget::Downed, Some(t @ UnitRef::Hero(j))) if self.s.hero(j).is_some_and(|a| a.downed) => Some(t),
            _ => return Err(ActionError::BadTarget),
        };
        let effects = def.effects.clone();
        self.hero(p).potions.remove(slot as usize);
        self.ev.push(Event::PotionUsed { player: p, potion: id, target });
        let mut ctx = Ctx { src: UnitRef::Hero(p), target, x: 0, consumed: 0, fatal: false, attack: false, card: false, bled: false };
        self.run(&effects, &mut ctx);
        Ok(())
    }

    fn raise(&mut self, p: u8, ally: u8, discard: u32) -> Result<(), ActionError> {
        let cost = self.s.raise_cost;
        let h = &self.s.heroes[p as usize];
        if h.energy < cost {
            return Err(ActionError::NotEnoughEnergy);
        }
        let pos = h.hand.iter().position(|c| c.uid == discard).ok_or(ActionError::NoSuchCard)?;
        if ally == p || !self.s.hero(ally).is_some_and(|a| a.downed) {
            return Err(ActionError::NotDowned);
        }
        let h = self.hero(p);
        h.energy -= cost;
        let card = h.hand.remove(pos);
        h.discard.push(card);
        self.revive(ally, 25, Some(p));
        Ok(())
    }

    fn pass(&mut self, p: u8, card: u32, to: u8) -> Result<(), ActionError> {
        let h = &self.s.heroes[p as usize];
        let pos = h.hand.iter().position(|c| c.uid == card).ok_or(ActionError::NoSuchCard)?;
        let passable = self.c.cards.get(&h.hand[pos].def).is_some_and(|d| d.keywords.contains(&Keyword::Passable));
        if !passable || to == p || !self.s.hero(to).is_some_and(|a| !a.downed) {
            return Err(ActionError::CannotPass);
        }
        if h.energy < 1 {
            return Err(ActionError::NotEnoughEnergy);
        }
        let h = self.hero(p);
        h.energy -= 1;
        let card = h.hand.remove(pos);
        self.ev.push(Event::Passed { from: p, to, card: card.def.clone() });
        self.hero(to).hand.push(card);
        Ok(())
    }

    fn shift(&mut self, p: u8) {
        if self.s.status(UnitRef::Hero(p), S::Chained) > 0 {
            return;
        }
        // only a werewolf has a beast to turn into: a copy of its card played by another (the
        // Skomoroh's echo) turned them into a "beast" with no pictures and no block of their own
        if self.s.heroes[p as usize].mechanic != Mechanic::Moon {
            return;
        }
        let h = self.hero(p);
        let to_beast = h.form == Form::Human;
        // the moon gathered as a man is the beast's fury: a Strength for every two, kept while it
        // stays a beast, so waiting for the full moon pays and turning early costs it
        let fury = if to_beast { (h.moon + 1) / 2 } else { h.flags.remove("lunar_fury").unwrap_or(0) };
        h.form = if to_beast { Form::Beast } else { Form::Human };
        h.moon = 0;
        let form = h.form;
        if to_beast && fury > 0 {
            self.hero(p).flags.insert("lunar_fury".into(), fury);
            self.add_status(UnitRef::Hero(p), S::Strength, fury);
        } else if !to_beast && fury > 0 {
            let has = self.s.status(UnitRef::Hero(p), S::Strength);
            self.add_status(UnitRef::Hero(p), S::Strength, -fury.min(has.max(0)));
        }
        self.ev.push(Event::Shifted { player: p, form });
        self.fire(p, Trigger::Shifted);
    }

    // ---- passives ----

    pub(crate) fn fire(&mut self, slot: u8, on: Trigger) {
        if self.depth >= 6 {
            return;
        }
        let h = &self.s.heroes[slot as usize];
        if h.downed {
            return;
        }
        let mut lists: Vec<Vec<Effect>> = Vec::new();
        for r in &h.relics {
            if let Some(def) = self.c.relics.get(r) {
                lists.extend(def.passives.iter().filter(|p| p.on == on).map(|p| p.effects.clone()));
            }
        }
        lists.extend(h.powers.iter().filter(|p| p.on == on).map(|p| p.effects.clone()));
        self.depth += 1;
        for fx in lists {
            let mut ctx = Ctx::passive(UnitRef::Hero(slot));
            self.run(&fx, &mut ctx);
        }
        self.depth -= 1;
    }

    fn fire_runes(&mut self, slot: u8) {
        // the thunder-oak staff: the first rune its bearer carved fires twice
        let twice = self.s.heroes[slot as usize].flags.contains_key("first_rune_twice");
        for (i, rune) in self.s.heroes[slot as usize].runes.clone().into_iter().enumerate() {
            self.fire_rune(slot, rune);
            if i == 0 && twice {
                self.fire_rune(slot, rune);
            }
        }
    }

    pub(crate) fn fire_rune(&mut self, slot: u8, rune: RuneKind) {
        let me = UnitRef::Hero(slot);
        {
            if self.s.heroes[slot as usize].downed {
                return;
            }
            self.ev.push(Event::RuneFired { player: slot, rune });
            match rune {
                RuneKind::Thunder => {
                    let foes = self.living();
                    // Storm: thunder strikes every enemy instead of one
                    if self.s.heroes.iter().any(|h| !h.downed && h.flags.contains_key("storm")) {
                        for t in foes {
                            self.hurt(Some(me), UnitRef::Enemy(t), 6, false);
                        }
                    } else if self.s.foretelling {
                        // who it strikes is not known yet
                    } else if let Some(t) = self.s.rng.pick(&foes).copied() {
                        self.hurt(Some(me), UnitRef::Enemy(t), 6, false);
                    }
                }
                RuneKind::Frost => self.gain_block(me, 4),
                RuneKind::Mist => self.add_status(me, S::Mist, 1),
                RuneKind::Flame => {
                    for t in self.living() {
                        self.hurt(Some(me), UnitRef::Enemy(t), 3, false);
                    }
                }
                RuneKind::Zhiva => self.heal(me, 2),
            }
        }
    }

    // ---- effect interpreter ----

    fn targets(&mut self, ctx: &Ctx, t: Tgt) -> Vec<UnitRef> {
        let hero_side = matches!(ctx.src, UnitRef::Hero(_));
        let heroes = |me: &Self| me.standing().into_iter().map(UnitRef::Hero).collect::<Vec<_>>();
        let enemies = |me: &Self| me.living().into_iter().map(UnitRef::Enemy).collect::<Vec<_>>();
        match t {
            Tgt::Me => vec![ctx.src],
            Tgt::Target => ctx.target.into_iter().collect(),
            Tgt::AllFoes => {
                if hero_side {
                    enemies(self)
                } else {
                    heroes(self)
                }
            }
            Tgt::RandomFoe if self.s.foretelling => vec![],
            Tgt::WeakestFoe => self.s.living_enemies().min_by_key(|e| e.hp).map(|e| UnitRef::Enemy(e.uid)).into_iter().collect(),
            Tgt::RandomFoe => {
                let all = if hero_side { enemies(self) } else { heroes(self) };
                self.s.rng.pick(&all).copied().into_iter().collect()
            }
            Tgt::AllFriends => {
                if hero_side {
                    heroes(self)
                } else {
                    enemies(self)
                }
            }
            Tgt::OtherFriends => {
                let mut all = if hero_side { heroes(self) } else { enemies(self) };
                all.retain(|u| *u != ctx.src);
                all
            }
        }
    }

    fn amount(&mut self, ctx: &Ctx, a: &Amount) -> i32 {
        match a {
            Amount::N(n) => *n,
            Amount::X(m) => ctx.x * m,
            Amount::Stat(st, who, m) => {
                let us = self.targets(ctx, *who);
                us.iter().map(|u| self.s.status(*u, *st)).sum::<i32>() * m
            }
            Amount::Consumed(m) => ctx.consumed * m,
            Amount::Pack(base, each) => {
                let others = self.s.enemies.iter().filter(|e| e.alive && UnitRef::Enemy(e.uid) != ctx.src).count() as i32;
                base + others * each
            }
            Amount::MissingHp(pct) => match ctx.src {
                UnitRef::Hero(i) => self.s.hero(i).map_or(0, |h| (h.max_hp - h.hp) * pct / 100),
                UnitRef::Enemy(u) => self.s.enemy(u).map_or(0, |e| (e.max_hp - e.hp) * pct / 100),
            },
            Amount::PlayedThisRound(m) => self.s.log.len() as i32 * m,
            Amount::Curses(who, m) => {
                let us = self.targets(ctx, *who);
                us.iter().map(|u| self.s.status(*u, S::Rot) + self.s.status(*u, S::Hex) + self.s.status(*u, S::Weak)).sum::<i32>() * m
            }
            Amount::Runes(kind, m) => {
                let n: usize = self.s.heroes.iter().map(|h| h.runes.iter().filter(|r| *r == kind).count()).sum();
                n as i32 * m
            }
            Amount::Discarded(m) => self.hero_flag(ctx.src, "last_discarded") * m,
            Amount::DiscardedThisTurn(m) => self.hero_flag(ctx.src, "turn_discarded") * m,
            Amount::HandSize(m) => match ctx.src {
                UnitRef::Hero(i) => self.s.heroes[i as usize].hand.len() as i32 * m,
                UnitRef::Enemy(_) => 0,
            },
            Amount::ExhaustPile(m) => match ctx.src {
                UnitRef::Hero(i) => self.s.heroes[i as usize].exhaust.len() as i32 * m,
                UnitRef::Enemy(_) => 0,
            },
            Amount::Hoard(base, pct) => match ctx.src {
                UnitRef::Enemy(u) => base + self.s.enemy(u).map_or(0, |e| e.stolen.values().sum::<i32>()) * pct / 100,
                UnitRef::Hero(_) => *base,
            },
        }
    }

    fn hero_flag(&self, u: UnitRef, key: &str) -> i32 {
        match u {
            UnitRef::Hero(i) => self.s.heroes[i as usize].flags.get(key).copied().unwrap_or(0),
            UnitRef::Enemy(_) => 0,
        }
    }

    /// Throws the card at `pos` of a hero's hand away, and runs what it does when thrown.
    fn discard_at(&mut self, slot: u8, pos: usize) {
        let card = self.hero(slot).hand.remove(pos);
        let on_discard = self.c.cards.get(&card.def).map(|d| d.view(card.upgraded, self.s.heroes[slot as usize].form).on_discard).unwrap_or_default();
        self.ev.push(Event::Discarded { player: slot, card: card.def.clone() });
        self.hero(slot).discard.push(card);
        *self.hero(slot).flags.entry("turn_discarded".into()).or_insert(0) += 1;
        if self.depth < 6 && !on_discard.is_empty() {
            self.depth += 1;
            let mut ctx = Ctx::passive(UnitRef::Hero(slot));
            ctx.card = true;
            self.run(&on_discard, &mut ctx);
            self.depth -= 1;
        }
        self.fire(slot, Trigger::Discarded);
    }

    /// The player picks one card to throw for a discard under way; the last pick lets the card go on.
    fn pick_discard(&mut self, p: u8, uid: u32) -> Result<(), ActionError> {
        let pos = self.s.heroes[p as usize].hand.iter().position(|c| c.uid == uid).ok_or(ActionError::NoSuchCard)?;
        self.discard_at(p, pos);
        let Some(mut ch) = self.hero(p).choosing.take() else { return Ok(()) };
        ch.left -= 1;
        ch.picked += 1;
        let empty = self.s.heroes[p as usize].hand.is_empty();
        if ch.left > 0 && !empty && !self.s.heroes[p as usize].downed {
            self.hero(p).choosing = Some(ch);
            return Ok(());
        }
        self.hero(p).flags.insert("last_discarded".into(), ch.picked as i32);
        let mut ctx = Ctx { src: UnitRef::Hero(p), target: ch.target.filter(|t| self.exists(*t)), x: ch.x, consumed: 0, fatal: false, attack: ch.attack, card: true, bled: false };
        self.run(&ch.rest, &mut ctx);
        Ok(())
    }

    /// Discards `n` random cards from a hero's hand (all of it when `n` is `None`); returns how many went.
    fn discard_cards(&mut self, slot: u8, n: Option<i32>) -> i32 {
        let mut gone = 0;
        loop {
            let left = self.s.heroes[slot as usize].hand.len();
            if left == 0 || n.is_some_and(|n| gone >= n) || self.s.heroes[slot as usize].downed {
                break;
            }
            let pos = if n.is_none() { left - 1 } else { self.s.rng.below(left as u32) as usize };
            self.discard_at(slot, pos);
            gone += 1;
        }
        self.hero(slot).flags.insert("last_discarded".into(), gone);
        gone
    }

    /// Burns a card out of play for this fight and lets the hero's powers know.
    fn exhaust_card(&mut self, slot: u8, card: Card) {
        self.ev.push(Event::Exhausted { player: slot, card: card.def.clone() });
        self.hero(slot).exhaust.push(card);
        self.fire(slot, Trigger::Exhausted);
    }

    fn cond(&mut self, ctx: &Ctx, c: &Cond) -> bool {
        let src_hero = match ctx.src {
            UnitRef::Hero(i) => self.s.hero(i),
            _ => None,
        };
        match c {
            Cond::Has(st, who, min) => {
                let us = self.targets(ctx, *who);
                !us.is_empty() && us.iter().all(|u| self.s.status(*u, *st) >= *min)
            }
            Cond::Link(kind) => self.s.log.iter().any(|p| UnitRef::Hero(p.player) != ctx.src && p.kind == *kind),
            Cond::Fatal => ctx.fatal,
            Cond::Form(f) => src_hero.is_some_and(|h| h.form == *f),
            Cond::Mask(m) => src_hero.is_some_and(|h| h.mask == Some(*m)),
            Cond::Coop => self.s.is_coop(),
            Cond::Not(inner) => !self.cond(ctx, inner),
        }
    }

    fn bleed(&mut self, ctx: &mut Ctx) {
        if ctx.attack && !ctx.bled {
            ctx.bled = true;
            let b = self.s.status(ctx.src, S::Bleed);
            if b > 0 {
                self.hurt(None, ctx.src, b, false);
            }
        }
    }

    pub(crate) fn run(&mut self, effects: &[Effect], ctx: &mut Ctx) {
        for (at, e) in effects.iter().enumerate() {
            // "discard N": the player picks them, and the rest of the card waits for that
            if let (Effect::Discard(a), UnitRef::Hero(me)) = (e, ctx.src) {
                let n = self.amount(ctx, a).max(0) as usize;
                let h = &self.s.heroes[me as usize];
                if n > 0 && h.hand.len() > n && h.choosing.is_none() && !h.downed {
                    self.hero(me).flags.insert("last_discarded".into(), 0);
                    self.hero(me).choosing = Some(crate::state::Choosing { left: n as u8, picked: 0, rest: effects[at + 1..].to_vec(), target: ctx.target, x: ctx.x, attack: ctx.attack });
                    self.ev.push(Event::Choose { player: me, count: n as u8 });
                    return;
                }
            }
            match e {
                Effect::Damage(a, t) | Effect::Hits(a, _, t) => {
                    let times = if let Effect::Hits(_, k, _) = e { *k } else { 1 };
                    self.bleed(ctx);
                    let n = self.amount(ctx, a);
                    ctx.fatal = false;
                    for _ in 0..times {
                        for u in self.targets(ctx, *t) {
                            // the duck is on the wing: only blows that sweep everyone, and runes, reach it
                            if *t == Tgt::Target && matches!(u, UnitRef::Enemy(id) if self.s.enemy(id).is_some_and(|e| e.flags.contains_key("flying"))) {
                                continue;
                            }
                            if self.strike(Some(ctx.src), u, n, ctx.attack) {
                                ctx.fatal = true;
                            }
                        }
                    }
                }
                Effect::Block(a, t) | Effect::Guard(a, t) => {
                    let guard = matches!(e, Effect::Guard(..));
                    let mut n = self.amount(ctx, a);
                    if ctx.card {
                        n += self.s.status(ctx.src, S::Fortitude);
                    }
                    let beast = matches!(ctx.src, UnitRef::Hero(i) if self.s.heroes[i as usize].form == Form::Beast);
                    for u in self.targets(ctx, *t) {
                        // the Beast can't shield itself with its own cards
                        if ctx.card && beast && u == ctx.src {
                            continue;
                        }
                        self.gain_block(u, n);
                        if let (true, UnitRef::Hero(to), UnitRef::Hero(by)) = (guard, u, ctx.src) {
                            if to != by {
                                self.hero(to).guard_by = Some(by);
                            }
                        }
                    }
                }
                Effect::Apply(st, a, t) => {
                    let mut n = self.amount(ctx, a);
                    // cards speak of Fear in whole percent; it is kept in tenths
                    if *st == S::Fear {
                        n *= 10;
                    }
                    for u in self.targets(ctx, *t) {
                        self.add_status(u, *st, n);
                    }
                }
                Effect::Remove(st, t) => {
                    for u in self.targets(ctx, *t) {
                        self.clear_status(u, *st);
                    }
                }
                Effect::Consume(st, t) => {
                    ctx.consumed = 0;
                    for u in self.targets(ctx, *t) {
                        let n = self.clear_status(u, *st);
                        ctx.consumed += if *st == S::Fear { n / 10 } else { n };
                    }
                }
                Effect::Cleanse(t) => {
                    for u in self.targets(ctx, *t) {
                        let all: Vec<StatusKind> = match u {
                            UnitRef::Hero(i) => self.s.heroes[i as usize].statuses.keys().copied().collect(),
                            UnitRef::Enemy(i) => self.s.enemy(i).map(|e| e.statuses.keys().copied().collect()).unwrap_or_default(),
                        };
                        for st in all.into_iter().filter(|s| s.debuff()) {
                            self.clear_status(u, st);
                        }
                    }
                }
                Effect::Draw(a) => {
                    let n = self.amount(ctx, a);
                    if let UnitRef::Hero(i) = ctx.src {
                        self.draw(i, n);
                    }
                }
                Effect::Energy(a, t) => {
                    let n = self.amount(ctx, a);
                    for u in self.targets(ctx, *t) {
                        if let UnitRef::Hero(i) = u {
                            let h = self.hero(i);
                            let delta = n.max(-h.energy);
                            h.energy += delta;
                            self.ev.push(Event::Energy { player: i, delta });
                        }
                    }
                }
                Effect::Heal(a, t) => {
                    let n = self.amount(ctx, a);
                    for u in self.targets(ctx, *t) {
                        self.heal(u, n);
                    }
                }
                Effect::LoseHp(a, t) => {
                    let n = self.amount(ctx, a);
                    for u in self.targets(ctx, *t) {
                        self.lose_hp(u, n);
                    }
                }
                Effect::Revive(pct, t) => {
                    // downed heroes are invisible to `targets`, so resolve by hand
                    let who: Vec<u8> = match t {
                        Tgt::Target => match ctx.target {
                            Some(UnitRef::Hero(i)) => vec![i],
                            _ => vec![],
                        },
                        _ => self.s.heroes.iter().filter(|h| h.downed).map(|h| h.slot).collect(),
                    };
                    let by = if let UnitRef::Hero(i) = ctx.src { Some(i) } else { None };
                    for i in who {
                        self.revive(i, *pct, by);
                    }
                }
                Effect::AddCard { card, pile, to, count } => {
                    for u in self.targets(ctx, *to) {
                        if let UnitRef::Hero(i) = u {
                            for _ in 0..*count {
                                self.add_card(i, card, *pile);
                            }
                        }
                    }
                }
                Effect::Taunt => {
                    if let UnitRef::Hero(i) = ctx.src {
                        for e in self.s.enemies.iter_mut().filter(|e| e.alive) {
                            if let Some(intent) = &mut e.intent {
                                if intent.target.is_some() {
                                    intent.target = Some(i);
                                }
                            }
                        }
                        self.ev.push(Event::Taunt { player: i });
                    }
                }
                Effect::If(c, then, otherwise) => {
                    let branch = if self.cond(ctx, c) { then } else { otherwise };
                    self.run(branch, ctx);
                }
                Effect::Repeat(a, body) => {
                    for _ in 0..self.amount(ctx, a).clamp(0, 50) {
                        self.run(body, ctx);
                    }
                }
                Effect::Rune(kind, t) => {
                    for u in self.targets(ctx, *t) {
                        if let UnitRef::Hero(i) = u {
                            let cap = if u == ctx.src { 5 } else { 3 };
                            let h = self.hero(i);
                            // no room left: the oldest rune gives way, and discharges as it goes
                            let spent = (h.runes.len() >= cap).then(|| h.runes.remove(0));
                            h.runes.push(*kind);
                            if let Some(old) = spent {
                                self.ev.push(Event::RuneDischarged { player: i, rune: old });
                                self.fire_rune(i, old);
                            }
                            self.ev.push(Event::RuneCarved { player: i, rune: *kind });
                        }
                    }
                }
                Effect::Invoke => {
                    for i in self.standing() {
                        self.fire_runes(i);
                    }
                }
                Effect::Shift => {
                    if let UnitRef::Hero(i) = ctx.src {
                        self.shift(i);
                    }
                }
                Effect::Mask(m) => {
                    if let UnitRef::Hero(i) = ctx.src {
                        if self.s.heroes[i as usize].mask != Some(*m) {
                            self.hero(i).mask = Some(*m);
                            self.ev.push(Event::MaskChanged { player: i, mask: *m });
                            self.fire(i, Trigger::MaskChanged);
                        }
                    }
                }
                Effect::Summon(id, n) => {
                    for _ in 0..*n {
                        if let Some(uid) = self.spawn(id) {
                            self.ev.push(Event::Summoned { enemy: uid, def: id.clone() });
                        }
                    }
                }
                Effect::StealGold(n) => {
                    if let (UnitRef::Enemy(uid), Some(UnitRef::Hero(i))) = (ctx.src, ctx.target) {
                        self.hero(i).gold_lost += n;
                        if let Some(idx) = self.enemy_idx(uid) {
                            *self.s.enemies[idx].stolen.entry(i).or_insert(0) += n;
                        }
                        self.ev.push(Event::GoldStolen { player: i, amount: *n });
                    }
                }
                Effect::StealGoldPct(pct) => {
                    if let (UnitRef::Enemy(uid), Some(UnitRef::Hero(i))) = (ctx.src, ctx.target) {
                        let h = self.s.heroes[i as usize].clone();
                        let n = (h.gold - h.gold_lost).max(0) * pct / 100;
                        if n > 0 {
                            self.hero(i).gold_lost += n;
                            if let Some(idx) = self.enemy_idx(uid) {
                                *self.s.enemies[idx].stolen.entry(i).or_insert(0) += n;
                            }
                            self.ev.push(Event::GoldStolen { player: i, amount: n });
                        }
                    }
                }
                Effect::ScatterHoard => {
                    if let UnitRef::Enemy(uid) = ctx.src {
                        if let Some(idx) = self.enemy_idx(uid) {
                            let n: i32 = std::mem::take(&mut self.s.enemies[idx].stolen).values().sum();
                            if n > 0 {
                                self.ev.push(Event::GoldScattered { enemy: uid, amount: n });
                            }
                        }
                    }
                }
                Effect::Flee => {
                    if let UnitRef::Enemy(uid) = ctx.src {
                        if let Some(idx) = self.enemy_idx(uid) {
                            let e = &mut self.s.enemies[idx];
                            e.alive = false;
                            e.fled = true;
                            e.intent = None;
                            self.ev.push(Event::EnemyFled { enemy: uid });
                        }
                    }
                }
                Effect::ConsumeCurses(t) => {
                    ctx.consumed = 0;
                    for u in self.targets(ctx, *t) {
                        for st in [S::Rot, S::Hex, S::Weak] {
                            ctx.consumed += self.clear_status(u, st);
                        }
                    }
                }
                Effect::Flag(name, n) => {
                    if let UnitRef::Hero(i) = ctx.src {
                        *self.hero(i).flags.entry(name.clone()).or_insert(0) += n;
                    }
                }
                Effect::Once(name, body) | Effect::OncePerTurn(name, body) => {
                    if let UnitRef::Hero(i) = ctx.src {
                        // per-round keys live under `turn_`, which the round start wipes
                        let key = if matches!(e, Effect::OncePerTurn(..)) { format!("turn_{name}") } else { format!("once_{name}") };
                        if !self.s.heroes[i as usize].flags.contains_key(&key) {
                            self.hero(i).flags.insert(key, 1);
                            self.run(body, ctx);
                        }
                    }
                }
                Effect::MaxHp(a, t) => {
                    let n = self.amount(ctx, a);
                    for u in self.targets(ctx, *t) {
                        if let UnitRef::Hero(i) = u {
                            let h = self.hero(i);
                            h.max_hp = (h.max_hp + n).max(1);
                            h.hp = (h.hp + n.max(0)).min(h.max_hp);
                        }
                    }
                }
                Effect::Echo(t) => {
                    let UnitRef::Hero(me) = ctx.src else { continue };
                    // an ally's last card; alone, your own
                    let last = self
                        .s
                        .log
                        .iter()
                        .rev()
                        .find(|p| p.player != me)
                        .or_else(|| self.s.log.iter().rev().find(|p| p.player == me))
                        .map(|p| (p.card.clone(), p.upgraded));
                    let Some((def, upgraded)) = last else { continue };
                    for u in self.targets(ctx, *t) {
                        if let UnitRef::Hero(i) = u {
                            self.s.next_uid += 1;
                            let card = Card { uid: self.s.next_uid, def: def.clone(), upgraded, free: true };
                            let h = self.hero(i);
                            if h.hand.len() < HAND_LIMIT {
                                h.hand.push(card);
                                self.ev.push(Event::CardAdded { player: i, card: def.clone(), pile: Pile::Hand });
                            }
                        }
                    }
                }
                Effect::Herb(herb) => {
                    if let UnitRef::Hero(i) = ctx.src {
                        let h = self.hero(i);
                        let room = if h.flags.contains_key("big_cauldron") { 4 } else { 3 };
                        // a full cauldron boils over: the oldest herb is thrown out, and bites
                        let spilled = (h.cauldron.len() >= room).then(|| h.cauldron.remove(0));
                        h.cauldron.push(herb.clone());
                        if let Some(old) = spilled {
                            self.ev.push(Event::HerbSpilled { player: i, herb: old.clone() });
                            self.spill(&old, ctx);
                        }
                        self.ev.push(Event::HerbAdded { player: i, herb: herb.clone() });
                    }
                }
                Effect::Brew => self.brew(ctx),
                Effect::Provoke => {
                    if let (UnitRef::Hero(me), Some(UnitRef::Enemy(uid))) = (ctx.src, ctx.target) {
                        if let Some(idx) = self.enemy_idx(uid) {
                            if let Some(intent) = &mut self.s.enemies[idx].intent {
                                if intent.target.is_some() {
                                    intent.target = Some(me);
                                    let mv = intent.mv;
                                    self.ev.push(Event::Intent { enemy: uid, mv, target: Some(me) });
                                }
                            }
                        }
                    }
                }
                Effect::Hook(name) => crate::hooks::run(self, ctx, name),
                Effect::Discard(a) => {
                    let n = self.amount(ctx, a);
                    if let UnitRef::Hero(me) = ctx.src {
                        self.discard_cards(me, Some(n));
                    }
                }
                Effect::DiscardHand => {
                    if let UnitRef::Hero(me) = ctx.src {
                        self.discard_cards(me, None);
                    }
                }
                Effect::ExhaustCards(a) => {
                    let n = self.amount(ctx, a);
                    if let UnitRef::Hero(me) = ctx.src {
                        for _ in 0..n.max(0) {
                            let left = self.s.heroes[me as usize].hand.len();
                            if left == 0 {
                                break;
                            }
                            let pos = self.s.rng.below(left as u32) as usize;
                            let card = self.hero(me).hand.remove(pos);
                            self.exhaust_card(me, card);
                        }
                    }
                }
                Effect::Recall(a) => {
                    let n = self.amount(ctx, a);
                    if let UnitRef::Hero(me) = ctx.src {
                        for _ in 0..n.max(0) {
                            let h = &self.s.heroes[me as usize];
                            let can: Vec<usize> = (0..h.discard.len()).filter(|i| self.c.cards.get(&h.discard[*i].def).is_some_and(|d| !matches!(d.kind, CardType::Curse | CardType::Status))).collect();
                            if can.is_empty() || h.hand.len() >= HAND_LIMIT {
                                break;
                            }
                            let i = can[self.s.rng.below(can.len() as u32) as usize];
                            let card = self.hero(me).discard.remove(i);
                            let def = card.def.clone();
                            self.hero(me).hand.push(card);
                            self.ev.push(Event::CardAdded { player: me, card: def, pile: Pile::Hand });
                        }
                    }
                }
            }
        }
    }

    /// What a herb thrown out of a full cauldron does: a weak taste of what it brews into.
    fn spill(&mut self, herb: &str, ctx: &mut Ctx) {
        let effects = match herb {
            "wormwood" => vec![Effect::Damage(Amount::N(2), Tgt::AllFoes)],
            "sleep" => vec![Effect::Block(Amount::N(3), Tgt::Me)],
            "weep" => vec![Effect::Heal(Amount::N(2), Tgt::Me)],
            "burst" => vec![Effect::Damage(Amount::N(5), Tgt::RandomFoe)],
            "fern" => vec![Effect::Draw(Amount::N(1))],
            "nettle" => vec![Effect::Apply(StatusKind::Bleed, Amount::N(1), Tgt::AllFoes)],
            "henbane" => vec![Effect::Apply(StatusKind::Fear, Amount::N(5), Tgt::AllFoes)],
            "valerian" => vec![Effect::Apply(StatusKind::Weak, Amount::N(1), Tgt::RandomFoe)],
            "st_john" => vec![Effect::Apply(StatusKind::TempStrength, Amount::N(1), Tgt::Me)],
            _ => return,
        };
        self.run(&effects, ctx);
    }

    /// Resolves the cauldron: the first recipe whose herbs match is brewed.
    fn brew(&mut self, ctx: &mut Ctx) {
        let UnitRef::Hero(me) = ctx.src else { return };
        let herbs = std::mem::take(&mut self.hero(me).cauldron);
        if herbs.is_empty() {
            return;
        }
        // fern flower stands in for any herb and doubles the brew
        let ferns = herbs.iter().filter(|h| *h == "fern").count();
        let plain: Vec<&Id> = herbs.iter().filter(|h| *h != "fern").collect();
        // the fern first stands in for the herb already in the pot most, so a pair stays a pair
        let mut copy: Vec<&Id> = plain.clone();
        if let Some(most) = plain.iter().max_by_key(|h| plain.iter().filter(|x| x == h).count()) {
            copy.extend(std::iter::repeat_n(*most, ferns));
        }
        let as_copy = |r: &&crate::data::RecipeDef| {
            let mut need: Vec<&Id> = r.herbs.iter().collect();
            let mut have = copy.clone();
            need.sort();
            have.sort();
            need == have
        };
        let found = self.c.recipes.iter().find(|r| ferns > 0 && !plain.is_empty() && as_copy(r)).or_else(|| self.c.recipes.iter().find(|r| {
            if r.herbs.len() != herbs.len() {
                return false;
            }
            let mut have = plain.clone();
            let mut wild = ferns;
            let mut stars = 0;
            for need in &r.herbs {
                if need == "*" {
                    stars += 1;
                } else if let Some(i) = have.iter().position(|h| *h == need) {
                    have.remove(i);
                } else if wild > 0 {
                    wild -= 1;
                } else {
                    return false;
                }
            }
            have.len() + wild == stars
        }));
        let Some(recipe) = found else { return };
        let mut effects = recipe.effects.clone();
        let id = recipe.id.clone();
        // "Brew for all": single targets widen to the whole side
        let wide = self.s.heroes[me as usize].flags.get("brew_all").copied().unwrap_or(0) > 0;
        if wide {
            *self.hero(me).flags.entry("brew_all".into()).or_insert(1) -= 1;
            fn widen(list: &mut [Effect]) {
                let w = |t: &mut Tgt| {
                    *t = match *t {
                        Tgt::Target | Tgt::RandomFoe => Tgt::AllFoes,
                        Tgt::Me => Tgt::AllFriends,
                        other => other,
                    }
                };
                for e in list {
                    match e {
                        Effect::Damage(_, t) | Effect::Hits(_, _, t) | Effect::Block(_, t) | Effect::Apply(_, _, t) | Effect::Heal(_, t) => w(t),
                        Effect::If(_, a, b) => {
                            widen(a);
                            widen(b);
                        }
                        _ => {}
                    }
                }
            }
            widen(&mut effects);
        }
        self.ev.push(Event::Brewed { player: me, recipe: id });
        // brews are potions, not weapon swings
        let was = ctx.attack;
        ctx.attack = false;
        for _ in 0..if ferns > 0 { 2 } else { 1 } {
            self.run(&effects, ctx);
        }
        ctx.attack = was;
    }

    // ---- round flow ----

    fn start_round(&mut self) {
        self.s.round += 1;
        self.s.log.clear();
        self.ev.push(Event::RoundStart(self.s.round));
        // a fight that drags on draws the dark in: each round past the mark it thickens on the heroes
        if self.s.round > self.s.stall {
            for slot in self.standing() {
                self.add_status(UnitRef::Hero(slot), S::Gloom, 1);
            }
        }

        let standing = self.standing();
        for idx in 0..self.s.enemies.len() {
            if !self.s.enemies[idx].alive {
                continue;
            }
            self.s.enemies[idx].flags.retain(|k, _| !k.starts_with("turn_"));
            let Some(d) = self.c.enemies.get(&self.s.enemies[idx].def) else { continue };
            let step = self.s.enemies[idx].step;
            // second phase: once worn down far enough, a boss changes its ways for good
            let (hp, max) = (self.s.enemies[idx].hp, self.s.enemies[idx].max_hp);
            let risen = self.s.enemies[idx].flags.contains_key("risen");
            if !d.phase2.is_empty() && (hp * 100 <= max * d.phase2_at || risen) && !self.s.enemies[idx].flags.contains_key("phase2") {
                self.s.enemies[idx].flags.insert("phase2".into(), step as i32);
                let enemy = self.s.enemies[idx].uid;
                self.ev.push(Event::Phase2 { enemy });
            }
            let mv = match self.s.enemies[idx].flags.get("phase2") {
                Some(since) => d.phase2[(step - *since as usize) % d.phase2.len()],
                None if step < d.opening.len() => d.opening[step],
                None => d.pattern[(step - d.opening.len()) % d.pattern.len()],
            };
            // a hero under Likho's eye draws every aimed blow
            let marked = self.s.heroes.iter().find(|h| !h.downed && h.flags.contains_key("marked")).map(|h| h.slot);
            // Koschei goes for whoever holds his needle, else whoever hurt the chain most
            let hunted = if self.s.enemies[idx].flags.contains_key("hunter") {
                let live = || self.s.heroes.iter().filter(|h| !h.downed);
                let holder = live().find(|h| h.hand.iter().any(|x| x.def == "koschei_needle")).map(|h| h.slot);
                let worst = live().filter(|h| h.flags.get("chain_dmg").copied().unwrap_or(0) > 0).max_by_key(|h| (h.flags["chain_dmg"], std::cmp::Reverse(h.slot))).map(|h| h.slot);
                holder.or(worst)
            } else {
                None
            };
            let target = if d.moves[mv].targeted { marked.or(hunted).or_else(|| self.s.rng.pick(&standing).copied()) } else { None };
            let e = &mut self.s.enemies[idx];
            e.intent = Some(Intent { mv, target });
            let enemy = e.uid;
            self.ev.push(Event::Intent { enemy, mv, target });
        }

        for slot in standing {
            let round = self.s.round;
            let h = self.hero(slot);
            if round > 1 {
                h.block = 0;
            }
            // dragged under, shaken by a quake, or caught in Mara's winter
            let less_energy = h.flags.remove("energy_less").unwrap_or(0);
            // one limb gone, a card less a turn; both, strength less as well
            let maimed = h.lost_limbs as i32;
            let less_energy = less_energy + (maimed >= 2) as i32;
            let less_draw = h.flags.remove("draw_less").unwrap_or(0) + h.flags.get("winter").copied().unwrap_or(0) + (maimed >= 1) as i32;
            h.energy = (BASE_ENERGY - less_energy).max(0);
            h.ready = false;
            h.flags.retain(|k, _| !k.starts_with("turn_"));
            let hungry = h.form == Form::Beast && !h.flags.contains_key("no_hunger");
            if hungry {
                self.lose_hp(UnitRef::Hero(slot), 2);
            }
            if self.s.heroes[slot as usize].downed {
                continue;
            }
            self.draw(slot, (BASE_DRAW - less_draw).max(1));
            self.fire(slot, Trigger::TurnStart);
        }
        self.check_end();
    }

    fn maybe_close(&mut self) {
        if self.s.phase == Phase::Heroes && self.s.standing_heroes().all(|h| h.ready) {
            self.end_round();
        }
    }

    fn end_round(&mut self) {
        // afflictions that last one hero turn lift as it ends
        // nobody pulled them out: the mire takes its due and lets go
        let sunk: Vec<u8> = self.s.heroes.iter().filter(|h| !h.downed && h.flags.contains_key("mired")).map(|h| h.slot).collect();
        for slot in sunk {
            self.lose_hp(UnitRef::Hero(slot), 8);
        }
        for h in &mut self.s.heroes {
            for flag in ["blind", "confused", "coiled", "mired", "asleep", "frozen", "charmed", "no_intents"] {
                h.flags.remove(flag);
            }
        }
        for e in &mut self.s.enemies {
            e.flags.remove("coil_left");
        }
        // end of the heroes' turn, in slot order
        for slot in self.standing() {
            // Yemelya: strength not spent is sleep gained
            let idle = self.s.hero(slot).filter(|h| h.mechanic == Mechanic::Sloth).map_or(0, |h| h.energy.max(0));
            if idle > 0 {
                self.add_status(UnitRef::Hero(slot), StatusKind::Drowse, idle);
            }
            self.fire(slot, Trigger::TurnEnd);
            self.fire_runes(slot);
            if self.s.heroes[slot as usize].downed {
                continue;
            }
            // a pick left unmade when the round closes is let go
            self.hero(slot).choosing = None;
            // the gathering dark takes its toll, through any block
            let gloom = self.s.status(UnitRef::Hero(slot), S::Gloom);
            if gloom > 0 {
                self.lose_hp(UnitRef::Hero(slot), gloom);
            }
            let hand = std::mem::take(&mut self.hero(slot).hand);
            let mut keep = Vec::new();
            for card in hand {
                let Some(def) = self.c.cards.get(&card.def) else { continue };
                let view = def.view(card.upgraded, self.s.heroes[slot as usize].form);
                if !def.in_hand_end.is_empty() {
                    let fx = def.in_hand_end.clone();
                    let mut ctx = Ctx::passive(UnitRef::Hero(slot));
                    self.run(&fx, &mut ctx);
                }
                let h = self.hero(slot);
                if view.has(Keyword::Ethereal) {
                    self.exhaust_card(slot, card);
                } else if view.has(Keyword::Retain) && !h.downed {
                    keep.push(card);
                } else {
                    h.discard.push(card);
                }
            }
            self.hero(slot).hand = keep;
            self.rot_tick(UnitRef::Hero(slot));
        }
        // Morovitsa Doom countdown
        for slot in self.standing() {
            let triggered = if let Some(left) = self.s.heroes[slot as usize].flags.get_mut("doom") {
                *left -= 1;
                *left <= 0
            } else {
                false
            };
            if triggered {
                self.s.heroes[slot as usize].flags.remove("doom");
                self.lose_hp(UnitRef::Hero(slot), 45);
            }
        }
        // Gorynych Head Regrowth
        let any_head_alive = self.s.enemies.iter().any(|e| e.alive && e.flags.contains_key("gorynych_head"));
        if any_head_alive {
            for e in &mut self.s.enemies {
                if !e.alive && e.flags.contains_key("gorynych_head") {
                    let turns = e.flags.entry("gorynych_regrowth".into()).or_insert(2);
                    *turns -= 1;
                    if *turns <= 0 {
                        e.alive = true;
                        e.hp = 35;
                        e.flags.remove("gorynych_regrowth");
                    }
                }
            }
        }
        self.check_end();
        if self.s.phase != Phase::Heroes {
            return;
        }

        let units: Vec<UnitRef> = (0..self.s.heroes.len() as u8)
            .map(UnitRef::Hero)
            .chain(self.s.enemies.iter().map(|e| UnitRef::Enemy(e.uid)))
            .collect();
        let mut pre_timed: std::collections::BTreeMap<(UnitRef, StatusKind), i32> = std::collections::BTreeMap::new();
        for &u in &units {
            for &st in &[StatusKind::Vulnerable, StatusKind::Weak, StatusKind::Chained] {
                let cur = self.s.status(u, st);
                if cur > 0 {
                    pre_timed.insert((u, st), cur);
                }
            }
        }

        self.ev.push(Event::EnemyPhase);
        let order: Vec<u32> = self.s.enemies.iter().map(|e| e.uid).collect();
        for uid in order {
            let Some(idx) = self.enemy_idx(uid) else { continue };
            if !self.s.enemies[idx].alive {
                continue;
            }
            self.s.enemies[idx].block = 0;
            // bones lying since the heroes' turn come together instead of striking
            if self.s.enemies[idx].flags.remove(crate::state::BONES).is_some() {
                let e = &mut self.s.enemies[idx];
                e.hp = (e.max_hp / 2).max(1);
                e.intent = None;
                let (unit, hp) = (UnitRef::Enemy(e.uid), e.hp);
                self.ev.push(Event::Heal { unit, amount: hp, hp });
                continue;
            }
            let Some(intent) = self.s.enemies[idx].intent.clone() else { continue };
            self.s.enemies[idx].step += 1;
            let me = UnitRef::Enemy(uid);
            let Some(mv) = self.c.enemies.get(&self.s.enemies[idx].def).and_then(|d| d.moves.get(intent.mv)).cloned() else {
                continue;
            };
            let attacks = matches!(mv.intent, IntentKind::Attack | IntentKind::AttackAll);
            let fear = self.s.status(me, S::Fear);
            if self.s.status(me, S::Stun) > 0 {
                self.add_status(me, S::Stun, -1);
                self.ev.push(Event::EnemySkipped { enemy: uid, fear: false });
            } else if attacks && fear > 0 && (self.s.rng.below(1000) as i32) < fear {
                self.clear_status(me, S::Fear);
                self.ev.push(Event::EnemySkipped { enemy: uid, fear: true });
            } else {
                let mut target = intent.target;
                if target.is_some_and(|t| self.s.heroes[t as usize].downed) {
                    let standing = self.standing();
                    target = self.s.rng.pick(&standing).copied();
                }
                self.ev.push(Event::EnemyMove { enemy: uid, mv: intent.mv });
                // Switcheroo: the blow meant for a hero lands on another enemy
                let mut aim = target.map(UnitRef::Hero);
                if let Some(redirect) = self.s.enemies[idx].flags.remove("misdirect") {
                    if aim.is_some() {
                        let others: Vec<u32> = self.living().into_iter().filter(|u| *u != uid).collect();
                        if !others.is_empty() {
                            if others.contains(&(redirect as u32)) {
                                aim = Some(UnitRef::Enemy(redirect as u32));
                            } else {
                                aim = self.s.rng.pick(&others).copied().map(UnitRef::Enemy);
                            }
                        }
                    }
                }
                let mut ctx = Ctx {
                    src: me,
                    target: aim,
                    x: 0,
                    consumed: 0,
                    fatal: false,
                    attack: true,
                    card: false,
                    bled: false,
                };
                self.run(&mv.effects, &mut ctx);
            }
            self.rot_tick(me);
            if self.s.standing_heroes().next().is_none() {
                break;
            }
        }

        for u in units {
            let all: Vec<(StatusKind, i32)> = match u {
                UnitRef::Hero(i) => self.s.heroes[i as usize].statuses.iter().map(|(k, v)| (*k, *v)).collect(),
                UnitRef::Enemy(i) => self.s.enemy(i).map(|e| e.statuses.iter().map(|(k, v)| (*k, *v)).collect()).unwrap_or_default(),
            };
            for (st, n) in all {
                if st.timed() {
                    let eligible = pre_timed.get(&(u, st)).copied().unwrap_or(0);
                    if eligible > 0 {
                        self.add_status(u, st, -1);
                    }
                } else if st.round_only() {
                    self.add_status(u, st, -n);
                }
            }
        }
        for h in &mut self.s.heroes {
            h.guard_by = None;
        }
        self.check_end();
        if self.s.phase == Phase::Heroes {
            self.start_round();
        }
    }

    /// Rot: X damage, then one stack fewer.
    fn rot_tick(&mut self, u: UnitRef) {
        let rot = self.s.status(u, S::Rot);
        if rot > 0 && self.exists(u) {
            // Pestilence: Rot on enemies bites twice and does not fade
            let mor = matches!(u, UnitRef::Enemy(_)) && self.s.heroes.iter().any(|h| !h.downed && h.flags.contains_key("mor"));
            self.hurt(None, u, rot, false);
            if mor {
                self.hurt(None, u, rot, false);
            } else {
                self.add_status(u, S::Rot, -1);
            }
        }
    }

    fn check_end(&mut self) {
        if self.s.phase != Phase::Heroes {
            return;
        }
        if self.s.standing_heroes().next().is_none() {
            self.s.phase = Phase::Lost;
            self.ev.push(Event::Ended { won: false });
        } else if self.s.enemies.iter().all(|e| !e.alive) {
            self.s.phase = Phase::Won;
            let has_curse = self.c.cards.contains_key(NAV_MARK);
            for h in &mut self.s.heroes {
                if h.downed && !h.beheaded {
                    // still on the edge when the fight ends: up at 1 HP, marked
                    h.downed = false;
                    h.hp = 1;
                    if has_curse && h.flags.remove("marked_fall").is_none() {
                        h.new_curses += 1;
                    }
                }
            }
            self.ev.push(Event::Ended { won: true });
        }
    }
}
