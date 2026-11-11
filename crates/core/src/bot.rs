//! Autopilot for disconnected players and the balance simulator:
//! block when something is swinging at you, otherwise hit.

use crate::data::*;
use crate::state::*;

/// Damage one enemy's shown intent will deal per hit, and how many hits, before the target's block.
pub fn intent_damage(c: &Content, s: &Combat, e: &Enemy) -> Option<(i32, i32)> {
    let intent = e.intent.as_ref()?;
    let mv = c.enemies.get(&e.def)?.moves.get(intent.mv)?;
    let me = UnitRef::Enemy(e.uid);
    mv.effects.iter().find_map(|fx| {
        let (a, hits) = match fx {
            Effect::Damage(a, _) => (a, 1),
            Effect::Hits(a, k, _) => (a, *k),
            _ => return None,
        };
        let base = match a {
            Amount::N(base) => *base,
            Amount::Pack(base, each) => base + (s.living_enemies().count() as i32 - 1) * each,
            _ => return None,
        };
        let mut d = base + s.status(me, StatusKind::Strength) + s.status(me, StatusKind::TempStrength);
        d = d * s.enemy_dmg / 100;
        if s.status(me, StatusKind::Weak) > 0 {
            d = d * 3 / 4;
        }
        let target_hero = intent.target.or_else(|| if s.heroes.len() == 1 || mv.intent == IntentKind::AttackAll { Some(0) } else { None });
        if let Some(slot) = target_hero {
            let u = UnitRef::Hero(slot);
            if s.status(u, StatusKind::Vulnerable) > 0 {
                d = d * 3 / 2;
            }
            if s.status(u, StatusKind::Mist) > 0 {
                d /= 2;
            }
        }
        Some((d.max(0), hits))
    })
}

/// Total damage aimed at a hero this round.
pub fn incoming(c: &Content, s: &Combat, slot: u8) -> i32 {
    s.living_enemies()
        .filter_map(|e| {
            let intent = e.intent.as_ref()?;
            let mv = c.enemies.get(&e.def)?.moves.get(intent.mv)?;
            let hits_me = intent.target == Some(slot) || (intent.target.is_none() && mv.intent == IntentKind::AttackAll);
            if !hits_me {
                return None;
            }
            let me = UnitRef::Enemy(e.uid);
            let u = UnitRef::Hero(slot);
            mv.effects.iter().find_map(|fx| {
                let (a, hits) = match fx {
                    Effect::Damage(a, _) => (a, 1),
                    Effect::Hits(a, k, _) => (a, *k),
                    _ => return None,
                };
                let base = match a {
                    Amount::N(base) => *base,
                    Amount::Pack(base, each) => base + (s.living_enemies().count() as i32 - 1) * each,
                    _ => return None,
                };
                let mut d = base + s.status(me, StatusKind::Strength) + s.status(me, StatusKind::TempStrength);
                d = d * s.enemy_dmg / 100;
                if s.status(me, StatusKind::Weak) > 0 {
                    d = d * 3 / 4;
                }
                if s.status(u, StatusKind::Vulnerable) > 0 {
                    d = d * 3 / 2;
                }
                if s.status(u, StatusKind::Mist) > 0 {
                    d /= 2;
                }
                Some(d.max(0) * hits)
            })
        })
        .sum()
}

fn blocks_self(view: &CardView) -> bool {
    view.effects.iter().any(|e| matches!(e, Effect::Block(_, Tgt::Me | Tgt::AllFriends)))
}

pub fn choose(c: &Content, s: &Combat, slot: u8) -> CombatAction {
    let Some(h) = s.hero(slot) else { return CombatAction::Ready(true) };
    // a discard to pick for: throw a card that pays for being thrown, or the least use
    if h.choosing.is_some() {
        let worth = |card: &Card| -> i32 {
            let Some(def) = c.cards.get(&card.def) else { return -100 };
            let v = def.view(card.upgraded, h.form);
            if !v.on_discard.is_empty() {
                -50
            } else if matches!(v.kind, CardType::Curse | CardType::Status) || v.has(Keyword::Unplayable) {
                -40
            } else {
                v.cost * 3 + v.effects.len() as i32 + if card.upgraded { 2 } else { 0 }
            }
        };
        if let Some(card) = h.hand.iter().min_by_key(|card| worth(card)) {
            return CombatAction::Discard { card: card.uid };
        }
    }
    // sunk in the mire or asleep: nothing to do but wait
    if h.flags.contains_key("mired") || h.flags.contains_key("asleep") {
        return CombatAction::Ready(true);
    }
    let frozen = h.flags.get("frozen").copied().unwrap_or(0) as usize;
    let playable: Vec<(&Card, CardView)> = h
        .hand
        .iter()
        .enumerate()
        .filter(|(i, _)| *i >= frozen)
        .filter_map(|(_, card)| {
            let v = c.cards.get(&card.def)?.view(card.upgraded, h.form);
            let cost = if v.x_cost { 1 } else { v.cost };
            // paying in blood is for players who can count; the autopilot never does
            let bleeds = v.effects.iter().any(|e| matches!(e, Effect::LoseHp(_, Tgt::Me)));
            // wrapped by the serpent, attacks are refused: don't offer them
            let affordable = !bleeds && !(v.kind == CardType::Attack && h.flags.contains_key("coiled")) && !crate::combat::needs_lost_arm(&v, h.lost_arms, h.def.as_str(), h.form);
            (!v.has(Keyword::Unplayable) && cost <= h.energy && affordable && !matches!(v.kind, CardType::Curse | CardType::Status)).then_some((card, v))
        })
        .collect();

    // a fallen friend comes first: the run ends when everyone is down
    if let Some(ally) = s.heroes.iter().find(|a| a.downed) {
        if let Some((card, _)) = playable.iter().find(|(_, v)| v.target == CardTarget::Downed) {
            return CombatAction::Play { card: card.uid, target: Some(UnitRef::Hero(ally.slot)) };
        }
        if h.energy >= s.raise_cost {
            if let Some(card) = h.hand.first() {
                return CombatAction::Raise { ally: ally.slot, discard: card.uid };
            }
        }
    }

    // no point striking what cannot die, or what a single blow cannot reach
    let others_out = s.living_enemies().any(|e| !e.flags.contains_key("hiding"));
    let reachable = |e: &&Enemy| !e.flags.contains_key("immortal") && !e.flags.contains_key("flying") && !(others_out && e.flags.contains_key("hiding"));
    let weakest = s
        .living_enemies()
        .filter(reachable)
        .min_by_key(|e| e.hp + e.block)
        .or_else(|| s.living_enemies().filter(|e| !e.flags.contains_key("immortal") && !(others_out && e.flags.contains_key("hiding"))).min_by_key(|e| e.hp + e.block))
        .or_else(|| s.living_enemies().filter(|e| !(others_out && e.flags.contains_key("hiding"))).min_by_key(|e| e.hp + e.block))
        .map(|e| UnitRef::Enemy(e.uid));
    // a draught is for a bad hour: when half the blood is gone, or the foe is a big one
    let hard_fight = s.enemies.iter().map(|e| e.max_hp).sum::<i32>() >= 100 * s.heroes.len() as i32;
    if h.hp * 2 <= h.max_hp || hard_fight {
        for (i, id) in h.potions.iter().enumerate() {
            let Some(def) = c.potions.get(id) else { continue };
            let target = match def.target {
                CardTarget::None => Some(None),
                CardTarget::Enemy => weakest.map(Some),
                CardTarget::Ally | CardTarget::AnyHero => Some(None),
                CardTarget::Downed => s.heroes.iter().find(|a| a.downed).map(|a| Some(UnitRef::Hero(a.slot))),
            };
            if let Some(target) = target {
                return CombatAction::UsePotion { slot: i as u8, target };
            }
        }
    }
    // the needle ends Koschei's deathlessness: nothing matters more
    if let Some((card, _)) = playable.iter().find(|(c, _)| c.def == "koschei_needle") {
        return CombatAction::Play { card: card.uid, target: None };
    }
    // something on the wing: sweeping blows first
    if s.living_enemies().any(|e| e.flags.contains_key("flying")) {
        let sweep = playable.iter().find(|(_, v)| v.target == CardTarget::None && v.effects.iter().any(|e| matches!(e, Effect::Damage(_, Tgt::AllFoes) | Effect::Hits(_, _, Tgt::AllFoes))));
        if let Some((card, _)) = sweep {
            return CombatAction::Play { card: card.uid, target: None };
        }
    }
    let other = s.standing_heroes().find(|a| a.slot != slot).map(|a| UnitRef::Hero(a.slot));
    let target_for = |v: &CardView| -> Option<Option<UnitRef>> {
        match v.target {
            CardTarget::None => Some(None),
            CardTarget::Enemy => weakest.map(Some),
            CardTarget::Ally => Some(other),
            CardTarget::AnyHero => Some(Some(other.unwrap_or(UnitRef::Hero(slot)))),
            CardTarget::Downed => None,
        }
    };

    // a shield for every scratch loses the race: small blows are taken on the chin while
    // there is blood to spare, and only a real wound, or a thin thread of life, is worth guarding
    let wound = incoming(c, s, slot) - h.block;
    let threatened = wound > 0 && (wound * 5 >= h.hp || h.hp * 2 <= h.max_hp);
    let pick = |want: &dyn Fn(&CardView) -> bool| {
        playable.iter().filter(|(_, v)| want(v)).find_map(|(card, v)| {
            target_for(v).map(|target| CombatAction::Play { card: card.uid, target })
        })
    };
    if threatened {
        if let Some(a) = pick(&blocks_self) {
            return a;
        }
    }
    if let Some(a) = pick(&|v| v.kind == CardType::Power) {
        return a;
    }
    if let Some(a) = pick(&|v| v.kind == CardType::Attack) {
        return a;
    }
    if let Some(a) = pick(&|v| !blocks_self(v) || threatened) {
        return a;
    }
    if let Some(a) = pick(&|_| true) {
        return a;
    }
    CombatAction::Ready(true)
}
