//! Named one-off mechanics that don't fit the effect primitives.

use crate::combat::{Ctx, Exec};
use crate::data::*;
use crate::state::*;

/// A hero's card as the shadow plays it back: blows and curses land on the hero, shields and
/// strength go to the shadow. Whatever only a hero can do (drawing, runes, masks) is lost in the mirror.
fn mirror(effects: &[Effect]) -> Vec<Effect> {
    // numbers that depend on the hero's own state have nothing to read in a shadow
    let flat = |a: &Amount| match a {
        Amount::N(n) => Amount::N(*n),
        _ => Amount::N(6),
    };
    let mut out = Vec::new();
    for e in effects {
        match e {
            Effect::Damage(a, _) => out.push(Effect::Damage(flat(a), Tgt::Target)),
            Effect::Hits(a, k, _) => out.push(Effect::Hits(flat(a), *k, Tgt::Target)),
            Effect::Block(a, _) | Effect::Guard(a, _) => out.push(Effect::Block(flat(a), Tgt::Me)),
            Effect::Heal(a, _) => out.push(Effect::Heal(flat(a), Tgt::Me)),
            Effect::Apply(st @ (StatusKind::Vulnerable | StatusKind::Weak), a, Tgt::Target | Tgt::AllFoes | Tgt::RandomFoe) => out.push(Effect::Apply(*st, flat(a), Tgt::Target)),
            Effect::Apply(st @ (StatusKind::Strength | StatusKind::Thorns), a, Tgt::Me | Tgt::AllFriends) => out.push(Effect::Apply(*st, flat(a), Tgt::Me)),
            Effect::If(_, then, _) => out.extend(mirror(then)),
            Effect::Repeat(_, body) | Effect::Once(_, body) | Effect::OncePerTurn(_, body) => out.extend(mirror(body)),
            _ => {}
        }
    }
    out
}

/// Mara's turns without her crown before she forges a new one.
const CROWN_BACK_AFTER: i32 = 3;

pub(crate) fn run(ex: &mut Exec, ctx: &mut Ctx, name: &str) {
    match name {
        // Vorozheya's "Transfer": every debuff on the heroes moves onto the targeted enemy
        "transfer" => {
            let Some(target) = ctx.target else { return };
            let heroes: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
            for slot in heroes {
                let debuffs: Vec<(StatusKind, i32)> =
                    ex.s.heroes[slot as usize].statuses.iter().filter(|(k, _)| k.debuff()).map(|(k, v)| (*k, *v)).collect();
                for (st, n) in debuffs {
                    ex.add_status(UnitRef::Hero(slot), st, -n);
                    ex.add_status(target, st, n);
                }
            }
        }
        // Volkhv's "Wheel of Kolyada": X times, one random rune on every hero fires
        "kolyada" => {
            for _ in 0..ctx.x {
                let heroes: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
                for slot in heroes {
                    let runes = ex.s.heroes[slot as usize].runes.clone();
                    if let Some(rune) = ex.s.rng.pick(&runes).copied() {
                        ex.fire_rune(slot, rune);
                    }
                }
            }
        }
        // Skomorokh's "Fairground": enemies swap who they are aiming at
        "balagan" => {
            let idx: Vec<usize> =
                ex.s.enemies.iter().enumerate().filter(|(_, e)| e.alive && e.intent.as_ref().is_some_and(|i| i.target.is_some())).map(|(i, _)| i).collect();
            let mut targets: Vec<Option<u8>> = idx.iter().map(|i| ex.s.enemies[*i].intent.as_ref().and_then(|x| x.target)).collect();
            ex.s.rng.shuffle(&mut targets);
            for (i, t) in idx.into_iter().zip(targets) {
                let e = &mut ex.s.enemies[i];
                if let Some(intent) = &mut e.intent {
                    intent.target = t;
                    let (enemy, mv) = (e.uid, intent.mv);
                    ex.ev.push(Event::Intent { enemy, mv, target: t });
                }
            }
        }
        // Skomorokh's "Switcheroo": the targeted enemy's next blow hits another enemy
        "misdirect" => {
            if let Some(UnitRef::Enemy(uid)) = ctx.target {
                let others: Vec<u32> = ex.living().into_iter().filter(|u| *u != uid).collect();
                if let Some(&aim_enemy) = ex.s.rng.pick(&others) {
                    if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                        e.flags.insert("misdirect".into(), aim_enemy as i32);
                    }
                }
            }
        }
        // Znakharka's "Herbalist": a random common herb comes to hand
        "random_herb" => {
            if let UnitRef::Hero(me) = ctx.src {
                let herbs = ["herb_wormwood", "herb_sleep", "herb_weep"];
                let pick = herbs[ex.s.rng.below(herbs.len() as u32) as usize];
                if ex.c.cards.contains_key(pick) {
                    ex.add_card(me, pick, Pile::Hand);
                }
            }
        }
        // Koschei, every turn: he knits back together while deathless; once the needle is
        // broken he drains the weakest hero and his last breath counts down to the party's end
        "koschei_turn" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            let Some(idx) = ex.s.enemies.iter().position(|e| e.uid == uid) else { return };
            if ex.s.enemies[idx].flags.contains_key("immortal") {
                let heal = ex.s.enemies[idx].max_hp / 10;
                ex.heal(ctx.src, heal);
                // his patience wears thin: every second turn he hits harder, so a party
                // that only shelters behind its shields cannot wait him out forever
                let wrath = ex.s.enemies[idx].flags.get("wrath").copied().unwrap_or(0) + 1;
                ex.s.enemies[idx].flags.insert("wrath".into(), wrath);
                if wrath % 2 == 0 {
                    ex.add_status(ctx.src, StatusKind::Strength, 1);
                }
                return;
            }
            let weakest = ex.s.standing_heroes().min_by_key(|h| h.hp).map(|h| h.slot);
            if let Some(slot) = weakest {
                ex.hurt(Some(ctx.src), UnitRef::Hero(slot), 10, false);
                ex.heal(ctx.src, 10);
            }
            let left = ex.s.enemies[idx].flags.get("last_breath").copied().unwrap_or(0) - 1;
            ex.s.enemies[idx].flags.insert("last_breath".into(), left);
            if left <= 0 {
                let all: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
                for slot in all {
                    let hp = ex.s.heroes[slot as usize].hp + ex.s.heroes[slot as usize].block;
                    ex.hurt(Some(ctx.src), UnitRef::Hero(slot), hp, false);
                }
            }
        }
        // the egg is cracked: the needle falls into a random hero's hand and Koschei rises
        "drop_needle" => {
            let heroes: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
            if let Some(slot) = ex.s.rng.pick(&heroes).copied() {
                ex.add_card(slot, "koschei_needle", Pile::Hand);
            }
            for e in ex.s.enemies.iter_mut().filter(|e| e.alive && e.flags.contains_key("immortal")) {
                e.flags.insert("risen".into(), 1);
            }
        }
        // the needle snaps: Koschei is mortal, at two fifths of his life, and breathing his last
        "break_needle" => {
            for e in ex.s.enemies.iter_mut().filter(|e| e.alive && e.flags.contains_key("immortal")) {
                e.flags.remove("immortal");
                e.flags.insert("last_breath".into(), 5);
                e.hp = (e.max_hp * 2 / 5).max(1);
            }
            // the tenth Circle: his shadow tears loose and fights on beside him
            if ex.s.shadow {
                let def: Id = "koschei_shadow".into();
                if let Some(uid) = ex.spawn(&def) {
                    ex.ev.push(Event::Summoned { enemy: uid, def });
                }
            }
        }
        // Koschei's shadow turns each hero's last card of the round against that same hero;
        // whoever played nothing is simply struck
        "shadow_mimic" => {
            for slot in ex.standing() {
                let last = ex.s.log.iter().rev().find(|p| p.player == slot).map(|p| (p.card.clone(), p.upgraded));
                let mirrored = last
                    .and_then(|(card, upgraded)| ex.c.cards.get(&card).map(|d| mirror(&d.view(upgraded, Form::Human).effects)))
                    .filter(|fx| !fx.is_empty())
                    .unwrap_or_else(|| vec![Effect::Damage(Amount::N(8), Tgt::Target)]);
                if ex.s.heroes[slot as usize].downed {
                    continue;
                }
                let mut turned = Ctx { src: ctx.src, target: Some(UnitRef::Hero(slot)), x: 0, consumed: 0, fatal: false, attack: true, card: false, bled: false };
                ex.run(&mirrored, &mut turned);
            }
        }
        // the duck falls and the egg drops out of it; a lone hero faces a thinner shell
        "hatch_egg" => {
            let def: Id = if ex.s.heroes.len() == 1 { "death_egg_solo".into() } else { "death_egg".into() };
            if let Some(uid) = ex.spawn(&def) {
                ex.ev.push(Event::Summoned { enemy: uid, def });
            }
        }
        // Koschei leaves his throne and calls his guard: two of them, one against a lone hero
        "koschei_rise" => {
            let n = if ex.s.heroes.len() == 1 { 1 } else { 2 };
            let def: Id = "koschei_guard".into();
            for _ in 0..n {
                if let Some(uid) = ex.spawn(&def) {
                    ex.ev.push(Event::Summoned { enemy: uid, def: def.clone() });
                }
            }
        }
        // the duck tires and settles for a round, then takes wing again
        "duck_land" | "duck_fly" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                if name == "duck_land" {
                    e.flags.remove("flying");
                } else {
                    e.flags.insert("flying".into(), 1);
                }
            }
        }
        // Likho's single eye settles on a hero: every aimed blow now goes to them,
        // and for one turn they play their cards unseen
        "likho_mark" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            for h in ex.s.heroes.iter_mut() {
                h.flags.remove("marked");
            }
            let h = &mut ex.s.heroes[slot as usize];
            h.flags.insert("marked".into(), 1);
            h.flags.insert("blind".into(), 1);
        }
        // the Poloz wraps a hero: no attacks from them until the serpent takes 15 damage,
        // or their next turn ends
        "poloz_coil" => {
            let (UnitRef::Enemy(uid), Some(UnitRef::Hero(slot))) = (ctx.src, ctx.target) else { return };
            ex.s.heroes[slot as usize].flags.insert("coiled".into(), uid as i32);
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                e.flags.insert("coil_left".into(), 15);
            }
        }
        // the Leshy tangles the paths: next turn every hero's aimed card finds a random enemy
        "tangle_paths" => {
            for h in ex.s.heroes.iter_mut().filter(|h| !h.downed) {
                h.flags.insert("confused".into(), 1);
            }
        }
        // one-turn afflictions an enemy lays on the hero it aimed at
        "drag_under" | "charm" | "sleep" | "auka_lure" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            let flag = match name {
                "drag_under" => "energy_less",
                "charm" => "charmed",
                "auka_lure" => "confused",
                _ => "asleep",
            };
            ex.s.heroes[slot as usize].flags.insert(flag.into(), 1);
        }
        // ...and on the whole party
        "quake" | "sirin_song" | "freeze" => {
            let (flag, n) = match name {
                "quake" => ("draw_less", 2),
                "sirin_song" => ("no_intents", 1),
                _ => ("frozen", 2),
            };
            for h in ex.s.heroes.iter_mut().filter(|h| !h.downed) {
                h.flags.insert(flag.into(), n);
            }
        }
        // the Bolotnik drags heroes into the mire: two of them, but never the whole party
        "mire_grab" => {
            let mut standing: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
            ex.s.rng.shuffle(&mut standing);
            let take = if standing.len() <= 1 { standing.len() } else { 2.min(standing.len() - 1) };
            for slot in standing.into_iter().take(take) {
                ex.s.heroes[slot as usize].flags.insert("mired".into(), 1);
            }
        }
        // the vurdalak elder calls a fallen servant back at half strength
        "raise_dead" => {
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| !e.alive && !e.fled) {
                e.alive = true;
                e.hp = (e.max_hp / 2).max(1);
                e.block = 0;
                e.statuses.clear();
                let (unit, hp, def) = (UnitRef::Enemy(e.uid), e.hp, e.def.clone());
                // seen getting up (the screen raises a fallen foe on this), then its health
                if let UnitRef::Enemy(enemy) = unit {
                    ex.ev.push(Event::Summoned { enemy, def });
                }
                ex.ev.push(Event::Heal { unit, amount: hp, hp });
            } else if ex.s.enemies.iter().filter(|e| e.alive).count() < 3 {
                // no one has fallen yet: he digs a fresh one out of the graveyard
                if let Some(uid) = ex.spawn(&"upyr".to_string()) {
                    ex.ev.push(Event::Summoned { enemy: uid, def: "upyr".into() });
                }
            }
        }
        // a card in the hand of the hero aimed at spoils: gone for this fight, a web in its place
        "spoil_card" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            let n = ex.s.heroes[slot as usize].hand.len() as u32;
            if n > 0 {
                let i = ex.s.rng.below(n) as usize;
                ex.s.heroes[slot as usize].hand.remove(i);
                ex.s.next_uid += 1;
                let uid = ex.s.next_uid;
                ex.s.heroes[slot as usize].hand.push(Card { uid, def: "web".into(), upgraded: false, free: false });
            }
        }
        // an anchutka runs off with a card out of the hand of the hero it aimed at
        "anchutka_snatch" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            let n = ex.s.heroes[slot as usize].hand.len() as u32;
            if n > 0 {
                let i = ex.s.rng.below(n) as usize;
                let h = &mut ex.s.heroes[slot as usize];
                let card = h.hand.remove(i);
                h.discard.push(card);
            }
        }
        // the witch's curses on one foe are laid on all the others as well
        "spread_curse" => {
            let Some(target) = ctx.target else { return };
            let kinds = [StatusKind::Rot, StatusKind::Hex, StatusKind::Weak];
            let have: Vec<(StatusKind, i32)> = kinds.iter().map(|k| (*k, ex.s.status(target, *k))).filter(|(_, n)| *n > 0).collect();
            let others: Vec<UnitRef> = ex.s.living_enemies().map(|e| UnitRef::Enemy(e.uid)).filter(|u| *u != target).collect();
            for u in others {
                for (k, n) in &have {
                    ex.add_status(u, *k, *n);
                }
            }
        }
        // every rune on the volkhv goes off at once, and they stay carved
        "fire_all_runes" => {
            let UnitRef::Hero(me) = ctx.src else { return };
            let runes = ex.s.heroes[me as usize].runes.clone();
            for r in runes {
                ex.fire_rune(me, r);
            }
        }
        // the cauldron thrown over the party: a shield for each herb, and no brew
        "spill_cauldron" => {
            let UnitRef::Hero(me) = ctx.src else { return };
            let herbs = std::mem::take(&mut ex.s.heroes[me as usize].cauldron).len() as i32;
            let all: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
            for slot in all {
                ex.gain_block(UnitRef::Hero(slot), herbs * 3);
            }
        }
        // the jester walks off with a foe's shield
        "steal_block" => {
            let Some(UnitRef::Enemy(uid)) = ctx.target else { return };
            let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) else { return };
            let n = std::mem::take(&mut e.block);
            ex.gain_block(ctx.src, n);
        }
        // the fever moves on to the next hero
        "pass_fever" => {
            let UnitRef::Hero(me) = ctx.src else { return };
            let standing: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
            let next = standing.iter().copied().find(|s| *s > me).or_else(|| standing.first().copied());
            if let Some(next) = next {
                ex.add_card(next, "fever", Pile::Draw);
            }
        }
        // Mara's winter deepens while her crown stands: one card fewer each turn, three at most
        // and once it is broken she forges it anew: on her third turn without it, the crown is back
        "winter" => {
            if ex.s.enemies.iter().any(|e| e.alive && e.def == "ice_crown") {
                for h in ex.s.heroes.iter_mut().filter(|h| !h.downed) {
                    let w = h.flags.entry("winter".into()).or_insert(0);
                    *w = (*w + 1).min(3);
                }
            } else if let UnitRef::Enemy(uid) = ctx.src {
                let Some(idx) = ex.s.enemies.iter().position(|e| e.uid == uid) else { return };
                let gone = ex.s.enemies[idx].flags.entry("crown_gone".into()).or_insert(0);
                *gone += 1;
                if *gone >= CROWN_BACK_AFTER {
                    ex.s.enemies[idx].flags.remove("crown_gone");
                    if let Some(crown) = ex.spawn(&"ice_crown".into()) {
                        ex.ev.push(Event::Summoned { enemy: crown, def: "ice_crown".into() });
                    }
                }
            }
        }
        "thaw" => {
            for h in ex.s.heroes.iter_mut() {
                h.flags.remove("winter");
            }
        }
        // Kot Bayun leaps down off his post to pounce, and is in reach of a plain blow until he
        // climbs back up to purr
        "bayun_leap" | "bayun_climb" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                if name == "bayun_leap" {
                    e.flags.remove("bayun_perch");
                } else {
                    e.flags.insert("bayun_perch".into(), 1);
                }
            }
        }
        // Kot Bayun: Heavy Eyelids into hand of standing heroes
        "bayun_eyelids" => {
            let heroes: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
            for slot in heroes {
                ex.add_card(slot, "heavy_eyelids", Pile::Hand);
                ex.add_card(slot, "heavy_eyelids", Pile::Hand);
            }
        }
        // Kot Bayun: claws that bite harder for every status or curse lying in the discard pile.
        // The hand is always thrown away before the enemies move, so it is the discard (where his
        // Heavy Eyelids go when they are not paid off) that he counts. A blow like any other:
        // strength, weakness and vulnerability count, and the intent shows how much
        "bayun_claws" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            let Some(base) = hook_damage(&ex.c, &ex.s, ctx.src, "bayun_claws", Some(slot)) else { return };
            ex.strike(Some(ctx.src), UnitRef::Hero(slot), base, true);
        }
        // Mokhovik: Kinetic Burst unleashing stored kinetic charge
        "kinetic_burst" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            let Some(base) = hook_damage(&ex.c, &ex.s, ctx.src, "kinetic_burst", Some(slot)) else { return };
            ex.strike(Some(ctx.src), UnitRef::Hero(slot), base, true);
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                e.flags.remove("kinetic_charge");
            }
        }
        // Baba Yaga: Steal potion or inflict decay
        "yaga_steal_potion" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            if !ex.s.heroes[slot as usize].potions.is_empty() {
                ex.s.heroes[slot as usize].potions.pop();
            } else {
                ex.add_status(UnitRef::Hero(slot), StatusKind::Rot, 3);
            }
        }
        // Polunochnitsa: Nightmare Affliction
        "nightmare_afflict" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            ex.add_card(slot, "dread", Pile::Draw);
            ex.strike(Some(ctx.src), UnitRef::Hero(slot), 6, false);
        }
        // Zlyden: Poverty curse (energy less next turn)
        "poverty_curse" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            ex.s.heroes[slot as usize].flags.insert("energy_less".into(), 1);
        }
        // Morovitsa: Doom Clock, counting down from 4. Gazed at again, a doomed hero's count is
        // not wound back up: the gaze comes round every third turn, and would otherwise never let
        // the count run out
        "doom_clock" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            ex.s.heroes[slot as usize].flags.entry("doom".into()).or_insert(DOOM_TURNS);
        }
        // The enemies move after the heroes' hands are thrown away, so a move that "blows cards out
        // of the hand" takes them from the next one instead: so many cards fewer drawn next turn
        // Gagana: Storm screech
        "storm_screech" => {
            for slot in ex.standing() {
                *ex.s.heroes[slot as usize].flags.entry("draw_less".into()).or_insert(0) += 2;
            }
        }
        // Gorynych: Roar
        "roar_discard" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            *ex.s.heroes[slot as usize].flags.entry("draw_less".into()).or_insert(0) += 1;
        }
        // Solovey: Deafening whistle
        "solovey_whistle" => {
            for slot in ex.standing() {
                *ex.s.heroes[slot as usize].flags.entry("draw_less".into()).or_insert(0) += 1;
            }
        }
        // Korochun: Bitter freeze
        "korochun_chill" => {
            for slot in ex.standing() {
                ex.s.heroes[slot as usize].flags.insert("frozen".into(), 1);
            }
        }
        // Vodyanoy Tsar: Whirlpool drag
        "tsar_whirlpool" => {
            for slot in ex.standing() {
                ex.s.heroes[slot as usize].flags.insert("energy_less".into(), 1);
            }
        }
        // Baba Yaga: Cauldron brew
        "yaga_cauldron" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            for slot in ex.standing() {
                ex.add_status(UnitRef::Hero(slot), StatusKind::Rot, 2);
            }
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                let heal = 15.min(e.max_hp - e.hp);
                if heal > 0 {
                    e.hp += heal;
                    ex.ev.push(Event::Heal { unit: UnitRef::Enemy(uid), amount: heal, hp: e.hp });
                }
            }
        }
        // ---- the new bosses of the first two acts ----
        // the Idolishche snatches the dearest card from the aimed hero's draw pile (from the
        // discard pile when that is empty), swallows it for the fight and grows by its cost
        "devour_hand" => {
            let (src, Some(UnitRef::Hero(slot))) = (ctx.src, ctx.target) else { return };
            let h = &ex.s.heroes[slot as usize];
            let from_draw = !h.draw.is_empty();
            let pile = if from_draw { &h.draw } else { &h.discard };
            let cost = |i: usize| ex.c.cards.get(&pile[i].def).map_or(0, |d| d.view(pile[i].upgraded, h.form).cost);
            let Some(i) = (0..pile.len()).max_by_key(|i| (cost(*i), std::cmp::Reverse(*i))) else { return };
            let gain = cost(i).max(1);
            let h = &mut ex.s.heroes[slot as usize];
            let card = if from_draw { h.draw.remove(i) } else { h.discard.remove(i) };
            ex.ev.push(Event::Exhausted { player: slot, card: card.def.clone() });
            ex.s.heroes[slot as usize].exhaust.push(card);
            ex.add_status(src, StatusKind::Strength, gain);
        }
        // Vikhor lifts a hero off the ground: whatever shield they held is blown away
        "lift" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            ex.s.heroes[slot as usize].block = 0;
        }
        // ...and gathers more wind
        "gust" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                let w = e.flags.entry("gale".into()).or_insert(0);
                *w = (*w + 2).min(crate::combat::GALE_MOST);
            }
        }
        // ---- the new wave ----
        // the changeling: the top card of the aimed hero's draw pile is carried off (laid aside,
        // as if burnt) and a foundling put in its place; the stolen cards come back on its death
        "swap_top" => {
            let (UnitRef::Enemy(uid), Some(UnitRef::Hero(slot))) = (ctx.src, ctx.target) else { return };
            let Some(card) = ex.s.heroes[slot as usize].draw.pop() else { return };
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                e.flags.insert(format!("stolen_{}", card.uid), slot as i32);
                *e.flags.entry("stolen".into()).or_insert(0) += 1;
            }
            ex.ev.push(Event::Exhausted { player: slot, card: card.def.clone() });
            ex.s.heroes[slot as usize].exhaust.push(card);
            ex.add_card(slot, "podkidysh", Pile::Draw);
        }
        "give_back" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            let Some(e) = ex.s.enemies.iter().find(|e| e.uid == uid) else { return };
            let stolen: Vec<(u32, u8)> = e.flags.iter().filter_map(|(k, v)| Some((k.strip_prefix("stolen_")?.parse().ok()?, *v as u8))).collect();
            for (card_uid, slot) in stolen {
                let h = &mut ex.s.heroes[slot as usize];
                if let Some(pos) = h.exhaust.iter().position(|c| c.uid == card_uid) {
                    let card = h.exhaust.remove(pos);
                    let pile = if h.hand.len() < HAND_LIMIT { Pile::Hand } else { Pile::Discard };
                    ex.ev.push(Event::CardAdded { player: slot, card: card.def.clone(), pile });
                    let h = &mut ex.s.heroes[slot as usize];
                    if pile == Pile::Hand {
                        h.hand.push(card);
                    } else {
                        h.discard.push(card);
                    }
                }
            }
        }
        // Tugarin swallows up to two attacks from the aimed hero's discard pile: gone for the
        // fight, and each makes him stronger
        "devour_attacks" => {
            let (src, Some(UnitRef::Hero(slot))) = (ctx.src, ctx.target) else { return };
            for _ in 0..2 {
                let h = &ex.s.heroes[slot as usize];
                let attacks: Vec<usize> = (0..h.discard.len()).filter(|i| ex.c.cards.get(&h.discard[*i].def).is_some_and(|d| d.kind == CardType::Attack)).collect();
                let Some(&i) = ex.s.rng.pick(&attacks) else { break };
                let card = ex.s.heroes[slot as usize].discard.remove(i);
                ex.ev.push(Event::Exhausted { player: slot, card: card.def.clone() });
                ex.s.heroes[slot as usize].exhaust.push(card);
                ex.add_status(src, StatusKind::Strength, 2);
            }
        }
        // the black volhv carves another rune on himself, five at most
        "carve_black" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                let runes = e.flags.entry("black_runes".into()).or_insert(0);
                *runes = (*runes + 1).min(5);
            }
        }
        // ...and calls on them all: three for each rune still whole, to every hero
        "invoke_black" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            let runes = ex.s.enemy(uid).and_then(|e| e.flags.get("black_runes")).copied().unwrap_or(0);
            if runes > 0 {
                for slot in ex.standing() {
                    ex.strike(Some(ctx.src), UnitRef::Hero(slot), 3 * runes, true);
                }
            }
        }
        // the zhivoglot swallows the aimed hero's shield whole
        "eat_block" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            let h = &mut ex.s.heroes[slot as usize];
            let eaten = h.block;
            if eaten > 0 {
                h.block = 0;
                let hp = h.hp;
                ex.ev.push(Event::Damage { src: Some(ctx.src), dst: UnitRef::Hero(slot), blocked: eaten, lost: 0, attack: false, hp, block: 0 });
                ex.gain_block(ctx.src, eaten);
            }
        }
        // ...and belches half of all it holds at the party
        "belch" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            let half = ex.s.enemy(uid).map_or(0, |e| e.block / 2);
            for slot in ex.standing() {
                ex.strike(Some(ctx.src), UnitRef::Hero(slot), 4 + half, true);
            }
        }
        // the obderikha flays every hero's shield off
        "strip_block" => {
            for slot in ex.standing() {
                let h = &mut ex.s.heroes[slot as usize];
                if h.block > 0 {
                    let (blocked, hp) = (h.block, h.hp);
                    h.block = 0;
                    ex.ev.push(Event::Damage { src: Some(ctx.src), dst: UnitRef::Hero(slot), blocked, lost: 0, attack: false, hp, block: 0 });
                }
            }
        }
        // the spinner feeds on every thread of hers still in the heroes' decks
        "feed_webs" => {
            let webs = ex.s.heroes.iter().map(|h| h.hand.iter().chain(&h.draw).chain(&h.discard).filter(|c| c.def == "silk").count() as i32).sum::<i32>();
            if webs > 0 {
                ex.heal(ctx.src, 3 * webs);
                ex.add_status(ctx.src, StatusKind::Strength, (webs + 1) / 2);
            }
        }
        // the priest of Chernobog gives the weakest of his flock to his god: its life becomes
        // his shield, and he grows stronger. With no one left to give, he shields himself
        "sacrifice" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            let victim = ex.s.enemies.iter().filter(|e| e.alive && e.uid != uid).min_by_key(|e| e.hp).map(|e| (e.uid, e.hp));
            match victim.and_then(|(v, hp)| Some((ex.enemy_idx(v)?, hp))) {
                Some((idx, hp)) => {
                    ex.s.enemies[idx].hp = 0;
                    ex.kill(idx, None);
                    ex.gain_block(ctx.src, hp);
                    ex.add_status(ctx.src, StatusKind::Strength, 3);
                }
                None => ex.gain_block(ctx.src, 15),
            }
        }
        // the alkonost's lullaby: all but one of the party fall asleep, until a friend's card
        // wakes them; one alone only grows drowsy
        "lullaby" => {
            let mut standing = ex.standing();
            if standing.len() <= 1 {
                for slot in standing {
                    *ex.s.heroes[slot as usize].flags.entry("draw_less".into()).or_insert(0) += 2;
                }
                return;
            }
            ex.s.rng.shuffle(&mut standing);
            for slot in standing.into_iter().skip(1) {
                ex.s.heroes[slot as usize].flags.insert("asleep".into(), 1);
            }
        }
        // Chernobog's black sun: it climbs unless the party hurt him hard since his last move;
        // risen full, it blinds everyone and he strikes with all its weight
        "eclipse_rise" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            let Some(idx) = ex.enemy_idx(uid) else { return };
            let e = &mut ex.s.enemies[idx];
            let harm = e.flags.insert("harm_taken".into(), 0).unwrap_or(0);
            let sun = e.flags.entry("eclipse".into()).or_insert(1);
            *sun = if harm >= ECLIPSE_BREAK { (*sun - 1).max(1) } else { *sun + 1 };
            if *sun >= ECLIPSE_FULL {
                *sun = 1;
                for h in ex.s.heroes.iter_mut().filter(|h| !h.downed) {
                    h.flags.insert("no_intents".into(), 1);
                }
                ex.add_status(ctx.src, StatusKind::Strength, 4);
            }
        }
        _ => debug_assert!(false, "unknown hook `{name}`"),
    }
}

/// Harm the party must do to Chernobog between his moves to push his black sun back a step.
pub const ECLIPSE_BREAK: i32 = 30;
/// The step at which the black sun is full.
pub const ECLIPSE_FULL: i32 = 4;

/// How many turns Morovitsa's Doom counts down before it strikes.
pub const DOOM_TURNS: i32 = 4;
/// How much health Doom takes when its count runs out.
pub const DOOM_HP: i32 = 30;

/// The base of a blow an enemy's hooked move deals (before strength, weakness and the like), for
/// the move itself and for the number its intent shows. `None` for a hook that deals no blow.
pub fn hook_damage(c: &Content, s: &Combat, src: UnitRef, name: &str, target: Option<u8>) -> Option<i32> {
    match name {
        "bayun_claws" => {
            // what lies in the discard, and what in the hand will go there when the turn ends (so
            // the intent tells the truth before the hand is thrown away)
            let curses = target.and_then(|t| s.heroes.get(t as usize)).map_or(0, |h| {
                let bad = |card: &&Card| c.cards.get(&card.def).is_some_and(|d| matches!(d.kind, CardType::Status | CardType::Curse));
                let lands = |card: &&Card| c.cards.get(&card.def).is_some_and(|d| {
                    let v = d.view(card.upgraded, h.form);
                    !v.has(Keyword::Ethereal) && !v.has(Keyword::Retain)
                });
                (h.discard.iter().filter(bad).count() + h.hand.iter().filter(bad).filter(lands).count()) as i32
            });
            Some(10 + curses * 4)
        }
        "kinetic_burst" => {
            let UnitRef::Enemy(uid) = src else { return None };
            let charge = s.enemy(uid).and_then(|e| e.flags.get("kinetic_charge")).copied().unwrap_or(0);
            Some(10 + charge * 3)
        }
        _ => None,
    }
}
