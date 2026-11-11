mod common;
use common::*;
use nav_core::*;
use StatusKind as S;

const ME: UnitRef = UnitRef::Hero(0);

/// Restarts with new seeds until the first enemy aims at `slot`.
fn aimed_at(c: &Content, heroes: &[HeroSetup], enemies: &[&str], slot: u8) -> Combat {
    for seed in 0..200 {
        let (s, _) = start(c, heroes, &ids(enemies), seed);
        if s.enemies[0].intent.as_ref().and_then(|i| i.target) == Some(slot) {
            return s;
        }
    }
    panic!("no seed aims at slot {slot}");
}

// ---- rng ----

#[test]
fn rng_is_deterministic_and_in_range() {
    let (mut a, mut b) = (Rng::new(42), Rng::new(42));
    for _ in 0..1000 {
        let x = a.range(3, 9);
        assert_eq!(x, b.range(3, 9));
        assert!((3..=9).contains(&x));
    }
    assert_ne!(Rng::new(1).next_u32(), Rng::new(2).next_u32());
    let mut v: Vec<u32> = (0..20).collect();
    Rng::new(5).shuffle(&mut v);
    let mut sorted = v.clone();
    sorted.sort();
    assert_eq!(sorted, (0..20).collect::<Vec<_>>());
    assert_ne!(v, sorted);
}

// ---- content ----

#[test]
fn content_loads_and_validates() {
    let c = content();
    let b = &c.heroes["bogatyr"];
    assert_eq!(b.max_hp, 85);
    assert_eq!(b.deck.len(), 10);
    assert_eq!(c.cards["bogatyr_strike"].view(false, Form::Human).effects, vec![Effect::Damage(Amount::N(6), Tgt::Target)]);
    assert_eq!(c.cards["bogatyr_defend"].view(false, Form::Human).effects, vec![Effect::Block(Amount::N(5), Tgt::Me)]);
}

#[test]
fn dangling_ids_are_rejected() {
    let mut c = Content::default();
    c.add_cards(r#"[(id: "bad", kind: Skill, effects: [AddCard(card: "ghost", pile: Hand, to: Me)])]"#).unwrap();
    assert!(c.validate().is_err());
}

// ---- damage, block, hp ----

#[test]
fn damage_hits_hp() {
    let (c, mut s) = solo(&["t_hit"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 90);
}

#[test]
fn multi_hit_and_aoe() {
    let (c, mut s) = solo(&["t_multi", "t_hit_all"], &["dummy", "dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_multi", Some(t));
    assert_eq!(s.enemies[0].hp, 94);
    play(&c, &mut s, 0, "t_hit_all", None);
    assert_eq!((s.enemies[0].hp, s.enemies[1].hp), (90, 96));
}

#[test]
fn block_absorbs_then_resets_next_round() {
    let (c, mut s) = solo(&["t_block"], &["hitter"]);
    play(&c, &mut s, 0, "t_block", None);
    assert_eq!(s.heroes[0].block, 5);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 45);
    assert_eq!(s.heroes[0].block, 0);
    assert_eq!(s.round, 2);
}

#[test]
fn enemy_block_lasts_until_its_next_turn() {
    let (c, mut s) = solo(&["t_hit"], &["armored"]);
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].block, 20);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!((s.enemies[0].hp, s.enemies[0].block), (100, 10));
}

#[test]
fn heal_is_capped_and_lose_hp_ignores_block() {
    let (c, mut s) = solo(&["t_block", "t_pain", "t_heal"], &["dummy"]);
    play(&c, &mut s, 0, "t_block", None);
    play(&c, &mut s, 0, "t_pain", None);
    assert_eq!((s.heroes[0].hp, s.heroes[0].block), (46, 5));
    play(&c, &mut s, 0, "t_heal", None);
    assert_eq!(s.heroes[0].hp, 50);
}

// ---- statuses ----

#[test]
fn strength_adds_to_attacks() {
    let (c, mut s) = solo(&["t_str", "t_hit", "t_multi"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_str", None);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 88);
    play(&c, &mut s, 0, "t_multi", Some(t));
    assert_eq!(s.enemies[0].hp, 76);
}

#[test]
fn weak_cuts_damage_by_a_quarter_and_expires() {
    let (c, mut s) = solo(&["t_weak_me", "t_hit"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_weak_me", None);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 93);
    end_round(&c, &mut s);
    assert_eq!(s.status(ME, S::Weak), 0);
}

#[test]
fn weak_enemy_hits_softer() {
    let (c, mut s) = solo(&["t_weak"], &["hitter"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_weak", Some(t));
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 43);
    assert_eq!(s.status(t, S::Weak), 1);
}

#[test]
fn vulnerable_takes_half_more() {
    let (c, mut s) = solo(&["t_vuln", "t_hit"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_vuln", Some(t));
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 85);
}

#[test]
fn rot_ticks_then_shrinks() {
    let (c, mut s) = solo(&["t_rot"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_rot", Some(t));
    end_round(&c, &mut s);
    assert_eq!((s.enemies[0].hp, s.status(t, S::Rot)), (97, 2));
    end_round(&c, &mut s);
    assert_eq!((s.enemies[0].hp, s.status(t, S::Rot)), (95, 1));
    end_round(&c, &mut s);
    assert_eq!((s.enemies[0].hp, s.status(t, S::Rot)), (94, 0));
}

#[test]
fn rot_on_a_hero_ticks_at_end_of_their_turn() {
    let (c, mut s) = solo(&["t_hit"], &["spore", "dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.status(ME, S::Rot), 3);
    end_round(&c, &mut s);
    assert_eq!((s.heroes[0].hp, s.status(ME, S::Rot)), (47, 2));
}

#[test]
fn hex_boosts_the_next_x_hits() {
    let (c, mut s) = solo(&["t_hex", "t_hit", "t_hit", "t_hit"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hex", Some(t));
    play(&c, &mut s, 0, "t_hit", Some(t));
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 74);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 64);
}

#[test]
fn bleed_hurts_the_attacker() {
    let (c, mut s) = solo(&["t_bleed"], &["hitter"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_bleed", Some(t));
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].hp, 96);
    assert_eq!(s.heroes[0].hp, 40);
}

#[test]
fn fear_caps_at_75_and_sometimes_skips() {
    let c = content();
    let mut skipped = 0;
    for seed in 0..40 {
        let (mut s, _) = start(&c, &[hero("t_plain", &["t_fear", "t_fear"])], &ids(&["hitter"]), seed);
        let t = foe(&s, 0);
        play(&c, &mut s, 0, "t_fear", Some(t));
        play(&c, &mut s, 0, "t_fear", Some(t));
        assert_eq!(s.status(t, S::Fear), 750, "kept in tenths: 75%");
        end_round(&c, &mut s);
        if s.heroes[0].hp == 50 {
            skipped += 1;
            assert_eq!(s.status(t, S::Fear), 0, "fear is spent when it triggers");
        }
    }
    assert!((20..=38).contains(&skipped), "skipped {skipped}/40 at 75%");
}

#[test]
fn stun_skips_one_turn() {
    let (c, mut s) = solo(&["t_stun"], &["hitter"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_stun", Some(t));
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 50);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 40);
}

#[test]
fn thorns_bite_back() {
    let (c, mut s) = solo(&["t_thorns"], &["hitter"]);
    play(&c, &mut s, 0, "t_thorns", None);
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].hp, 97);
}

#[test]
fn fortitude_adds_to_card_block() {
    let (c, mut s) = solo(&["t_fort", "t_block"], &["dummy"]);
    play(&c, &mut s, 0, "t_fort", None);
    play(&c, &mut s, 0, "t_block", None);
    assert_eq!(s.heroes[0].block, 7);
}

#[test]
fn hunt_mark_lasts_one_round() {
    let (c, mut s) = solo(&["t_mark", "t_hit"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_mark", Some(t));
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 86);
    end_round(&c, &mut s);
    assert_eq!(s.status(t, S::HuntMark), 0);
}

#[test]
fn cleanse_strips_debuffs_only() {
    let (c, mut s) = solo(&["t_weak_me", "t_str", "t_cleanse"], &["dummy"]);
    play(&c, &mut s, 0, "t_weak_me", None);
    play(&c, &mut s, 0, "t_str", None);
    play(&c, &mut s, 0, "t_cleanse", None);
    assert_eq!((s.status(ME, S::Weak), s.status(ME, S::Strength)), (0, 2));
}

// ---- deck ----

#[test]
fn opening_hand_energy_and_reshuffle() {
    let (c, mut s) = solo(&["t_block"; 7], &["dummy"]);
    assert_eq!((s.heroes[0].hand.len(), s.heroes[0].draw.len(), s.heroes[0].energy), (5, 2, 3));
    end_round(&c, &mut s);
    let h = &s.heroes[0];
    assert_eq!((h.hand.len(), h.draw.len(), h.discard.len()), (5, 2, 0));
}

#[test]
fn hand_is_capped_at_ten() {
    let (c, mut s) = solo(&["t_draw9"; 12], &["dummy"]);
    play(&c, &mut s, 0, "t_draw9", None);
    assert_eq!((s.heroes[0].hand.len(), s.heroes[0].draw.len()), (10, 1));
}

#[test]
fn draw_stops_on_an_empty_deck() {
    let (c, mut s) = solo(&["t_draw", "t_block"], &["dummy"]);
    play(&c, &mut s, 0, "t_draw", None);
    assert_eq!(s.heroes[0].hand.len(), 1);
}

#[test]
fn add_card_to_hand() {
    let (c, mut s) = solo(&["t_gift"], &["dummy"]);
    play(&c, &mut s, 0, "t_gift", None);
    assert_eq!(s.heroes[0].hand.iter().filter(|x| x.def == "t_hit").count(), 2);
}

// ---- keywords ----

#[test]
fn exhaust_leaves_the_deck() {
    let (c, mut s) = solo(&["t_exhaust"], &["dummy"]);
    play(&c, &mut s, 0, "t_exhaust", None);
    assert_eq!((s.heroes[0].exhaust.len(), s.heroes[0].discard.len()), (1, 0));
}

#[test]
fn retain_stays_and_ethereal_burns() {
    let (c, mut s) = solo(&["t_retain", "t_ether", "t_cost2"], &["dummy"]);
    let kept = s.heroes[0].hand.iter().find(|x| x.def == "t_retain").unwrap().uid;
    end_round(&c, &mut s);
    let h = &s.heroes[0];
    assert!(h.hand.iter().any(|x| x.uid == kept));
    assert_eq!(h.exhaust.len(), 1);
    assert_eq!(h.exhaust[0].def, "t_ether");
    assert!(h.hand.iter().any(|x| x.def == "t_cost2"));
}

#[test]
fn unplayable_and_unaffordable_leave_state_untouched() {
    let (c, mut s) = solo(&["web", "t_cost2", "t_cost2"], &["dummy"]);
    let before = s.hash64();
    assert_eq!(try_play(&c, &mut s, 0, "web", None), Err(ActionError::Unplayable));
    assert_eq!(s.hash64(), before);
    play(&c, &mut s, 0, "t_cost2", None);
    let before = s.hash64();
    assert_eq!(try_play(&c, &mut s, 0, "t_cost2", None), Err(ActionError::NotEnoughEnergy));
    assert_eq!(s.hash64(), before);
}

#[test]
fn bad_targets_are_refused() {
    let (c, mut s) = solo(&["t_hit", "t_revive"], &["dummy"]);
    assert_eq!(try_play(&c, &mut s, 0, "t_hit", None), Err(ActionError::BadTarget));
    assert_eq!(try_play(&c, &mut s, 0, "t_hit", Some(ME)), Err(ActionError::BadTarget));
    assert_eq!(try_play(&c, &mut s, 0, "t_hit", Some(UnitRef::Enemy(9999))), Err(ActionError::BadTarget));
    assert_eq!(try_play(&c, &mut s, 0, "t_revive", Some(ME)), Err(ActionError::BadTarget));
}

#[test]
fn upgrade_changes_cost_and_numbers() {
    let c = content();
    let mut h = hero("t_plain", &[]);
    h.deck = vec![("t_up".into(), true)];
    let (mut s, _) = start(&c, &[h], &ids(&["dummy"]), 1);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_up", Some(t));
    assert_eq!((s.enemies[0].hp, s.heroes[0].energy), (91, 3));
}

#[test]
fn link_needs_an_ally_card_of_that_type() {
    let (c, mut s) = duo(&["t_link", "t_link", "t_block"], &["t_block"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_block", None);
    play(&c, &mut s, 0, "t_link", Some(t));
    assert_eq!(s.enemies[0].hp, 175, "own skill doesn't count");
    play(&c, &mut s, 1, "t_block", None);
    play(&c, &mut s, 0, "t_link", Some(t));
    assert_eq!(s.enemies[0].hp, 165);
}

#[test]
fn consume_eats_the_curse_for_a_bonus() {
    let (c, mut s) = solo(&["t_rot", "t_consume"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_rot", Some(t));
    play(&c, &mut s, 0, "t_consume", Some(t));
    assert_eq!((s.enemies[0].hp, s.status(t, S::Rot)), (91, 0));
}

#[test]
fn x_cost_spends_everything() {
    let (c, mut s) = solo(&["t_x"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_x", Some(t));
    assert_eq!((s.enemies[0].hp, s.heroes[0].energy), (91, 0));
}

#[test]
fn fatal_bonus_only_on_a_kill() {
    let (c, mut s) = solo(&["t_pain", "t_pain", "t_fatal", "t_fatal"], &["weakling", "dummy"]);
    let (weak, tank) = (foe(&s, 0), foe(&s, 1));
    play(&c, &mut s, 0, "t_pain", None);
    play(&c, &mut s, 0, "t_pain", None);
    play(&c, &mut s, 0, "t_fatal", Some(tank));
    assert_eq!(s.heroes[0].hp, 42);
    play(&c, &mut s, 0, "t_fatal", Some(weak));
    assert_eq!(s.heroes[0].hp, 50);
    assert!(!s.enemies[0].alive);
}

#[test]
fn coop_condition_and_played_counter() {
    let (c, mut s) = solo(&["t_coop", "t_played"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_coop", None);
    assert_eq!(s.heroes[0].block, 2);
    play(&c, &mut s, 0, "t_played", Some(t));
    assert_eq!(s.enemies[0].hp, 97);
    let (c, mut s) = duo(&["t_coop"], &[], &["dummy"]);
    play(&c, &mut s, 0, "t_coop", None);
    assert_eq!(s.heroes[0].block, 9);
}

#[test]
fn power_sticks_for_the_fight() {
    let (c, mut s) = solo(&["t_power"], &["dummy"]);
    play(&c, &mut s, 0, "t_power", None);
    let h = &s.heroes[0];
    assert_eq!(h.hand.len() + h.discard.len() + h.exhaust.len() + h.draw.len(), 0);
    end_round(&c, &mut s);
    assert_eq!(s.status(ME, S::Strength), 1);
    end_round(&c, &mut s);
    assert_eq!(s.status(ME, S::Strength), 2);
}

#[test]
fn ally_energy_and_solo_fallback() {
    let (c, mut s) = duo(&["t_energy"], &[], &["dummy"]);
    assert_eq!(try_play(&c, &mut s, 0, "t_energy", Some(ME)), Err(ActionError::BadTarget));
    play(&c, &mut s, 0, "t_energy", Some(UnitRef::Hero(1)));
    assert_eq!(s.heroes[1].energy, 5);
    let (c, mut s) = solo(&["t_energy"], &["dummy"]);
    play(&c, &mut s, 0, "t_energy", None);
    assert_eq!(s.heroes[0].energy, 3);
}

// ---- Bogatyr ----

#[test]
fn father_amulet_shields_the_party() {
    let c = content();
    let (s, _) = start(&c, &[HeroSetup::fresh(&c, "bogatyr").unwrap(), hero("t_plain", &[])], &ids(&["dummy"]), 3);
    assert_eq!((s.heroes[0].block, s.heroes[1].block), (6, 3));
}

#[test]
fn valor_one_per_five_absorbed() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("bogatyr", &["bogatyr_defend"])], &ids(&["hitter"]), 3);
    play(&c, &mut s, 0, "bogatyr_defend", None);
    end_round(&c, &mut s);
    assert_eq!((s.heroes[0].hp, s.status(ME, S::Valor)), (45, 2));
}

#[test]
fn guard_redirects_overflow_to_the_bogatyr() {
    let c = content();
    let party = [hero("bogatyr", &["bogatyr_guard"]), hero("t_plain", &[])];
    let mut s = aimed_at(&c, &party, &["hitter"], 1);
    play(&c, &mut s, 0, "bogatyr_guard", Some(UnitRef::Hero(1)));
    assert_eq!((s.heroes[1].block, s.heroes[1].guard_by), (7, Some(0)));
    end_round(&c, &mut s);
    assert_eq!((s.heroes[0].hp, s.heroes[1].hp), (47, 50));
    assert_eq!(s.heroes[1].guard_by, None);
    assert_eq!(s.heroes[0].valor_rem, 3);
}

#[test]
fn guard_on_self_is_plain_block() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("bogatyr", &["bogatyr_guard"])], &ids(&["dummy"]), 3);
    play(&c, &mut s, 0, "bogatyr_guard", Some(ME));
    assert_eq!((s.heroes[0].block, s.heroes[0].guard_by), (7, None));
}

#[test]
fn challenge_pulls_every_attack() {
    let c = content();
    let party = [hero("bogatyr", &["bogatyr_challenge"]), hero("t_plain", &[])];
    let mut s = aimed_at(&c, &party, &["hitter"], 1);
    play(&c, &mut s, 0, "bogatyr_challenge", None);
    assert_eq!(s.enemies[0].intent.as_ref().unwrap().target, Some(0));
    end_round(&c, &mut s);
    assert_eq!((s.heroes[0].hp, s.heroes[1].hp), (50, 50));
}

#[test]
fn svyatogor_spends_valor() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("bogatyr", &["bogatyr_sweep", "bogatyr_svyatogor"])], &ids(&["dummy"]), 3);
    s.heroes[0].energy = 10;
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "bogatyr_sweep", Some(t));
    assert_eq!((s.enemies[0].hp, s.status(ME, S::Valor)), (88, 2));
    play(&c, &mut s, 0, "bogatyr_svyatogor", Some(t));
    assert_eq!((s.enemies[0].hp, s.status(ME, S::Valor)), (84, 0));
}

#[test]
fn ancestor_mail_and_heavy_burden() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("bogatyr", &["bogatyr_ancestor_mail"])], &ids(&["hitter"]), 3);
    play(&c, &mut s, 0, "bogatyr_ancestor_mail", None);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 43);

    let party = [hero("bogatyr", &["bogatyr_heavy_burden"]), hero("t_plain", &["t_pain"])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 3);
    play(&c, &mut s, 0, "bogatyr_heavy_burden", None);
    play(&c, &mut s, 1, "t_pain", None);
    assert_eq!(s.status(ME, S::Valor), 2);
}

// ---- runes ----

#[test]
fn runes_fire_at_end_of_turn_and_on_invoke() {
    let (c, mut s) = solo(&["t_rune_thunder", "t_invoke"], &["dummy"]);
    play(&c, &mut s, 0, "t_rune_thunder", None);
    play(&c, &mut s, 0, "t_invoke", None);
    assert_eq!(s.enemies[0].hp, 94);
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].hp, 88);
    assert_eq!(s.heroes[0].runes, vec![RuneKind::Thunder], "runes stay carved");
}

#[test]
fn rune_kinds() {
    let (c, mut s) = solo(&["t_rune_flame", "t_rune_zhiva", "t_rune_mist", "t_pain"], &["hitter", "dummy"]);
    for card in ["t_pain", "t_rune_flame", "t_rune_zhiva", "t_rune_mist"] {
        play(&c, &mut s, 0, card, None);
    }
    end_round(&c, &mut s);
    assert_eq!((s.enemies[0].hp, s.enemies[1].hp), (97, 97));
    // 50 - 4 pain + 2 zhiva - half of 10 through the mist
    assert_eq!(s.heroes[0].hp, 43);
}

#[test]
fn ally_holds_at_most_three_runes_and_they_count() {
    let (c, mut s) = duo(&["t_rune_frost"; 4], &["t_rune_thunder", "t_rune_thunder", "t_runes_dmg"], &["dummy"]);
    for _ in 0..4 {
        play(&c, &mut s, 0, "t_rune_frost", Some(UnitRef::Hero(1)));
    }
    assert_eq!(s.heroes[1].runes.len(), 3);
    let t = foe(&s, 0);
    play(&c, &mut s, 1, "t_rune_thunder", None);
    play(&c, &mut s, 1, "t_rune_thunder", None);
    play(&c, &mut s, 1, "t_runes_dmg", Some(t));
    assert_eq!(s.enemies[0].hp, 170);
}

#[test]
fn a_new_rune_pushes_out_the_oldest_which_discharges() {
    let (c, mut s) = duo(&["t_rune_frost"; 4], &[], &["dummy"]);
    for _ in 0..3 {
        play(&c, &mut s, 0, "t_rune_frost", Some(UnitRef::Hero(1)));
    }
    assert_eq!(s.heroes[1].block, 0);
    // the fourth finds no room: the oldest fires one last time and is gone
    let ev = play(&c, &mut s, 0, "t_rune_frost", Some(UnitRef::Hero(1)));
    assert_eq!(s.heroes[1].runes.len(), 3);
    assert_eq!(s.heroes[1].block, 4, "the frost rune gave its block as it went");
    assert!(ev.iter().any(|e| matches!(e, Event::RuneDischarged { player: 1, rune: RuneKind::Frost })));
}

// ---- werewolf ----

#[test]
fn beast_form_flips_cards_and_bans_own_block() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_moon", &["t_shift", "t_claw", "t_claw", "t_block"])], &ids(&["dummy"]), 3);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_claw", Some(t));
    assert_eq!(s.enemies[0].hp, 94);
    play(&c, &mut s, 0, "t_shift", None);
    assert_eq!(s.heroes[0].form, Form::Beast);
    play(&c, &mut s, 0, "t_claw", Some(t));
    // one moon gathered: a Strength of fury, half again as a beast's blow
    assert_eq!((s.enemies[0].hp, s.status(t, S::Bleed)), (79, 2));
    play(&c, &mut s, 0, "t_block", None);
    assert_eq!(s.heroes[0].block, 0);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 48, "hunger");
}

#[test]
fn five_attacks_fill_the_moon() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_moon", &["t_claw"; 5])], &ids(&["dummy"]), 3);
    let t = foe(&s, 0);
    for i in 0..5 {
        assert_eq!(s.heroes[0].form, Form::Human, "attack {i}");
        play(&c, &mut s, 0, "t_claw", Some(t));
    }
    assert_eq!((s.heroes[0].form, s.heroes[0].moon), (Form::Beast, 0));
}

// ---- masks ----

#[test]
fn masks() {
    let (c, mut s) = solo(&["t_bear", "t_hit"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_bear", None);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 87);

    let (c, mut s) = duo(&["t_goat", "t_block"], &[], &["dummy"]);
    play(&c, &mut s, 0, "t_goat", None);
    play(&c, &mut s, 0, "t_block", None);
    assert_eq!((s.heroes[0].block, s.heroes[1].block), (5, 4));

    let (c, mut s) = solo(&["t_mara", "t_hit"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_mara", None);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.status(t, S::Fear), 75, "a blow in the Mara mask gives 7.5% Fear");
}

// ---- falling and rising ----

#[test]
fn everyone_down_loses_the_run() {
    let (c, mut s) = solo(&["t_suicide"], &["dummy"]);
    let ev = play(&c, &mut s, 0, "t_suicide", None);
    assert_eq!(s.phase, Phase::Lost);
    assert!(ev.contains(&Event::Ended { won: false }));
    assert_eq!(apply(&c, &mut s, 0, &CombatAction::Ready(true)), Err(ActionError::NotHeroPhase));
}

#[test]
fn raise_costs_two_energy_and_a_card() {
    let c = content();
    let party = [hero("t_plain", &["t_block", "t_block"]), hero("t_plain", &[])];
    let mut s = aimed_at(&c, &party, &["brute"], 1);
    end_round(&c, &mut s);
    assert!(s.heroes[1].downed);
    assert_eq!(s.round, 2, "the round closes without the fallen hero");
    assert_eq!(apply(&c, &mut s, 1, &CombatAction::Ready(true)), Err(ActionError::Downed));
    let card = s.heroes[0].hand[0].uid;
    apply(&c, &mut s, 0, &CombatAction::Raise { ally: 1, discard: card }).unwrap();
    let ally = &s.heroes[1];
    assert_eq!((ally.downed, ally.hp, ally.falls, ally.new_curses), (false, 12, 1, 0));
    assert_eq!((s.heroes[0].energy, s.heroes[0].hand.len()), (1, 1));
    assert_eq!(apply(&c, &mut s, 0, &CombatAction::Raise { ally: 1, discard: 0 }).is_err(), true);
}

#[test]
fn second_fall_brings_the_nav_mark() {
    let c = content();
    let mut h = hero("t_plain", &["t_suicide"]);
    h.falls = 1;
    let (mut s, _) = start(&c, &[h, hero("t_plain", &[])], &ids(&["dummy"]), 3);
    play(&c, &mut s, 0, "t_suicide", None);
    let h = &s.heroes[0];
    assert_eq!((h.falls, h.new_curses), (2, 1));
    assert!(h.discard.iter().any(|x| x.def == NAV_MARK));
}

#[test]
fn downed_at_victory_stands_at_one_hp_marked() {
    let (c, mut s) = duo(&["t_hit"], &["t_suicide"], &["weakling"]);
    play(&c, &mut s, 1, "t_suicide", None);
    s.enemies[0].hp = 10;
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.phase, Phase::Won);
    let h = &s.heroes[1];
    assert_eq!((h.downed, h.hp, h.new_curses), (false, 1, 1));
}

#[test]
fn revive_card_and_nav_mark_in_hand() {
    let (c, mut s) = duo(&["t_revive"], &["t_suicide"], &["dummy"]);
    play(&c, &mut s, 1, "t_suicide", None);
    play(&c, &mut s, 0, "t_revive", Some(UnitRef::Hero(1)));
    assert_eq!((s.heroes[1].downed, s.heroes[1].hp), (false, 25));

    let (c, mut s) = solo(&[NAV_MARK], &["dummy"]);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 48);
}

// ---- enemies ----

#[test]
fn enemy_hp_scales_with_party_size() {
    let c = content();
    for (n, hp) in [(1, 100), (2, 180), (3, 250), (4, 320)] {
        let party: Vec<HeroSetup> = (0..n).map(|_| hero("t_plain", &[])).collect();
        let (s, _) = start(&c, &party, &ids(&["dummy"]), 1);
        assert_eq!(s.enemies[0].max_hp, hp);
    }
}

#[test]
fn opening_then_pattern() {
    let (c, mut s) = solo(&[], &["opener"]);
    assert_eq!(s.enemies[0].intent.as_ref().unwrap().mv, 0);
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].intent.as_ref().unwrap().mv, 1);
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].intent.as_ref().unwrap().mv, 1);
    assert_eq!(s.status(foe(&s, 0), S::Strength), 1);
}

#[test]
fn summon_adds_enemies_that_act_next_round() {
    let (c, mut s) = solo(&[], &["summoner"]);
    end_round(&c, &mut s);
    assert_eq!(s.enemies.len(), 3);
    assert!(s.enemies[1].intent.is_some());
}

#[test]
fn thief_steals_then_flees_or_drops_the_gold() {
    let (c, mut s) = solo(&[], &["thief"]);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].gold_lost, 15);
    end_round(&c, &mut s);
    assert_eq!(s.phase, Phase::Won);
    assert!(s.enemies[0].fled);
    assert_eq!(s.heroes[0].gold_lost, 15);

    let (c, mut s) = solo(&["t_hit", "t_hit"], &["thief"]);
    end_round(&c, &mut s);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!((s.phase, s.heroes[0].gold_lost), (Phase::Won, 0));
}

#[test]
fn wolves_bite_and_howl() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &[])], &ids(&["grey_wolf", "grey_wolf"]), 3);
    assert!((13..=15).contains(&s.enemies[0].max_hp));
    end_round(&c, &mut s);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 50 - 24);
    end_round(&c, &mut s);
    assert_eq!(s.status(foe(&s, 0), S::Strength), 2, "both wolves howled");
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 50 - 24 - 16);
}

#[test]
fn kikimora_tangles_then_chokes() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &[])], &ids(&["kikimora"]), 3);
    end_round(&c, &mut s);
    let h = &s.heroes[0];
    assert!(h.hand.iter().chain(&h.discard).chain(&h.draw).any(|x| x.def == "web"));
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 42);
}

// ---- turn structure ----

#[test]
fn round_closes_only_when_everyone_is_ready() {
    let (c, mut s) = duo(&[], &[], &["dummy"]);
    ready(&c, &mut s, 0);
    assert_eq!(s.round, 1);
    apply(&c, &mut s, 0, &CombatAction::Ready(false)).unwrap();
    ready(&c, &mut s, 1);
    assert_eq!(s.round, 1);
    ready(&c, &mut s, 0);
    assert_eq!(s.round, 2);
    assert!(s.heroes.iter().all(|h| !h.ready));
}

#[test]
fn reduce_is_pure() {
    let (c, s) = solo(&["t_hit"], &["dummy"]);
    let before = s.hash64();
    let card = s.heroes[0].hand[0].uid;
    let (next, _) = reduce(&c, &s, 0, &CombatAction::Play { card, target: Some(foe(&s, 0)) }).unwrap();
    assert_eq!(s.hash64(), before);
    assert_ne!(next.hash64(), before);
}

fn bot_fight(c: &Content, party: &[HeroSetup], enemies: &[&str], seed: u64) -> (Combat, Vec<u64>) {
    let (mut s, _) = start(c, party, &ids(enemies), seed);
    let mut hashes = Vec::new();
    for _ in 0..2000 {
        if s.phase != Phase::Heroes {
            break;
        }
        for p in 0..s.heroes.len() as u8 {
            if s.phase != Phase::Heroes || s.heroes[p as usize].downed || s.heroes[p as usize].ready {
                continue;
            }
            let a = bot::choose(c, &s, p);
            apply(c, &mut s, p, &a).unwrap_or_else(|e| panic!("bot made an illegal move {a:?}: {e:?}"));
            hashes.push(s.hash64());
        }
    }
    (s, hashes)
}

#[test]
fn bot_finishes_fights_and_replays_identically() {
    let c = content();
    let solo = [HeroSetup::fresh(&c, "bogatyr").unwrap()];
    let (a, ha) = bot_fight(&c, &solo, &["grey_wolf", "grey_wolf"], 11);
    let (_, hb) = bot_fight(&c, &solo, &["grey_wolf", "grey_wolf"], 11);
    assert_eq!(a.phase, Phase::Won);
    assert_eq!(ha, hb, "same seed and actions must give the same state");
    let (_, hc) = bot_fight(&c, &solo, &["grey_wolf", "grey_wolf"], 12);
    assert_ne!(ha, hc);

    let (k, _) = bot_fight(&c, &solo, &["kikimora"], 5);
    assert_eq!(k.phase, Phase::Won);

    let four: Vec<HeroSetup> = (0..4).map(|_| HeroSetup::fresh(&c, "bogatyr").unwrap()).collect();
    let (f, _) = bot_fight(&c, &four, &["kikimora", "grey_wolf", "grey_wolf"], 5);
    assert_ne!(f.phase, Phase::Heroes, "a four-player fight must end");
}

// ---- Koschei ----

fn koschei_fight(deck: &[&str]) -> (Content, Combat) {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", deck)], &ids(&["koschei", "death_oak"]), 7);
    s.heroes[0].energy = 99;
    (c, s)
}

/// Puts a free test card straight into the hero's hand.
fn give(s: &mut Combat, uid: u32, def: &str) {
    s.heroes[0].hand.push(Card { uid, def: def.into(), upgraded: false, free: true });
}

#[test]
fn koschei_cannot_die_while_the_needle_is_whole() {
    let (c, mut s) = koschei_fight(&["t_hit"; 5]);
    let k = foe(&s, 0);
    s.enemies[0].hp = 12;
    play(&c, &mut s, 0, "t_hit", Some(k));
    play(&c, &mut s, 0, "t_hit", Some(k));
    assert_eq!((s.enemies[0].hp, s.enemies[0].alive), (1, true));
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].hp, 1 + 60, "he knits back a tenth of his life each turn");
    assert_eq!(s.phase, Phase::Heroes);
}

#[test]
fn the_chain_of_death_comes_one_link_at_a_time() {
    let (c, mut s) = koschei_fight(&[]);
    let names = |s: &Combat| -> Vec<String> { s.living_enemies().map(|e| e.def.clone()).collect() };
    assert_eq!(names(&s), ["koschei", "death_oak"]);
    for (n, (link, next)) in [("death_oak", "death_chest"), ("death_chest", "death_hare")].into_iter().enumerate() {
        let idx = s.enemies.iter().position(|e| e.def == link && e.alive).unwrap();
        s.enemies[idx].hp = 1;
        s.enemies[idx].block = 0;
        let t = foe(&s, idx);
        give(&mut s, 9000 + n as u32, "t_hit");
        play(&c, &mut s, 0, "t_hit", Some(t));
        assert!(names(&s).contains(&next.to_string()), "{link} gives way to {next}");
    }
    // the hare: the first blow of the round misses
    let hare = s.enemies.iter().position(|e| e.def == "death_hare").unwrap();
    let t = foe(&s, hare);
    give(&mut s, 9100, "t_hit");
    give(&mut s, 9101, "t_hit");
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[hare].hp, 60);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[hare].hp, 50);
}

#[test]
fn the_duck_is_only_hit_by_sweeping_blows() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &["t_hit", "t_hit_all"])], &ids(&["death_duck"]), 7);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 60);
    play(&c, &mut s, 0, "t_hit_all", None);
    assert_eq!(s.enemies[0].hp, 56);
}

#[test]
fn the_egg_keeps_its_shell_and_drops_the_needle() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &["t_hit"; 5])], &ids(&["koschei", "death_egg"]), 7);
    let egg = foe(&s, 1);
    assert_eq!(s.enemies[1].block, 30);
    play(&c, &mut s, 0, "t_hit", Some(egg));
    play(&c, &mut s, 0, "t_hit", Some(egg));
    assert_eq!(s.enemies[1].block, 10);
    end_round(&c, &mut s);
    assert_eq!(s.enemies[1].block, 30, "the shell is whole again");
    for _ in 0..4 {
        play(&c, &mut s, 0, "t_hit", Some(egg));
    }
    assert!(!s.enemies[1].alive);
    assert!(s.heroes[0].hand.iter().any(|x| x.def == "koschei_needle"));
}

#[test]
fn breaking_the_needle_starts_the_last_breath() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &["koschei_needle"])], &ids(&["koschei"]), 7);
    s.heroes[0].hp = 500;
    s.heroes[0].max_hp = 500;
    play(&c, &mut s, 0, "koschei_needle", None);
    assert_eq!(s.enemies[0].hp, 240, "mortal, at two fifths of his life");
    assert!(!s.enemies[0].flags.contains_key("immortal"));
    // five turns of breath, then everyone falls
    for turn in 1..=5 {
        assert_eq!(s.phase, Phase::Heroes, "turn {turn}");
        end_round(&c, &mut s);
    }
    assert_eq!(s.phase, Phase::Lost);
}

#[test]
fn a_mortal_koschei_can_be_killed() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &["koschei_needle", "t_hit"])], &ids(&["koschei"]), 7);
    let k = foe(&s, 0);
    play(&c, &mut s, 0, "koschei_needle", None);
    s.enemies[0].hp = 5;
    play(&c, &mut s, 0, "t_hit", Some(k));
    assert_eq!(s.phase, Phase::Won);
}

#[test]
fn the_duck_settles_every_third_turn() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &["t_hit"; 5])], &ids(&["death_duck"]), 7);
    let t = foe(&s, 0);
    end_round(&c, &mut s);
    end_round(&c, &mut s);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 60, "still on the wing");
    end_round(&c, &mut s);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 50, "settled: any blow lands");
    end_round(&c, &mut s);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, 50, "and away again");
}

#[test]
fn a_deathless_koschei_grows_angrier() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &[])], &ids(&["koschei"]), 7);
    s.heroes[0].hp = 9000;
    s.heroes[0].max_hp = 9000;
    let k = foe(&s, 0);
    for _ in 0..6 {
        end_round(&c, &mut s);
    }
    assert_eq!(s.status(k, StatusKind::Strength), 3);
}

// ---- act one elites and the Leshy ----

#[test]
fn every_blow_that_lands_feeds_the_shatuns_rage() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &["t_hit", "t_multi", "t_rot"])], &ids(&["shatun"]), 7);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.status(t, StatusKind::Strength), 2);
    play(&c, &mut s, 0, "t_multi", Some(t));
    assert_eq!(s.status(t, StatusKind::Strength), 8, "three more hits");
    play(&c, &mut s, 0, "t_rot", Some(t));
    end_round(&c, &mut s);
    assert_eq!(s.status(t, StatusKind::Strength), 8, "rot is not a blow");
}

#[test]
fn likhos_eye_draws_every_aimed_attack() {
    let c = content();
    let party = [hero("t_plain", &[]), hero("t_plain", &[]), hero("t_plain", &[])];
    let (mut s, _) = start(&c, &party, &ids(&["likho", "hitter"]), 7);
    let marked = s.enemies[0].intent.as_ref().unwrap().target.unwrap();
    end_round(&c, &mut s);
    assert!(s.heroes[marked as usize].flags.contains_key("marked"));
    assert!(s.heroes[marked as usize].flags.contains_key("blind"));
    for round in 0..4 {
        for e in s.living_enemies() {
            assert_eq!(e.intent.as_ref().unwrap().target, Some(marked), "round {round}: {} aims at the marked hero", e.def);
        }
        end_round(&c, &mut s);
        if s.phase != Phase::Heroes || s.heroes[marked as usize].downed {
            break;
        }
    }
    assert!(!s.heroes.iter().any(|h| h.flags.contains_key("blind") && h.slot != marked));
}

#[test]
fn the_poloz_holds_until_hurt_enough_or_the_turn_ends() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &["t_hit", "t_rot", "t_block"])], &ids(&["poloz"]), 7);
    let t = foe(&s, 0);
    end_round(&c, &mut s);
    assert!(s.heroes[0].flags.contains_key("coiled"));
    assert_eq!(try_play(&c, &mut s, 0, "t_hit", Some(t)), Err(ActionError::Coiled));
    play(&c, &mut s, 0, "t_block", None);
    end_round(&c, &mut s);
    assert!(!s.heroes[0].flags.contains_key("coiled"), "free again after one turn");

    // an ally breaks the hold by hurting the serpent
    let party = [hero("t_plain", &["t_hit"]), hero("t_plain", &["t_hit", "t_hit"])];
    let (mut s, _) = start(&c, &party, &ids(&["poloz"]), 7);
    let t = foe(&s, 0);
    let held = s.enemies[0].intent.as_ref().unwrap().target.unwrap();
    end_round(&c, &mut s);
    let free = 1 - held;
    assert_eq!(try_play(&c, &mut s, held, "t_hit", Some(t)), Err(ActionError::Coiled));
    play(&c, &mut s, free, "t_hit", Some(t));
    assert!(s.heroes[held as usize].flags.contains_key("coiled"), "10 of 15");
    if s.heroes[free as usize].hand.iter().any(|x| x.def == "t_hit") {
        play(&c, &mut s, free, "t_hit", Some(t));
        assert!(!s.heroes[held as usize].flags.contains_key("coiled"));
        play(&c, &mut s, held, "t_hit", Some(t));
    }
}

#[test]
fn the_leshy_tangles_paths_and_grows_at_half_health() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &["t_hit"])], &ids(&["leshy", "dummy"]), 7);
    s.heroes[0].hp = 9000;
    s.heroes[0].max_hp = 9000;
    let leshy = foe(&s, 0);
    // tangled paths is the second move of his pattern
    end_round(&c, &mut s);
    end_round(&c, &mut s);
    assert_eq!(c.enemies["leshy"].moves[s.enemies[0].intent.as_ref().unwrap().mv].id, "tangled_paths");
    end_round(&c, &mut s);
    assert!(s.heroes[0].flags.contains_key("confused"));
    // an aimed blow now lands on whoever the forest decides
    let before: i32 = s.living_enemies().map(|e| e.hp + e.block).sum();
    play(&c, &mut s, 0, "t_hit", Some(leshy));
    let after: i32 = s.living_enemies().map(|e| e.hp + e.block).sum();
    assert_eq!(before - after, 10, "the blow still lands somewhere");
    end_round(&c, &mut s);
    assert!(!s.heroes[0].flags.contains_key("confused"));

    // worn to half, he changes for good
    s.enemies[0].hp = 100;
    end_round(&c, &mut s);
    assert!(s.enemies[0].flags.contains_key("phase2"));
    let mv = s.enemies[0].intent.as_ref().unwrap().mv;
    assert_eq!(c.enemies["leshy"].moves[mv].id, "towering_stomp");
    s.enemies[0].hp = 280;
    end_round(&c, &mut s);
    assert!(s.enemies[0].flags.contains_key("phase2"), "there is no way back from the second phase");
}

// ---- acts two and three ----

fn set_flag(s: &mut Combat, slot: usize, flag: &str, n: i32) {
    s.heroes[slot].flags.insert(flag.into(), n);
}

#[test]
fn the_upyr_heals_by_half_of_what_it_draws() {
    let (c, mut s) = solo(&[], &["upyr"]);
    s.enemies[0].hp = 20;
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 40);
    assert_eq!(s.enemies[0].hp, 25);
}

#[test]
fn a_soul_of_nav_strikes_half_through_any_shield() {
    let (c, mut s) = solo(&[], &["nav_soul"]);
    s.heroes[0].block = 50;
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 44);
}

#[test]
fn the_igosha_cannot_be_aimed_at_while_others_stand() {
    let (c, mut s) = solo(&["t_hit", "t_hit"], &["igosha", "weakling"]);
    let (ig, other) = (foe(&s, 0), foe(&s, 1));
    assert_eq!(try_play(&c, &mut s, 0, "t_hit", Some(ig)), Err(ActionError::Hidden));
    s.enemies[1].hp = 1;
    play(&c, &mut s, 0, "t_hit", Some(other));
    assert!(!s.enemies[1].alive);
    play(&c, &mut s, 0, "t_hit", Some(ig));
}

#[test]
fn the_kostomakha_gets_up_once() {
    let (c, mut s) = solo(&["t_hit", "t_hit"], &["kostomakha"]);
    let k = foe(&s, 0);
    s.enemies[0].hp = 1;
    play(&c, &mut s, 0, "t_hit", Some(k));
    assert!(s.enemies[0].alive);
    assert_eq!(s.enemies[0].hp, 0, "bones first, up on its turn");
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].hp, 27);
    s.enemies[0].hp = 1;
    play(&c, &mut s, 0, "t_hit", Some(k));
    assert!(!s.enemies[0].alive);
}

#[test]
fn a_friend_pulls_a_hero_out_of_the_mire() {
    let (c, mut s) = duo(&["t_energy"], &["t_block"], &["dummy"]);
    set_flag(&mut s, 1, "mired", 1);
    assert_eq!(try_play(&c, &mut s, 1, "t_block", None), Err(ActionError::Held));
    play(&c, &mut s, 0, "t_energy", Some(UnitRef::Hero(1)));
    play(&c, &mut s, 1, "t_block", None);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[1].hp, 50);
}

#[test]
fn the_mire_takes_its_due_from_a_hero_left_in_it() {
    let (c, mut s) = duo(&[], &[], &["dummy"]);
    set_flag(&mut s, 1, "mired", 1);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[1].hp, 42);
    assert!(!s.heroes[1].flags.contains_key("mired"));
}

#[test]
fn the_bolotnik_never_takes_the_whole_party() {
    let (c, mut s) = duo(&[], &[], &["bolotnik"]);
    end_round(&c, &mut s);
    assert_eq!(s.heroes.iter().filter(|h| h.flags.contains_key("mired")).count(), 1);
}

#[test]
fn a_charmed_attack_turns_on_a_friend() {
    let (c, mut s) = duo(&["t_hit", "t_hit"], &[], &["dummy"]);
    let d = foe(&s, 0);
    set_flag(&mut s, 0, "charmed", 1);
    play(&c, &mut s, 0, "t_hit", Some(d));
    assert_eq!(s.enemies[0].hp, s.enemies[0].max_hp);
    assert!(s.heroes[1].hp < 50);
    // the charm is spent: the next blow lands where it was aimed
    play(&c, &mut s, 0, "t_hit", Some(d));
    assert!(s.enemies[0].hp < s.enemies[0].max_hp);
}

#[test]
fn alone_a_charmed_attack_is_wasted() {
    let (c, mut s) = solo(&["t_hit"], &["dummy"]);
    let d = foe(&s, 0);
    set_flag(&mut s, 0, "charmed", 1);
    play(&c, &mut s, 0, "t_hit", Some(d));
    assert_eq!(s.enemies[0].hp, s.enemies[0].max_hp);
    assert_eq!(s.heroes[0].hp, 50);
}

#[test]
fn frozen_cards_are_the_leftmost_ones() {
    let (c, mut s) = solo(&["t_block"; 5], &["dummy"]);
    set_flag(&mut s, 0, "frozen", 2);
    let (first, third) = (s.heroes[0].hand[0].uid, s.heroes[0].hand[2].uid);
    assert_eq!(apply(&c, &mut s, 0, &CombatAction::Play { card: first, target: None }), Err(ActionError::Frozen));
    assert!(apply(&c, &mut s, 0, &CombatAction::Play { card: third, target: None }).is_ok());
}

#[test]
fn a_sleeper_misses_the_turn_and_wakes() {
    let (c, mut s) = solo(&["t_block"; 10], &["dummy"]);
    set_flag(&mut s, 0, "asleep", 1);
    assert_eq!(try_play(&c, &mut s, 0, "t_block", None), Err(ActionError::Held));
    end_round(&c, &mut s);
    play(&c, &mut s, 0, "t_block", None);
}

#[test]
fn dragged_under_and_shaken_heroes_start_the_turn_short() {
    let (c, mut s) = solo(&["t_block"; 12], &["dummy"]);
    set_flag(&mut s, 0, "energy_less", 1);
    set_flag(&mut s, 0, "draw_less", 2);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].energy, 2);
    assert_eq!(s.heroes[0].hand.len(), 3);
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].energy, 3);
    assert_eq!(s.heroes[0].hand.len(), 5);
}

#[test]
fn the_fever_burns_and_moves_on() {
    let (c, mut s) = duo(&[], &[], &["dummy"]);
    s.heroes[0].hand.push(Card { uid: 9999, def: "fever".into(), upgraded: false, free: false });
    let has = |h: &Hero| h.hand.iter().chain(&h.draw).chain(&h.discard).any(|x| x.def == "fever");
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, 47);
    assert!(!has(&s.heroes[0]));
    assert!(has(&s.heroes[1]));
    end_round(&c, &mut s);
    assert_eq!(s.heroes[1].hp, 47);
    assert!(has(&s.heroes[0]));
    assert!(!has(&s.heroes[1]));
}

#[test]
fn the_vurdalak_elder_raises_its_dead() {
    let (c, mut s) = solo(&["t_hit"], &["vurdalak_elder", "weakling"]);
    let w = foe(&s, 1);
    s.enemies[1].hp = 1;
    play(&c, &mut s, 0, "t_hit", Some(w));
    assert!(!s.enemies[1].alive);
    for _ in 0..3 {
        end_round(&c, &mut s);
    }
    assert!(s.enemies[1].alive);
    assert_eq!(s.enemies[1].hp, 5);
    assert!(s.enemies[1].intent.is_some());
}

#[test]
fn maras_winter_lasts_while_her_crown_stands() {
    let (c, mut s) = solo(&["t_hit"; 12], &["mara", "ice_crown"]);
    // tough enough that her blows take no limb, which would cost a card of its own
    s.heroes[0].max_hp = 1000;
    s.heroes[0].hp = 1000;
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].flags.get("winter"), Some(&1));
    assert_eq!(s.heroes[0].flags.get("frozen"), Some(&2));
    assert_eq!(s.heroes[0].hand.len(), 4);
    let crown = foe(&s, 1);
    s.enemies[1].hp = 1;
    s.enemies[1].block = 0;
    let (first, third) = (s.heroes[0].hand[0].uid, s.heroes[0].hand[2].uid);
    assert_eq!(apply(&c, &mut s, 0, &CombatAction::Play { card: first, target: Some(crown) }), Err(ActionError::Frozen));
    apply(&c, &mut s, 0, &CombatAction::Play { card: third, target: Some(crown) }).unwrap();
    assert!(!s.enemies[1].alive);
    assert!(!s.heroes[0].flags.contains_key("winter"));
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hand.len(), 5);
    assert!(!s.heroes[0].flags.contains_key("frozen"));
}

#[test]
fn koschei_hunts_whoever_hurt_the_chain_most() {
    let c = content();
    let party = [hero("t_plain", &[]), hero("t_plain", &["t_hit"; 5]), hero("t_plain", &["t_hit"; 5])];
    let (mut s, _) = start(&c, &party, &ids(&["koschei", "death_oak"]), 7);
    let oak = foe(&s, 1);
    play(&c, &mut s, 1, "t_hit", Some(oak));
    play(&c, &mut s, 2, "t_hit", Some(oak));
    play(&c, &mut s, 2, "t_hit", Some(oak));
    for _ in 0..3 {
        end_round(&c, &mut s);
        if let Some(t) = s.enemies[0].intent.as_ref().unwrap().target {
            assert_eq!(t, 2);
        }
    }
}

#[test]
fn the_needle_goes_from_hand_to_hand_and_koschei_follows_it() {
    let c = content();
    let party = [hero("t_plain", &["koschei_needle"]), hero("t_plain", &[])];
    let (mut s, _) = start(&c, &party, &ids(&["koschei", "dummy"]), 7);
    s.enemies[0].flags.insert("risen".into(), 1);
    let uid = s.heroes[0].hand[0].uid;
    assert_eq!(apply(&c, &mut s, 0, &CombatAction::Pass { card: uid, to: 0 }), Err(ActionError::CannotPass));
    apply(&c, &mut s, 0, &CombatAction::Pass { card: uid, to: 1 }).unwrap();
    assert_eq!(s.heroes[0].energy, 2);
    assert!(s.heroes[1].hand.iter().any(|x| x.uid == uid));
    // he rises, calls two guards, and from then on aims at the holder
    end_round(&c, &mut s);
    assert_eq!(s.living_enemies().filter(|e| e.def == "koschei_guard").count(), 0);
    end_round(&c, &mut s);
    assert_eq!(s.living_enemies().filter(|e| e.def == "koschei_guard").count(), 2);
    assert_eq!(s.enemies[0].intent.as_ref().unwrap().target, Some(1), "the needle is retained in hand 1");
    apply(&c, &mut s, 1, &CombatAction::Pass { card: uid, to: 0 }).unwrap();
    end_round(&c, &mut s);
}

#[test]
fn only_the_needle_can_be_handed_over() {
    let (c, mut s) = duo(&["t_block"], &[], &["dummy"]);
    let uid = s.heroes[0].hand[0].uid;
    assert_eq!(apply(&c, &mut s, 0, &CombatAction::Pass { card: uid, to: 1 }), Err(ActionError::CannotPass));
}

#[test]
fn a_lone_hero_faces_a_shorter_chain() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &["t_hit_all"; 5])], &ids(&["koschei", "death_duck"]), 7);
    s.enemies[1].flags.remove("flying");
    s.enemies[1].hp = 1;
    play(&c, &mut s, 0, "t_hit_all", None);
    let egg = s.living_enemies().find(|e| e.def == "death_egg_solo").expect("the thin-shelled egg");
    assert_eq!(egg.block, 20);
    let enc = c.encounters.values().find(|e| e.id == "a4_koschei").unwrap();
    assert_eq!(enc.solo, ids(&["koschei", "death_oak_chest"]));
}

#[test]
fn curses_that_spoil_the_next_turn() {
    let (c, mut s) = solo(&["t_block"; 12], &["dummy"]);
    s.heroes[0].hand.push(Card { uid: 9001, def: "evil_dream".into(), upgraded: false, free: false });
    s.heroes[0].hand.push(Card { uid: 9002, def: "ice_shard".into(), upgraded: false, free: false });
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].energy, 2);
    // the dream is back in the deck and may be drawn again; the shard melted for good
    let h = &s.heroes[0];
    assert!(!h.hand.iter().chain(&h.draw).chain(&h.discard).any(|x| x.def == "ice_shard"));
    assert_eq!(h.hand.len(), 4);
}

#[test]
fn on_the_tenth_circle_the_needle_frees_koscheis_shadow() {
    let c = content();
    let opts = CombatOpts { shadow: true, ..CombatOpts::default() };
    let party = [hero("t_plain", &["koschei_needle", "t_hit", "t_block"]), hero("t_plain", &[])];
    let (mut s, _) = start_with(&c, &party, &ids(&["koschei"]), 7, opts);
    s.heroes[0].energy = 9;
    play(&c, &mut s, 0, "koschei_needle", None);
    let shadow = s.living_enemies().find(|e| e.def == "koschei_shadow").expect("the shadow is loose").uid;
    assert!(s.enemies.iter().any(|e| e.uid == shadow && e.alive));
    // alone with the shadow: one hero ends on a block card, the other plays nothing
    let party = [hero("t_plain", &["t_hit", "t_block"]), hero("t_plain", &[])];
    let (mut duel, _) = start(&c, &party, &ids(&["koschei_shadow"]), 7);
    let it = foe(&duel, 0);
    play(&c, &mut duel, 0, "t_hit", Some(it));
    play(&c, &mut duel, 0, "t_block", None);
    end_round(&c, &mut duel);
    assert_eq!(duel.heroes[0].hp, 50, "a mirrored shield shields the shadow, and hurts nobody");
    assert_eq!(duel.heroes[1].hp, 42, "the idle hero is struck");
    // without the tenth circle there is no shadow
    let (mut plain, _) = start(&c, &[hero("t_plain", &["koschei_needle"])], &ids(&["koschei"]), 7);
    play(&c, &mut plain, 0, "koschei_needle", None);
    assert!(!plain.enemies.iter().any(|e| e.def == "koschei_shadow"));
}

#[test]
fn the_shadow_can_mirror_every_card_in_the_game() {
    let c = content();
    let opts = CombatOpts { shadow: true, ..CombatOpts::default() };
    for def in c.cards.values().filter(|d| !d.hero.is_empty() && !d.id.starts_with("t_")) {
        let party = [hero(&def.hero, &[]), hero("t_plain", &[])];
        let (mut s, _) = start_with(&c, &party, &ids(&["koschei_shadow", "dummy"]), 3, opts.clone());
        s.heroes[0].hp = 500;
        s.heroes[0].max_hp = 500;
        s.log.push(Played { player: 0, card: def.id.clone(), upgraded: true, kind: def.kind });
        end_round(&c, &mut s);
        assert!(s.heroes[0].hp <= 500, "{}", def.id);
        assert_eq!(s.phase, Phase::Heroes, "{}", def.id);
    }
}

#[test]
fn bones_lie_a_turn_before_they_get_up() {
    let (c, mut s) = solo(&["t_hit", "t_hit"], &["bones"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.phase, nav_core::Phase::Heroes, "a heap of bones is not a won fight");
    assert_eq!(s.status(t, S::Strength), 0);
    let e = &s.enemies[0];
    assert!(e.alive && e.hp == 0 && e.flags.contains_key(nav_core::state::BONES));
    assert_eq!(e.intent.as_ref().map(|i| i.mv), Some(1), "it shows that it is getting up");
    assert!(try_play(&c, &mut s, 0, "t_hit", Some(t)).is_err(), "bones cannot be struck");
    let hp = s.heroes[0].hp;
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].hp, hp, "getting up is all it does that turn");
    assert_eq!(s.enemies[0].hp, 5, "up at half");
    assert!(!s.enemies[0].flags.contains_key(nav_core::state::BONES));
}


#[test]
fn a_hand_thrown_away_counts_and_fires_what_waits_for_it() {
    let (c, mut s) = solo(&["t_on_discard", "t_toss", "t_sly", "t_block", "t_block"], &["dummy"]);
    play(&c, &mut s, 0, "t_on_discard", None);
    let before = s.enemies[0].hp;
    play(&c, &mut s, 0, "t_toss", None);
    let h = &s.heroes[0];
    assert!(h.hand.is_empty());
    // three thrown: 2 each, the sly card's 7, and the power's block for each
    assert_eq!(s.enemies[0].hp, before - 7 - 3 * 2);
    assert_eq!(h.block, 3 * 2);
    assert_eq!(h.discard.len(), 4);
}

#[test]
fn the_end_of_a_turn_is_no_discard() {
    let (c, mut s) = solo(&["t_sly", "t_sly", "t_sly", "t_sly", "t_sly"], &["dummy"]);
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].hp, 100);
}

#[test]
fn burnt_cards_go_for_good_and_powers_hear_them() {
    let (c, mut s) = solo(&["t_on_exhaust", "t_burn", "t_block", "t_block", "t_block"], &["dummy"]);
    play(&c, &mut s, 0, "t_on_exhaust", None);
    play(&c, &mut s, 0, "t_burn", None);
    let h = &s.heroes[0];
    assert_eq!(h.exhaust.len(), 2);
    assert_eq!(h.hand.len(), 1);
    assert_eq!(h.block, 6);
}

#[test]
fn a_card_is_called_back_from_the_discard_pile() {
    let (c, mut s) = solo(&["t_recall", "t_toss1", "t_cost3", "t_cost3", "t_cost3"], &["dummy"]);
    play(&c, &mut s, 0, "t_toss1", None);
    // the player picks what goes
    let pick = s.heroes[0].hand.iter().find(|c| c.def == "t_cost3").unwrap().uid;
    apply(&c, &mut s, 0, &CombatAction::Discard { card: pick }).unwrap();
    assert_eq!(s.heroes[0].hand.len(), 3);
    play(&c, &mut s, 0, "t_recall", None);
    // the recall card is in hand no more, and one card came back
    assert_eq!(s.heroes[0].hand.len(), 3);
}


#[test]
fn a_discard_waits_for_the_player_to_pick_and_the_card_goes_on_after() {
    let (c, mut s) = solo(&["t_toss_pick", "t_sly", "t_block", "t_block", "t_block"], &["dummy"]);
    let before = s.enemies[0].hp;
    play(&c, &mut s, 0, "t_toss_pick", None);
    assert!(s.heroes[0].choosing.is_some());
    // nothing else is done meanwhile
    assert_eq!(try_play(&c, &mut s, 0, "t_block", None), Err(ActionError::Choosing));
    assert_eq!(s.enemies[0].hp, before, "the blow waits for the pick");
    let sly = s.heroes[0].hand.iter().find(|c| c.def == "t_sly").unwrap().uid;
    apply(&c, &mut s, 0, &CombatAction::Discard { card: sly }).unwrap();
    let block = s.heroes[0].hand.iter().find(|c| c.def == "t_block").unwrap().uid;
    apply(&c, &mut s, 0, &CombatAction::Discard { card: block }).unwrap();
    assert!(s.heroes[0].choosing.is_none());
    // the sly card's 7 on being thrown, then 3 for each of the two thrown
    assert_eq!(s.enemies[0].hp, before - 7 - 6);
    assert_eq!(s.heroes[0].hand.len(), 2);
}

#[test]
fn a_discard_of_more_than_the_hand_takes_it_all_without_asking() {
    let (c, mut s) = solo(&["t_toss_pick", "t_block"], &["dummy"]);
    play(&c, &mut s, 0, "t_toss_pick", None);
    assert!(s.heroes[0].choosing.is_none());
    assert!(s.heroes[0].hand.is_empty());
}


#[test]
fn a_fight_drawn_out_gathers_the_dark_on_the_heroes() {
    let (c, mut s) = solo(&["t_block", "t_block", "t_block", "t_block", "t_block"], &["dummy"]);
    for _ in 0..STALL_ROUND {
        end_round(&c, &mut s);
    }
    let me = UnitRef::Hero(0);
    assert_eq!(s.status(me, StatusKind::Gloom), 1, "round {}", s.round);
    let hp = s.heroes[0].hp;
    end_round(&c, &mut s);
    assert_eq!(s.status(me, StatusKind::Gloom), 2);
    assert!(s.heroes[0].hp < hp, "the dark takes its toll");
    // two stacks: block is a fifth less
    play(&c, &mut s, 0, "t_block", None);
    assert_eq!(s.heroes[0].block, 4);
}

#[test]
fn a_boss_fight_holds_the_dark_off_longer() {
    let (c, mut s) = solo(&["t_block", "t_block", "t_block", "t_block", "t_block"], &["dummy"]);
    s.stall = nav_core::combat::STALL_BOSS;
    for _ in 0..STALL_ROUND {
        end_round(&c, &mut s);
    }
    let me = UnitRef::Hero(0);
    assert_eq!(s.status(me, StatusKind::Gloom), 0, "round {}", s.round);
    for _ in STALL_ROUND..nav_core::combat::STALL_BOSS {
        end_round(&c, &mut s);
    }
    assert_eq!(s.status(me, StatusKind::Gloom), 1, "round {}", s.round);
}


#[test]
fn only_a_werewolf_turns_into_the_beast() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_plain", &["t_shift", "t_block"])], &ids(&["dummy"]), 7);
    play(&c, &mut s, 0, "t_shift", None);
    assert_eq!(s.heroes[0].form, Form::Human, "an echo of the wolf's card changes no one else");
    play(&c, &mut s, 0, "t_block", None);
    assert_eq!(s.heroes[0].block, 5);
}


#[test]
fn the_moon_gathered_as_a_man_is_the_beasts_fury() {
    let c = content();
    let (mut s, _) = start(&c, &[hero("t_moon", &["t_hit", "t_hit", "t_hit", "t_hit", "t_hit", "t_shift"])], &ids(&["dummy"]), 7);
    s.heroes[0].energy = 10;
    // the five blows in hand, the shift kept for after
    let h = &mut s.heroes[0];
    let mut all: Vec<Card> = h.hand.drain(..).chain(h.draw.drain(..)).collect();
    let shift = all.iter().position(|c| c.def == "t_shift").unwrap();
    let shift = all.remove(shift);
    h.hand = all;
    for _ in 0..5 {
        let foe = foe(&s, 0);
        play(&c, &mut s, 0, "t_hit", Some(foe));
    }
    assert_eq!(s.heroes[0].form, Form::Beast);
    assert_eq!(s.status(UnitRef::Hero(0), StatusKind::Strength), 3, "a full moon: three");
    s.heroes[0].hand.push(shift);
    play(&c, &mut s, 0, "t_shift", None);
    assert_eq!(s.heroes[0].form, Form::Human);
    assert_eq!(s.status(UnitRef::Hero(0), StatusKind::Strength), 0, "the fury goes with the beast");
}

#[test]
fn every_werewolf_card_has_a_beast_side() {
    let c = content();
    for d in c.cards.values().filter(|d| d.hero == "volkolak" && d.kind == CardType::Attack) {
        assert!(d.two_sided(false), "{}", d.id);
    }
}

#[test]
fn the_hoarder_takes_a_share_and_throws_it_away_for_good() {
    let (c, mut s) = solo(&[], &["kladovik"]);
    s.heroes[0].gold = 100;
    end_round(&c, &mut s);
    assert_eq!(s.heroes[0].gold_lost, 30, "a third of the purse");
    let hp = s.heroes[0].hp;
    end_round(&c, &mut s);
    // three coins, each 3 and a tenth of the thirty thrown
    assert_eq!(hp - s.heroes[0].hp, 18);
    assert!(s.enemies[0].stolen.is_empty(), "the gold is scattered");
    assert_eq!(s.heroes[0].gold_lost, 30, "and gone for good");
}

#[test]
fn test_vulnerable_comparison_0_vs_3_stacks() {
    let (_c, mut s0) = solo(&[], &["hitter"]);
    let (c, mut s3) = solo(&[], &["hitter"]);

    // Hero in s0 has 0 Vulnerable, Hero in s3 has 3 Vulnerable
    s3.heroes[0].statuses.insert(StatusKind::Vulnerable, 3);

    // Check displayed intent damage:
    let intent0 = bot::intent_damage(&c, &s0, &s0.enemies[0]);
    let intent3 = bot::intent_damage(&c, &s3, &s3.enemies[0]);
    assert_eq!(intent0, Some((10, 1)), "Displayed intent for 0 vulnerable should be 10");
    assert_eq!(intent3, Some((15, 1)), "Displayed intent for 3 vulnerable should be 15 (+50%)");

    // Check actual damage received:
    let hp0_before = s0.heroes[0].hp;
    end_round(&c, &mut s0);
    let dmg0 = hp0_before - s0.heroes[0].hp;

    let hp3_before = s3.heroes[0].hp;
    end_round(&c, &mut s3);
    let dmg3 = hp3_before - s3.heroes[0].hp;

    assert_eq!(dmg0, 10, "Hero with 0 vulnerable takes exactly 10 damage");
    assert_eq!(dmg3, 15, "Hero with 3 vulnerable takes exactly 15 damage (+50%)");

    // And after 1 enemy attack round, 3 vulnerable decays to 2 (since it was present before EnemyPhase)
    assert_eq!(s3.status(UnitRef::Hero(0), StatusKind::Vulnerable), 2);
}

#[test]
fn test_vulnerable_applied_by_enemy_does_not_burn_in_same_round() {
    let (c, mut s) = solo(&[], &["poloz"]);
    // Poloz pattern: move 0 is coil (6 dmg), move 1 is constrict (12 dmg, 1 Vulnerable), move 2 is venom (9 dmg)
    // Round 1: coil
    end_round(&c, &mut s);
    assert_eq!(s.status(UnitRef::Hero(0), StatusKind::Vulnerable), 0);

    // Round 2: constrict (12 dmg, 1 Vulnerable)
    let hp_before_constrict = s.heroes[0].hp;
    end_round(&c, &mut s);
    assert_eq!(hp_before_constrict - s.heroes[0].hp, 12);
    // Vulnerable must NOT have burned at the end of that same round!
    assert_eq!(s.status(UnitRef::Hero(0), StatusKind::Vulnerable), 1, "Vulnerable 1 applied in EnemyPhase remains for next round");

    // Next round (Round 3): hero has Vulnerable 1. Intent damage for venom (base 9) must show 9 * 3 / 2 = 13
    let next_intent = bot::intent_damage(&c, &s, &s.enemies[0]);
    assert_eq!(next_intent, Some((13, 1)), "Intent shows 13 (9 * 1.5) because hero is vulnerable");

    // Round 3 execution: hero takes 13 damage
    let hp_before_venom = s.heroes[0].hp;
    end_round(&c, &mut s);
    assert_eq!(hp_before_venom - s.heroes[0].hp, 13, "Hero took 13 damage from venom under Vulnerable");
    // After Round 3, the Vulnerable stack that was active during the attack finally expires
    assert_eq!(s.status(UnitRef::Hero(0), StatusKind::Vulnerable), 0, "Vulnerable expires after taking the attack");
}

#[test]
fn test_switcheroo_with_one_enemy_does_not_skip_attack() {
    let (c, mut s) = solo(&["sk_switcheroo"], &["hitter"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "sk_switcheroo", Some(t));
    let hp_before = s.heroes[0].hp;
    end_round(&c, &mut s);
    // With 1 enemy, the attack is NOT cancelled! Enemy hits hero for 10.
    assert_eq!(hp_before - s.heroes[0].hp, 10, "Attack is not cancelled when only 1 enemy is present");
}

#[test]
fn test_switcheroo_with_two_enemies_redirects_to_other_enemy() {
    let (c, mut s) = solo(&["sk_switcheroo"], &["hitter", "dummy"]);
    let t = foe(&s, 0); // hitter
    play(&c, &mut s, 0, "sk_switcheroo", Some(t));
    let hero_hp_before = s.heroes[0].hp;
    let dummy_hp_before = s.enemies[1].hp;
    end_round(&c, &mut s);
    // Hero took 0 damage
    assert_eq!(s.heroes[0].hp, hero_hp_before, "Hero takes 0 damage because attack was redirected");
    // Dummy took 10 damage from hitter!
    assert_eq!(dummy_hp_before - s.enemies[1].hp, 10, "Other enemy took 10 damage from hitter");
}
