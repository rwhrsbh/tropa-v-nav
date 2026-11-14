//! The new bosses of the first two acts: each one's trick works as its trait says.
mod common;
use common::*;
use nav_core::combat::{house_rule_kind, HOUSE_RULE_COST};
use nav_core::*;

fn deck(card: &str) -> Vec<&str> {
    vec![card; 10]
}

#[test]
fn the_idolishche_eats_what_is_left_in_hand() {
    let (c, mut s) = solo(&deck("t_block"), &["idolishche"]);
    s.enemies[0].hp = 200;
    let held = s.heroes[0].hand.len() as i32;
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].hp, 200 + 3 * held, "three for every card still held");
}

#[test]
fn the_idolishche_swallows_the_dearest_card_and_grows() {
    let mut cards = deck("t_block");
    cards.extend(["t_cost2"; 3]);
    let (c, mut s) = solo(&cards, &["idolishche"]);
    end_round(&c, &mut s);
    let h = &s.heroes[0];
    assert!(h.exhaust.iter().any(|x| x.def == "t_cost2") || !h.draw.iter().chain(&h.discard).any(|x| x.def == "t_cost2"), "the dear card is gone");
    assert_eq!(s.status(foe(&s, 0), StatusKind::Strength), 2, "grown by its cost");
}

#[test]
fn vikhors_wind_rides_his_blows_and_skills_lull_it() {
    let (c, mut s) = solo(&deck("t_block"), &["vikhor"]);
    assert_eq!(s.enemies[0].flags.get("gale"), Some(&1));
    play(&c, &mut s, 0, "t_block", None);
    assert_eq!(s.enemies[0].flags.get("gale"), Some(&0), "a skill lulls the wind");
    let (c, mut s) = solo(&deck("t_hit"), &["vikhor"]);
    s.enemies[0].flags.insert("gale".into(), 5);
    let hp = s.heroes[0].hp;
    end_round(&c, &mut s);
    assert_eq!(hp - s.heroes[0].hp, 7 + 5, "the howl carries the wind");
    assert_eq!(s.enemies[0].flags.get("gale"), Some(&6), "and it rises with the round");
}

#[test]
fn gore_takes_half_while_a_gore_card_is_held() {
    let (c, mut s) = solo(&deck("t_hit"), &["gore"]);
    let hp = s.enemies[0].hp;
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(hp - s.enemies[0].hp, 10);
    s.heroes[0].hand.push(Card { uid: 9999, def: "gore".into(), upgraded: false, free: false });
    let hp = s.enemies[0].hp;
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(hp - s.enemies[0].hp, 5, "held grief halves the blow");
    play(&c, &mut s, 0, "gore", None);
    let hp = s.enemies[0].hp;
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(hp - s.enemies[0].hp, 10, "lived through, it no longer shields him");
}

#[test]
fn the_domovoy_forbids_a_kind_of_card_each_round() {
    let (c, mut s) = solo(&deck("t_hit"), &["domovoy"]);
    assert_eq!(house_rule_kind(s.enemies[0].flags["house_rule"]), CardType::Power, "the first round forbids powers");
    let hp = s.heroes[0].hp;
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.heroes[0].hp, hp, "an attack is allowed");
    end_round(&c, &mut s);
    assert_eq!(house_rule_kind(s.enemies[0].flags["house_rule"]), CardType::Attack);
    let hp = s.heroes[0].hp;
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(hp - s.heroes[0].hp, HOUSE_RULE_COST, "now it costs blood");
}
