//! The new wave of foes: each one's trick works as its trait says.
mod common;
use common::*;
use nav_core::*;

fn deck(card: &str) -> Vec<&str> {
    vec![card; 10]
}

#[test]
fn the_biryuk_bites_harder_alone() {
    let (c, mut s) = solo(&deck("t_block"), &["biryuk", "dummy"]);
    let hp = s.heroes[0].hp;
    end_round(&c, &mut s);
    assert_eq!(hp - s.heroes[0].hp, 7, "with a friend beside it, a plain bite");
    let (c, mut s) = solo(&deck("t_block"), &["biryuk"]);
    end_round(&c, &mut s);
    assert_eq!(hp - s.heroes[0].hp, 10, "alone, half again");
}

#[test]
fn the_polevik_taxes_the_first_card_of_a_turn() {
    let (c, mut s) = solo(&deck("t_cost2"), &["polevik"]);
    assert_eq!(s.heroes[0].energy, 3);
    play(&c, &mut s, 0, "t_cost2", None);
    assert_eq!(s.heroes[0].energy, 0, "the first card costs one more");
    let (c, mut s) = solo(&deck("t_block"), &["polevik"]);
    play(&c, &mut s, 0, "t_block", None);
    play(&c, &mut s, 0, "t_block", None);
    assert_eq!(s.heroes[0].energy, 3, "a free card stays free");
}

#[test]
fn tugarins_wings_halve_the_first_three_blows() {
    let (c, mut s) = solo(&deck("t_hit"), &["tugarin"]);
    let hp = s.enemies[0].hp;
    for _ in 0..4 {
        let t = foe(&s, 0);
        play(&c, &mut s, 0, "t_hit", Some(t));
    }
    assert_eq!(hp - s.enemies[0].hp, 5 * 3 + 10);
}

#[test]
fn a_heavy_blow_cracks_a_black_rune() {
    let (c, mut s) = solo(&deck("t_hit"), &["black_volhv"]);
    assert_eq!(s.enemies[0].flags.get("black_runes"), Some(&1));
    let t = foe(&s, 0);
        play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].flags.get("black_runes"), Some(&0), "ten through the shield cracks one");
}

#[test]
fn the_shade_halves_healing() {
    let (c, mut s) = solo(&deck("t_heal"), &["ancestor_shade"]);
    s.heroes[0].hp = 20;
    play(&c, &mut s, 0, "t_heal", None);
    assert_eq!(s.heroes[0].hp, 24);
}

#[test]
fn the_changeling_gives_back_what_it_stole() {
    let (c, mut s) = solo(&deck("t_hit"), &["podmenysh"]);
    let before = s.heroes[0].draw.len();
    end_round(&c, &mut s);
    let h = &s.heroes[0];
    assert!(h.exhaust.iter().any(|x| x.def == "t_hit"), "a card was carried off");
    assert!(h.draw.iter().chain(&h.hand).chain(&h.discard).any(|x| x.def == "podkidysh"), "and a foundling left: {before}");
    while s.enemies[0].alive {
        let t = foe(&s, 0);
        play(&c, &mut s, 0, "t_hit", Some(t));
    }
    let h = &s.heroes[0];
    assert!(!h.exhaust.iter().any(|x| x.def == "t_hit"), "the stolen card is back");
}

#[test]
fn the_zhivoglot_swallows_a_shield() {
    let (c, mut s) = solo(&deck("t_block"), &["zhivoglot"]);
    play(&c, &mut s, 0, "t_block", None);
    play(&c, &mut s, 0, "t_block", None);
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].block, 10, "both shields gone into it");
}

#[test]
fn a_twin_mourner_rises_while_her_sister_stands() {
    let (c, mut s) = solo(&deck("t_hit"), &["zhlya", "karna"]);
    s.enemies[0].hp = 5;
    let t = foe(&s, 0);
        play(&c, &mut s, 0, "t_hit", Some(t));
    assert!(!s.enemies[0].alive);
    assert_eq!(s.status(foe(&s, 1), StatusKind::Strength), 3, "her sister is enraged");
    end_round(&c, &mut s);
    end_round(&c, &mut s);
    assert!(s.enemies[0].alive, "she is up again");
    assert_eq!(s.enemies[0].hp, s.enemies[0].max_hp * 40 / 100);
}

#[test]
fn chernobogs_sun_climbs_unless_he_is_hurt() {
    let (c, mut s) = solo(&deck("t_block"), &["chernobog"]);
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].flags.get("eclipse"), Some(&2), "left alone, the sun climbs");
    let (c, mut s) = solo(&deck("t_hit"), &["chernobog"]);
    for _ in 0..3 {
        let t = foe(&s, 0);
        play(&c, &mut s, 0, "t_hit", Some(t));
    }
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].flags.get("eclipse"), Some(&1), "hurt hard, it is held down");
}

#[test]
fn the_lullaby_leaves_one_awake_and_a_friend_wakes_the_rest() {
    let (c, mut s) = duo(&deck("t_block"), &deck("t_energy"), &["alkonost"]);
    end_round(&c, &mut s);
    end_round(&c, &mut s);
    let asleep: Vec<u8> = s.heroes.iter().filter(|h| h.flags.contains_key("asleep")).map(|h| h.slot).collect();
    assert_eq!(asleep.len(), 1, "one of two sleeps");
    let sleeper = asleep[0];
    let waker = 1 - sleeper;
    if waker == 1 {
        play(&c, &mut s, 1, "t_energy", Some(UnitRef::Hero(0)));
        assert!(!s.heroes[0].flags.contains_key("asleep"), "a friend's card wakes them");
    }
}

#[test]
fn the_priest_gives_the_weakest_to_his_god() {
    let (c, mut s) = solo(&deck("t_block"), &["chernobog_priest", "weakling"]);
    end_round(&c, &mut s);
    end_round(&c, &mut s);
    end_round(&c, &mut s);
    assert!(!s.enemies[1].alive, "the weakling was given");
    assert!(s.status(foe(&s, 0), StatusKind::Strength) >= 3);
}
