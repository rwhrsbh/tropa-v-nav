//! The six heroes with their real cards and relics.

mod common;
use common::*;
use nav_core::*;
use StatusKind as S;

const ME: UnitRef = UnitRef::Hero(0);

/// A real hero with a chosen deck and their starting relic.
fn with(c: &Content, hero_id: &str, deck: &[&str]) -> HeroSetup {
    let d = &c.heroes[hero_id];
    HeroSetup { def: hero_id.into(), hp: d.max_hp, max_hp: d.max_hp, deck: deck.iter().map(|x| (x.to_string(), false)).collect(), relics: vec![d.relic.clone()], falls: 0, potions: vec![], lost_limbs: 0, lost_arms: 0, beheaded: false, walks_headless: false }
}

fn fight(hero_id: &str, deck: &[&str], enemies: &[&str]) -> (Content, Combat) {
    let c = content();
    let (mut s, _) = start(&c, &[with(&c, hero_id, deck)], &ids(enemies), 5);
    s.heroes[0].energy = 20;
    (c, s)
}

#[test]
fn every_hero_has_ten_cards_and_the_documented_hp() {
    let c = content();
    for (id, hp) in [("bogatyr", 85), ("vorozheya", 70), ("volhv", 72), ("volkolak", 80), ("znaharka", 68), ("skomoroh", 70)] {
        let h = &c.heroes[id];
        assert_eq!((h.max_hp, h.deck.len()), (hp, 10), "{id}");
    }
}

#[test]
fn every_hero_wins_the_first_fights_on_autopilot() {
    let c = content();
    for hero_id in c.heroes.keys().filter(|h| !h.starts_with("t_")) {
        for enemies in [vec!["grey_wolf", "grey_wolf"], vec!["kikimora"]] {
            let (mut s, _) = start(&c, &[HeroSetup::fresh(&c, hero_id).unwrap()], &ids(&enemies), 21);
            for _ in 0..600 {
                if s.phase != Phase::Heroes {
                    break;
                }
                let a = bot::choose(&c, &s, 0);
                apply(&c, &mut s, 0, &a).unwrap_or_else(|e| panic!("{hero_id}: illegal bot move {a:?}: {e:?}"));
            }
            assert_eq!(s.phase, Phase::Won, "{hero_id} vs {enemies:?}");
        }
    }
}

// ---- Vorozheya ----

#[test]
fn motanka_doll_rots_someone_at_the_start() {
    let (_, s) = fight("vorozheya", &[], &["dummy"]);
    assert_eq!(s.status(foe(&s, 0), S::Rot), 3);
}

#[test]
fn lifting_the_curse_cashes_in_every_mark() {
    let (c, mut s) = fight("vorozheya", &["vor_whisper", "vor_hex", "vor_black_candle", "vor_lift"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "vor_hex", Some(t));
    play(&c, &mut s, 0, "vor_black_candle", Some(t));
    assert_eq!(s.status(t, S::Hex), 4, "black candle doubles the Evil Eye");
    play(&c, &mut s, 0, "vor_whisper", Some(t));
    // 5 + 3 from the Evil Eye; rot 3 from the doll + 1
    assert_eq!((s.enemies[0].hp, s.status(t, S::Rot), s.status(t, S::Hex)), (92, 4, 3));
    play(&c, &mut s, 0, "vor_lift", Some(t));
    assert_eq!(s.enemies[0].hp, 92 - 3 * 7);
    assert_eq!((s.status(t, S::Rot), s.status(t, S::Hex)), (0, 0));
}

#[test]
fn pestilence_makes_rot_bite_twice_and_stay() {
    let (c, mut s) = fight("vorozheya", &["vor_pestilence"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "vor_pestilence", None);
    end_round(&c, &mut s);
    assert_eq!((s.enemies[0].hp, s.status(t, S::Rot)), (94, 3));
}

#[test]
fn transfer_moves_debuffs_to_the_enemy_and_blood_pact_feeds_an_ally() {
    let c = content();
    let party = [with(&c, "vorozheya", &["vor_transfer", "vor_blood_pact", "t_weak_me"]), hero("t_plain", &[])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 5);
    let t = foe(&s, 0);
    s.heroes[1].statuses.insert(S::Vulnerable, 2);
    play(&c, &mut s, 0, "t_weak_me", None);
    play(&c, &mut s, 0, "vor_transfer", Some(t));
    assert_eq!((s.status(ME, S::Weak), s.status(UnitRef::Hero(1), S::Vulnerable)), (0, 0));
    assert_eq!((s.status(t, S::Weak), s.status(t, S::Vulnerable)), (1, 2));
    play(&c, &mut s, 0, "vor_blood_pact", Some(UnitRef::Hero(1)));
    assert_eq!((s.heroes[0].hp, s.heroes[1].energy), (66, 5));
}

// ---- Volkhv ----

#[test]
fn the_staff_doubles_the_first_rune_and_perun_counts_thunder() {
    let (c, mut s) = fight("volhv", &["vol_thunder", "vol_thunder", "vol_perun", "vol_invoke"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "vol_thunder", Some(ME));
    play(&c, &mut s, 0, "vol_thunder", Some(ME));
    play(&c, &mut s, 0, "vol_perun", Some(t));
    assert_eq!(s.enemies[0].hp, 90);
    play(&c, &mut s, 0, "vol_invoke", None);
    // three strikes of 6: the first rune fires twice
    assert_eq!(s.enemies[0].hp, 72);
}

#[test]
fn storm_hits_everyone_and_the_dream_makes_a_rune_free() {
    let (c, mut s) = fight("volhv", &["vol_storm", "vol_prophetic_dream", "vol_thunder", "vol_invoke"], &["dummy", "dummy"]);
    s.heroes[0].flags.remove("first_rune_twice");
    play(&c, &mut s, 0, "vol_storm", None);
    play(&c, &mut s, 0, "vol_prophetic_dream", None);
    let before = s.heroes[0].energy;
    play(&c, &mut s, 0, "vol_thunder", Some(ME));
    assert_eq!(s.heroes[0].energy, before, "the rune after the dream is free");
    play(&c, &mut s, 0, "vol_invoke", None);
    assert_eq!((s.enemies[0].hp, s.enemies[1].hp), (94, 94));
}

#[test]
fn kolyada_fires_a_rune_per_energy() {
    let (c, mut s) = fight("volhv", &["vol_frost", "vol_kolyada"], &["dummy"]);
    s.heroes[0].flags.remove("first_rune_twice");
    play(&c, &mut s, 0, "vol_frost", Some(ME));
    s.heroes[0].energy = 3;
    play(&c, &mut s, 0, "vol_kolyada", None);
    assert_eq!((s.heroes[0].block, s.heroes[0].energy), (12, 0));
}

// ---- Volkolak ----

#[test]
fn the_fang_draws_on_the_first_shift_only() {
    let (c, mut s) = fight("volkolak", &["wolf_shift", "wolf_shift", "wolf_knife", "wolf_knife", "wolf_knife", "wolf_dodge", "wolf_dodge"], &["dummy"]);
    let hand = s.heroes[0].hand.len();
    let shifts = s.heroes[0].hand.iter().filter(|x| x.def == "wolf_shift").count();
    if shifts == 0 {
        return;
    }
    play(&c, &mut s, 0, "wolf_shift", None);
    assert_eq!((s.heroes[0].form, s.heroes[0].hand.len()), (Form::Beast, hand - 1 + 2));
    if s.heroes[0].hand.iter().any(|x| x.def == "wolf_shift") {
        let hand = s.heroes[0].hand.len();
        play(&c, &mut s, 0, "wolf_shift", None);
        assert_eq!((s.heroes[0].form, s.heroes[0].hand.len()), (Form::Human, hand - 1));
    }
}

#[test]
fn beast_side_of_the_cards() {
    let (c, mut s) = fight("volkolak", &["wolf_shift", "wolf_knife", "wolf_dodge", "wolf_maul"], &["hitter"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "wolf_shift", None);
    play(&c, &mut s, 0, "wolf_knife", Some(t));
    assert_eq!((s.enemies[0].hp, s.status(t, S::Bleed)), (87, 2), "claws: 9 x1.5 and bleeding");
    play(&c, &mut s, 0, "wolf_dodge", Some(t));
    assert_eq!(s.enemies[0].hp, 81, "the leap is an attack in Beast form");
    play(&c, &mut s, 0, "wolf_maul", Some(t));
    assert_eq!(s.enemies[0].hp, 81 - 4 * 9);
}

#[test]
fn silver_chain_holds_the_beast_back_and_blood_moon_ends_hunger() {
    let (c, mut s) = fight("volkolak", &["wolf_silver_chain", "wolf_shift"], &["dummy"]);
    play(&c, &mut s, 0, "wolf_silver_chain", None);
    play(&c, &mut s, 0, "wolf_shift", None);
    assert_eq!((s.heroes[0].form, s.heroes[0].block), (Form::Human, 12));

    let (c, mut s) = fight("volkolak", &["wolf_blood_moon", "wolf_shift"], &["dummy"]);
    play(&c, &mut s, 0, "wolf_blood_moon", None);
    play(&c, &mut s, 0, "wolf_shift", None);
    end_round(&c, &mut s);
    assert_eq!((s.heroes[0].form, s.heroes[0].hp), (Form::Beast, 80));
}

#[test]
fn devour_heals_and_grows() {
    let (c, mut s) = fight("volkolak", &["wolf_devour", "t_pain"], &["weakling", "dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_pain", None);
    play(&c, &mut s, 0, "wolf_devour", Some(t));
    assert_eq!((s.heroes[0].max_hp, s.heroes[0].hp), (81, 81));
}

// ---- Znakharka ----

#[test]
fn herbs_act_at_once_and_brew_by_recipe() {
    let (c, mut s) = fight("znaharka", &["herb_wormwood", "herb_wormwood", "zn_brew"], &["dummy", "dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "herb_wormwood", Some(t));
    play(&c, &mut s, 0, "herb_wormwood", Some(t));
    assert_eq!((s.enemies[0].hp, s.heroes[0].cauldron.len()), (92, 2));
    play(&c, &mut s, 0, "zn_brew", Some(t));
    assert_eq!((s.enemies[0].hp, s.enemies[1].hp), (86, 94), "wormwood twice: 6 to all");
    assert!(s.heroes[0].cauldron.is_empty());
}

#[test]
fn a_full_cauldron_throws_out_the_oldest_herb_and_it_bites() {
    let (c, mut s) = fight("znaharka", &["herb_wormwood", "herb_sleep", "herb_sleep", "herb_sleep", "herb_weep"], &["dummy"]);
    let t = foe(&s, 0);
    // her own relic gives her a cauldron of four
    play(&c, &mut s, 0, "herb_wormwood", Some(t));
    for _ in 0..3 {
        play(&c, &mut s, 0, "herb_sleep", None);
    }
    let hp = s.enemies[0].hp;
    let ev = play(&c, &mut s, 0, "herb_weep", None);
    assert_eq!(s.heroes[0].cauldron, ids(&["sleep", "sleep", "sleep", "weep"]));
    assert_eq!(s.enemies[0].hp, hp - 2, "wormwood spilled: 2 to all foes");
    assert!(ev.iter().any(|e| matches!(e, Event::HerbSpilled { player: 0, herb } if herb == "wormwood")));
}

#[test]
fn documented_recipes_and_the_fern() {
    let (c, mut s) = fight("znaharka", &["herb_sleep", "herb_weep", "zn_brew", "t_pain", "t_pain"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_pain", None);
    play(&c, &mut s, 0, "t_pain", None);
    play(&c, &mut s, 0, "herb_sleep", None);
    play(&c, &mut s, 0, "herb_weep", None);
    assert_eq!(s.heroes[0].hp, 62);
    play(&c, &mut s, 0, "zn_brew", Some(t));
    assert_eq!(s.heroes[0].hp, 67, "sleep-grass and weeping-grass: heal 5");

    let (c, mut s) = fight("znaharka", &["herb_burst", "herb_fern", "zn_brew"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "herb_burst", Some(t));
    play(&c, &mut s, 0, "herb_fern", None);
    play(&c, &mut s, 0, "zn_brew", Some(t));
    // 5 on play; the fern stands in for a second burst-grass (24), and doubles the brew
    assert_eq!(s.enemies[0].hp, 100 - 5 - 48);
}

#[test]
fn the_birch_pot_holds_four_and_brew_for_all_widens() {
    let (c, mut s) = fight("znaharka", &["herb_sleep", "herb_sleep", "herb_sleep", "herb_sleep", "herb_sleep"], &["dummy"]);
    for _ in 0..5 {
        play(&c, &mut s, 0, "herb_sleep", None);
    }
    assert_eq!(s.heroes[0].cauldron.len(), 4);

    let (c, mut s) = fight("znaharka", &["herb_burst", "zn_brew_for_all", "zn_brew"], &["dummy", "dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "herb_burst", Some(t));
    play(&c, &mut s, 0, "zn_brew_for_all", None);
    play(&c, &mut s, 0, "zn_brew", Some(t));
    assert_eq!((s.enemies[0].hp, s.enemies[1].hp), (85, 90));
}

#[test]
fn living_water_raises_at_half() {
    let c = content();
    let party = [with(&c, "znaharka", &["zn_living_water"]), hero("t_plain", &["t_suicide"])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 5);
    play(&c, &mut s, 1, "t_suicide", None);
    play(&c, &mut s, 0, "zn_living_water", Some(UnitRef::Hero(1)));
    assert_eq!((s.heroes[1].downed, s.heroes[1].hp), (false, 25));
    assert_eq!(s.heroes[0].exhaust.len(), 1);
}

// ---- Skomorokh ----

#[test]
fn echo_copies_an_ally_for_free_and_the_copy_burns() {
    let c = content();
    let party = [with(&c, "skomoroh", &["sk_echo"]), hero("t_plain", &["t_hit"])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 5);
    let t = foe(&s, 0);
    play(&c, &mut s, 1, "t_hit", Some(t));
    play(&c, &mut s, 0, "sk_echo", None);
    let copy = s.heroes[0].hand.iter().find(|x| x.def == "t_hit").expect("echo gives a copy");
    assert!(copy.free);
    let energy = s.heroes[0].energy;
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!((s.heroes[0].energy, s.enemies[0].hp), (energy, 180 - 20));
    assert!(s.heroes[0].discard.iter().all(|x| x.def != "t_hit"), "the copy is not kept");
}

#[test]
fn echo_alone_repeats_your_own_card() {
    let (c, mut s) = fight("skomoroh", &["sk_dance", "sk_echo"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "sk_dance", Some(t));
    play(&c, &mut s, 0, "sk_echo", None);
    play(&c, &mut s, 0, "sk_dance", Some(t));
    assert_eq!(s.enemies[0].hp, 90);
}

#[test]
fn bells_draw_once_a_turn_and_mummers_pay_the_party() {
    let c = content();
    let party = [with(&c, "skomoroh", &["sk_mummers", "sk_mask_bear", "sk_mask_goat", "sk_dance", "sk_dance", "sk_dance", "sk_dance"]), hero("t_plain", &[])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 5);
    s.heroes[0].energy = 20;
    let has = |s: &Combat, id: &str| s.heroes[0].hand.iter().any(|x| x.def == id);
    if !(has(&s, "sk_mummers") && has(&s, "sk_mask_bear") && has(&s, "sk_mask_goat")) {
        return;
    }
    play(&c, &mut s, 0, "sk_mummers", None);
    let hand = s.heroes[0].hand.len();
    play(&c, &mut s, 0, "sk_mask_bear", None);
    assert_eq!((s.heroes[0].hand.len(), s.heroes[1].energy), (hand, 4), "one card back, one energy to the ally");
    let hand = s.heroes[0].hand.len();
    play(&c, &mut s, 0, "sk_mask_goat", None);
    assert_eq!((s.heroes[0].hand.len(), s.heroes[1].energy), (hand - 1, 4), "only the first change each turn pays");
}

#[test]
fn switcheroo_turns_a_blow_on_another_enemy() {
    let (c, mut s) = fight("skomoroh", &["sk_switcheroo"], &["hitter", "dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "sk_switcheroo", Some(t));
    end_round(&c, &mut s);
    assert_eq!((s.heroes[0].hp, s.enemies[1].hp), (70, 90));
}

#[test]
fn death_dance_counts_the_whole_partys_cards() {
    let c = content();
    let party = [with(&c, "skomoroh", &["sk_death_dance"]), hero("t_plain", &["t_block", "t_block"])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 5);
    let t = foe(&s, 0);
    play(&c, &mut s, 1, "t_block", None);
    play(&c, &mut s, 1, "t_block", None);
    play(&c, &mut s, 0, "sk_death_dance", Some(t));
    assert_eq!(s.enemies[0].hp, 180 - 6);
}

#[test]
fn every_hero_has_ninety_four_cards_five_of_them_for_company() {
    let c = content();
    for hero in ["bogatyr", "vorozheya", "volhv", "volkolak", "znaharka", "skomoroh", "emelya"] {
        let mine: Vec<&CardDef> = c.cards.values().filter(|d| d.hero == hero).collect();
        // the jester has nine more: the fifth set, his discards; Yemelya has three to start with, not four
        assert_eq!(mine.len(), match hero { "skomoroh" => 103, "emelya" => 93, _ => 94 }, "{hero}");
        // Yemelya keeps to himself: none of his cards is for company
        assert!(hero == "emelya" || mine.iter().filter(|d| d.keywords.contains(&Keyword::Coop)).count() >= 5, "{hero}");
        // what is paid for in sleep is not free
        let free = mine.iter().filter(|d| d.cost == 0 && d.drowse == 0 && !d.x_cost && d.rarity != Rarity::Starter).count();
        assert!(free <= 8, "{hero} has {free} free cards");
    }
}

/// Every hero card, plain and upgraded, in both of the werewolf's forms, can be played
/// on the kind of target it asks for without the rules refusing it.
#[test]
fn every_hero_card_can_be_played() {
    let c = content();
    for def in c.cards.values().filter(|d| !d.hero.is_empty()) {
        for upgraded in [false, true] {
            for beast in [false, true] {
                if beast && def.hero != "volkolak" {
                    continue;
                }
                let party = [hero(&def.hero, &[]), hero("t_plain", &[])];
                let (mut s, _) = start(&c, &party, &ids(&["dummy", "dummy"]), 11);
                if beast {
                    s.heroes[0].form = Form::Beast;
                }
                s.heroes[0].energy = 9;
                s.heroes[0].statuses.insert(StatusKind::Drowse, 30);
                s.heroes[0].hand.push(Card { uid: 9000, def: def.id.clone(), upgraded, free: false });
                let view = def.view(upgraded, s.heroes[0].form);
                if view.has(Keyword::Unplayable) {
                    continue;
                }
                let target = match view.target {
                    CardTarget::None => None,
                    CardTarget::Enemy => Some(UnitRef::Enemy(s.enemies[0].uid)),
                    CardTarget::Ally | CardTarget::AnyHero => Some(UnitRef::Hero(1)),
                    CardTarget::Downed => {
                        s.heroes[1].downed = true;
                        s.heroes[1].hp = 0;
                        Some(UnitRef::Hero(1))
                    }
                };
                let res = apply(&c, &mut s, 0, &CombatAction::Play { card: 9000, target });
                assert!(res.is_ok(), "{} (upgraded {upgraded}, beast {beast}): {res:?}", def.id);
                // and the round can still be closed afterwards
                if s.phase == Phase::Heroes {
                    end_round(&c, &mut s);
                }
            }
        }
    }
}

#[test]
fn foretelling_leaves_out_what_falls_on_a_random_foe_and_rolls_nothing() {
    let (c, mut s) = fight("bogatyr", &["bogatyr_strike"], &["grey_wolf", "grey_wolf"]);
    s.heroes[0].relics.push("sheep_knuckles".into());
    let card = s.heroes[0].hand[0].uid;
    let foe = s.enemies[0].uid;
    let (hp, other) = (s.enemies[0].hp, s.enemies[1].hp);

    let mut copy = s.clone();
    copy.foretelling = true;
    apply(&c, &mut copy, 0, &CombatAction::Play { card, target: Some(UnitRef::Enemy(foe)) }).unwrap();
    assert_eq!(copy.rng, s.rng, "no roll, so the forecast stands");
    assert!(copy.enemies[0].hp < hp, "the card's own blow shows");
    assert_eq!(hp - copy.enemies[0].hp, c.cards["bogatyr_strike"].view(false, s.heroes[0].form).effects.iter().find_map(|e| if let Effect::Damage(Amount::N(n), _) = e { Some(*n) } else { None }).unwrap());
    assert_eq!(copy.enemies[1].hp, other, "the relic's stray blow does not");

    // played for real, the relic does strike
    apply(&c, &mut s, 0, &CombatAction::Play { card, target: Some(UnitRef::Enemy(foe)) }).unwrap();
    assert_ne!(s.rng, copy.rng);
}

#[test]
fn a_blow_goes_through_beside_a_relic_striking_at_random_and_a_wasted_one_is_told() {
    let (c, mut s) = fight("bogatyr", &["bogatyr_strike", "bogatyr_strike"], &["kikimora"]);
    s.heroes[0].relics.push("sheep_knuckles".into());
    let foe = s.enemies[0].uid;
    let hp = s.enemies[0].hp;
    let first = s.heroes[0].hand[0].uid;
    apply(&c, &mut s, 0, &CombatAction::Play { card: first, target: Some(UnitRef::Enemy(foe)) }).unwrap();
    let blow = c.cards["bogatyr_strike"].view(false, s.heroes[0].form).effects.iter().find_map(|e| if let Effect::Damage(Amount::N(n), _) = e { Some(*n) } else { None }).unwrap();
    assert_eq!(hp - s.enemies[0].hp, blow + 2, "the card's blow and the relic's");

    // charmed and alone, the attack goes nowhere: the relic still strikes, and the waste is told
    s.heroes[0].flags.insert("charmed".into(), 1);
    let hp = s.enemies[0].hp;
    let second = s.heroes[0].hand[0].uid;
    let ev = apply(&c, &mut s, 0, &CombatAction::Play { card: second, target: Some(UnitRef::Enemy(foe)) }).unwrap();
    assert_eq!(s.enemies[0].hp, hp, "once a turn, the relic is spent");
    assert!(ev.iter().any(|e| matches!(e, Event::Missed { unit: UnitRef::Hero(0) })), "{ev:?}");
}

#[test]
fn the_hare_dodges_once_a_turn_and_says_so() {
    let (c, mut s) = fight("bogatyr", &["bogatyr_strike", "bogatyr_strike"], &["death_hare"]);
    let foe = s.enemies[0].uid;
    let hp = s.enemies[0].hp;
    let first = s.heroes[0].hand[0].uid;
    let ev = apply(&c, &mut s, 0, &CombatAction::Play { card: first, target: Some(UnitRef::Enemy(foe)) }).unwrap();
    assert_eq!(s.enemies[0].hp, hp);
    assert!(ev.iter().any(|e| matches!(e, Event::Missed { unit } if *unit == UnitRef::Enemy(foe))), "{ev:?}");
    let second = s.heroes[0].hand[0].uid;
    apply(&c, &mut s, 0, &CombatAction::Play { card: second, target: Some(UnitRef::Enemy(foe)) }).unwrap();
    assert!(s.enemies[0].hp < hp, "the second one lands");
}

#[test]
fn yemelya_aims_at_nobody_and_lies_with_sloth_in_his_deck() {
    let c = content();
    let h = &c.heroes["emelya"];
    assert_eq!(h.deck.iter().filter(|d| *d == "em_sloth").count(), 2);
    for card in c.cards.values().filter(|d| d.hero == "emelya") {
        assert_eq!(card.target, CardTarget::None, "{} must not be aimed", card.id);
    }
    assert!(c.cards["em_sloth"].keywords.contains(&Keyword::Unplayable));
}

#[test]
fn strength_yemelya_leaves_unspent_is_sleep_gained_and_his_cards_spend_it() {
    let (c, mut s) = fight("emelya", &["em_dream_bite"], &["dummy"]);
    s.heroes[0].energy = 3;
    s.heroes[0].statuses.insert(StatusKind::Drowse, 4);
    let hp = s.enemies[0].hp;
    let card = s.heroes[0].hand[0].uid;
    apply(&c, &mut s, 0, &CombatAction::Play { card, target: None }).unwrap();
    assert_eq!(hp - s.enemies[0].hp, 12, "three for each");
    assert_eq!(s.status(UnitRef::Hero(0), StatusKind::Drowse), 0);
    let left = s.heroes[0].energy;
    end_round(&c, &mut s);
    assert_eq!(s.status(UnitRef::Hero(0), StatusKind::Drowse), left);
}

/// A foe striking one hero for `n` at its next blow.
fn struck(hp: i32, block: i32, n: i32, seed: u64) -> (Content, Combat, Vec<Event>) {
    let c = content();
    let mut party = with(&c, "bogatyr", &["bogatyr_strike"]);
    party.hp = hp;
    party.max_hp = 100;
    let (mut s, _) = start(&c, &[party], &ids(&["kikimora"]), seed);
    s.heroes[0].block = block;
    let ev = nav_core::combat::test_hit_hero(&c, &mut s, 0, n);
    (c, s, ev)
}

#[test]
fn a_light_blow_takes_no_limb_and_a_heavy_one_may() {
    assert!((0..60).all(|seed| struck(100, 0, 20, seed).1.heroes[0].lost_limbs == 0));
    let cut = (0..60).filter(|seed| struck(100, 0, 60, *seed).1.heroes[0].lost_limbs == 1).count();
    assert!(cut > 5 && cut < 55, "a chance, not a certainty: {cut} of 60");
    // what the shield stopped never reached the arm
    assert!((0..60).all(|seed| struck(100, 50, 60, seed).1.heroes[0].lost_limbs == 0));
}

#[test]
fn a_blow_far_past_what_is_left_takes_the_head_and_nothing_raises_him() {
    let (_, s, ev) = struck(10, 0, 12, 1);
    assert!(s.heroes[0].downed && !s.heroes[0].beheaded, "barely past: only down");
    assert!(ev.iter().all(|e| !matches!(e, Event::Beheaded { .. })));
    let (_, s, ev) = struck(10, 0, 70, 1);
    assert!(s.heroes[0].downed && s.heroes[0].beheaded);
    assert!(ev.iter().any(|e| matches!(e, Event::Beheaded { player: 0 })));
}

#[test]
fn each_limb_gone_costs_more() {
    let c = content();
    let deck = ["bogatyr_strike"; 12];
    let at = |limbs: u8| {
        let mut party = with(&c, "bogatyr", &deck);
        party.lost_limbs = limbs;
        let (s, _) = start(&c, &[party], &ids(&["dummy"]), 3);
        (s.heroes[0].hand.len(), s.heroes[0].energy)
    };
    let (whole, one, two) = (at(0), at(1), at(2));
    assert_eq!(one, (whole.0 - 1, whole.1));
    assert_eq!(two, (whole.0 - 1, whole.1 - 1));
}

#[test]
fn an_arm_lost_takes_what_it_did() {
    use nav_core::combat::{needs_lost_arm, SHIELD_ARM, WEAPON_ARM};
    let c = content();
    let card = |id: &str| c.cards.get(id).unwrap().view(false, Form::Human);
    let strike = card("bogatyr_strike");
    let guard = card("bogatyr_defend");
    let h = Form::Human;
    assert!(!needs_lost_arm(&strike, 0, "bogatyr", h) && !needs_lost_arm(&guard, 0, "bogatyr", h));
    assert!(needs_lost_arm(&strike, WEAPON_ARM, "bogatyr", h) && !needs_lost_arm(&guard, WEAPON_ARM, "bogatyr", h), "no weapon arm, no attacks");
    assert!(needs_lost_arm(&guard, SHIELD_ARM, "bogatyr", h) && !needs_lost_arm(&strike, SHIELD_ARM, "bogatyr", h), "no shield arm, no block");
    // the basket is on the shield arm, the jug on the other
    let herb = card("herb_wormwood");
    assert!(needs_lost_arm(&herb, SHIELD_ARM, "znaharka", h) && !needs_lost_arm(&herb, WEAPON_ARM, "znaharka", h));
    let brew = card("zn_brew");
    assert!(needs_lost_arm(&brew, WEAPON_ARM, "znaharka", h) && !needs_lost_arm(&brew, SHIELD_ARM, "znaharka", h));
    // a beast bites with no arm at all
    let knife = card("wolf_knife");
    assert!(needs_lost_arm(&knife, WEAPON_ARM, "volkolak", h));
    assert!(!needs_lost_arm(&knife, WEAPON_ARM | SHIELD_ARM, "volkolak", Form::Beast));
}

#[test]
fn the_armless_still_have_their_own_cards() {
    use nav_core::combat::{needs_lost_arm, SHIELD_ARM, WEAPON_ARM};
    let c = content();
    for id in ["stump_bash", "stump_kick", "stump_shoulder", "stump_spray", "stump_bite", "stump_spite"] {
        let v = c.cards.get(id).unwrap().view(false, Form::Human);
        assert!(!needs_lost_arm(&v, WEAPON_ARM | SHIELD_ARM, "bogatyr", Form::Human), "{id}");
    }
}

#[test]
fn some_of_yemelyas_cards_are_paid_for_in_sleep() {
    let (c, mut s) = fight("emelya", &["em_oven_fork"], &["dummy"]);
    assert_eq!(try_play(&c, &mut s, 0, "em_oven_fork", None).err(), Some(ActionError::NotEnoughEnergy), "no sleep, no card");
    s.heroes[0].statuses.insert(StatusKind::Drowse, 5);
    let (hp, energy) = (s.enemies[0].hp, s.heroes[0].energy);
    play(&c, &mut s, 0, "em_oven_fork", None);
    assert_eq!(s.status(UnitRef::Hero(0), StatusKind::Drowse), 3, "two of the five are spent");
    assert_eq!(s.heroes[0].energy, energy, "and no strength at all");
    assert_eq!(hp - s.enemies[0].hp, 8);
}

#[test]
fn the_pikes_will_plays_the_top_of_the_deck_for_nothing() {
    let card = |uid, def: &str| Card { uid, def: def.into(), upgraded: false, free: false };
    let (c, mut s) = fight("emelya", &["em_pike_fetch"], &["dummy"]);
    s.heroes[0].discard.clear();
    s.heroes[0].draw = vec![card(9001, "em_lie"), card(9002, "em_stove_door")];
    s.heroes[0].energy = 1;
    let hp = s.enemies[0].hp;
    play(&c, &mut s, 0, "em_pike_fetch", None);
    assert_eq!(hp - s.enemies[0].hp, 8, "the card on top struck by itself");
    assert_eq!(s.heroes[0].energy, 0, "only the card played by hand was paid for");
    assert!(s.heroes[0].discard.iter().any(|x| x.uid == 9002), "and it lies where played cards lie");
    assert!(s.heroes[0].hand.iter().any(|x| x.uid == 9001), "the draw that follows takes the next one");

    // sloth on top is not played even by the pike: it is thrown away as it is
    let (c, mut s) = fight("emelya", &["em_pike_fetch"], &["dummy"]);
    s.heroes[0].discard.clear();
    s.heroes[0].draw = vec![card(9001, "em_stove_door"), card(9002, "em_sloth")];
    let hp = s.enemies[0].hp;
    play(&c, &mut s, 0, "em_pike_fetch", None);
    assert_eq!(s.enemies[0].hp, hp);
    assert!(s.heroes[0].discard.iter().any(|x| x.uid == 9002));
    assert!(s.heroes[0].hand.iter().any(|x| x.uid == 9001));
}

#[test]
fn two_shifts_in_one_turn_turn_the_werewolf_back() {
    let (c, mut s) = fight("volkolak", &["wolf_shift", "wolf_lunar_tide"], &["dummy"]);
    s.heroes[0].energy = 5;
    play(&c, &mut s, 0, "wolf_shift", None);
    assert_eq!(s.heroes[0].form, Form::Beast);
    play(&c, &mut s, 0, "wolf_lunar_tide", None);
    assert_eq!(s.heroes[0].form, Form::Human, "the second shift turns him back");
}

#[test]
fn a_card_that_shifts_the_man_shifts_the_beast_too() {
    let c = content();
    let shifts = |fx: &[Effect]| fx.iter().any(|e| matches!(e, Effect::Shift));
    let mut bad = Vec::new();
    for def in c.cards.values().filter(|d| d.hero == "volkolak") {
        for up in [false, true] {
            let (man, beast) = (def.view(up, Form::Human), def.view(up, Form::Beast));
            if shifts(&man.effects) != shifts(&beast.effects) || shifts(&man.on_discard) != shifts(&beast.on_discard) {
                bad.push(format!("{}{}", def.id, if up { "+" } else { "" }));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
