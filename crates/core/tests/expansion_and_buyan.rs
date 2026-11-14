mod common;
use common::*;
use nav_core::*;

fn test_content() -> Content {
    content()
}

#[test]
fn test_buyan_map_structure() {
    let mut rng = Rng::new(12345);
    let map = generate_buyan_map(&mut rng, Circle(0));
    assert_eq!(map.floors.len(), 6, "Buyan must have exactly 6 floors");

    // Floor 0: Shore (Fights)
    assert!(map.floors[0].iter().all(|n| n.kind == NodeKind::Fight));
    // Floor 1: Sacred Grove (Events)
    assert!(map.floors[1].iter().all(|n| n.kind == NodeKind::Event));
    // Floor 2: Heart (Elites)
    assert!(map.floors[2].iter().all(|n| n.kind == NodeKind::Elite));
    // Floor 3: Sanctuary (Shop & Rest)
    assert!(map.floors[3].iter().any(|n| n.kind == NodeKind::Shop));
    assert!(map.floors[3].iter().any(|n| n.kind == NodeKind::Rest));
    // Floor 4: Citadel Approach (Fight & Event)
    assert!(map.floors[4].iter().any(|n| n.kind == NodeKind::Fight));
    assert!(map.floors[4].iter().any(|n| n.kind == NodeKind::Event));
    // Floor 5: Iron Kurgan (Rest before Koschei)
    assert_eq!(map.floors[5].len(), 1);
    assert_eq!(map.floors[5][0].kind, NodeKind::Rest);
    assert!(map.floors[5][0].next.is_empty(), "Last floor connects to Boss");
}

#[test]
fn test_new_encounters_and_enemies_load() {
    let c = test_content();
    let want_encounters = [
        "a1_bayun", "a1_mokhovik", "a1_nochnitsy",
        "a1_solovey", "a1_korochun",
        "a2_baba_yaga", "a2_vodyanoy_tsar", "a2_polunochnitsa", "a2_zlydni",
        "a3_morovitsa",
        "a4_sea_guards", "a4_ogneya", "a4_morovitsa",
        "a4_gagana", "a4_gorynych", "a4_koschei",
    ];
    for enc_id in want_encounters {
        let enc = c.encounters.get(enc_id).unwrap_or_else(|| panic!("missing encounter `{enc_id}`"));
        for enemy_id in enc.enemies.iter().chain(&enc.solo).chain(&enc.extra3).chain(&enc.extra4) {
            assert!(c.enemies.contains_key(enemy_id), "encounter `{enc_id}` references unknown enemy `{enemy_id}`");
        }
    }

    // Verify Buyan omens
    assert!(c.events.contains_key("alatyr_stone"), "missing alatyr_stone omen");
    assert!(c.events.contains_key("waters_of_buyan"), "missing waters_of_buyan omen");
    assert!(c.events.contains_key("buyan_oak"), "missing buyan_oak omen");

    // Verify status card heavy_eyelids
    assert!(c.cards.contains_key("heavy_eyelids"), "missing heavy_eyelids card");
}

#[test]
fn test_mokhovik_kinetic_moss_mechanic() {
    let (c, mut s) = solo(&["t_hit_all"], &["mokhovik"]);
    let initial_hp = s.enemies[0].hp;
    // "t_hit_all" deals 4 damage (< 8 threshold).
    // Kinetic moss absorbs it: damage is reduced to 1, and kinetic_charge increases!
    play(&c, &mut s, 0, "t_hit_all", None);
    assert_eq!(s.enemies[0].hp, initial_hp - 1, "Mokhovik absorbs hits under 8 dmg to 1");
    assert_eq!(s.enemies[0].flags.get("kinetic_charge").copied(), Some(1), "Mokhovik gains kinetic charge");
}

#[test]
fn test_gagana_spell_mirror_mechanic() {
    let (c, mut s) = solo(&["t_hit"], &["gagana"]);
    let initial_hero_hp = s.heroes[0].hp;
    let t = foe(&s, 0);
    // "t_hit" deals 10 damage to Gagana. Spell mirror reflects 30% (3 dmg) back to hero!
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert!(s.heroes[0].hp < initial_hero_hp, "Gagana reflects damage back to attacker");
}

#[test]
fn test_nochnitsa_flicker_mechanic() {
    let (c, mut s) = solo(&["t_hit", "t_hit"], &["nochnitsa"]);
    let t = foe(&s, 0);
    // Round 1: normal hit (deals 10)
    play(&c, &mut s, 0, "t_hit", Some(t));
    let hp_after_r1 = s.enemies[0].hp;
    // End round 1, advance to Round 2 (even round):
    end_round(&c, &mut s);
    assert_eq!(s.round, 2);
    // Round 2: attack is flickered to 1 dmg!
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, hp_after_r1 - 1, "Flicker reduces attack on even round to 1");
}

#[test]
fn test_telemetry_serialization() {
    let mut enemies = std::collections::BTreeMap::new();
    enemies.insert("kot_bayun".to_string(), serde_json::json!({
        "damage_dealt": 28,
        "rounds": 3,
        "hero_kills": 0
    }));

    let json_val = serde_json::json!({
        "version": "0.1.8",
        "run_id": "123456789",
        "won": true,
        "circle": 1,
        "act": 4,
        "floor": 6,
        "heroes": ["bogatyr", "volhv"],
        "final_deck": ["strike+", "shield_wall"],
        "relics": ["alatyr_shard"],
        "cards_played": {"strike+": 12},
        "enemies_summary": enemies
    });

    let s = serde_json::to_string(&json_val).expect("serializes cleanly");
    assert!(s.contains("kot_bayun"));
    assert!(s.contains("123456789"));
    assert!(s.contains("true"));
}

#[test]
fn test_act3_to_act4_heal() {
    let c = test_content();
    let mut run = Run::new(&c, &["bogatyr".to_string()], 42, 0).unwrap();
    run.act = 3;
    run.fighting = Some(NodeKind::Boss);
    run.players[0].hp = 20;
    let max_hp = run.players[0].max_hp;

    // Simulate rewards taken after Act 3 boss:
    run.stage = Stage::Reward {
        rewards: vec![Reward { gold: 0, cards: vec![], relic: None, done: false }],
    };
    run.apply(&c, 0, &RunAction::TakeCard(None)).unwrap();

    // Run transitioned to Act 4 (Buyan):
    assert_eq!(run.act, 4);
    // Missing HP was (max_hp - 20). 80% healed:
    let expected_delta = ((max_hp - 20) * 80 + 99) / 100;
    assert_eq!(run.players[0].hp, 20 + expected_delta);
}

#[test]
fn test_bayun_perch_blocks_targeted_melee_allows_sweeping() {
    let (c, mut s) = solo(&["t_hit", "t_hit_all"], &["kot_bayun"]);
    let t = foe(&s, 0);
    let start_hp = s.enemies[0].hp;
    assert!(s.enemies[0].flags.contains_key("bayun_perch"));

    // Targeted strike misses because cat is on high perch!
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!(s.enemies[0].hp, start_hp, "Targeted strike cannot reach cat on his perch");

    // AllFoes sweeping strike hits!
    play(&c, &mut s, 0, "t_hit_all", None);
    assert_eq!(s.enemies[0].hp, start_hp - 4, "Sweeping strike hits cat on his perch");
}

#[test]
fn ogneya_runs_hotter_with_every_attack() {
    let (c, mut s) = solo(&["t_block"], &["ogneya"]);
    let lash = c.enemies["ogneya"].moves.iter().position(|m| m.id == "fire_whip").unwrap();
    s.enemies[0].intent = Some(Intent { mv: lash, target: Some(0) });
    end_round(&c, &mut s);
    assert_eq!(s.enemies[0].statuses.get(&StatusKind::Strength).copied().unwrap_or(0), 1);
}

#[test]
fn a_fallen_gorynych_head_grows_back_and_is_seen_to() {
    let (c, mut s) = solo(&["t_block"], &["gorynych_fire", "gorynych_poison"]);
    let fire = s.enemies[0].uid;
    s.enemies[0].hp = 0;
    s.enemies[0].alive = false;
    // the hero stands through it all
    s.heroes[0].max_hp = 999;
    s.heroes[0].hp = 999;
    let mut seen = false;
    for _ in 0..3 {
        let round = s.round;
        let ev = apply(&c, &mut s, 0, &CombatAction::Ready(true)).unwrap();
        seen |= ev.iter().any(|e| matches!(e, Event::Summoned { enemy, .. } if *enemy == fire));
        assert!(s.round > round);
    }
    assert!(s.enemies[0].alive, "the head grew back");
    assert!(seen, "and the others were told");
}

#[test]
fn bayun_leaps_down_to_pounce_and_climbs_back_to_purr() {
    let (c, mut s) = solo(&["t_block"], &["kot_bayun"]);
    s.heroes[0].max_hp = 999;
    s.heroes[0].hp = 999;
    let moves = &c.enemies["kot_bayun"].moves;
    let pounce = moves.iter().position(|m| m.id == "pounce").unwrap();
    let purr = moves.iter().position(|m| m.id == "purr").unwrap();
    s.enemies[0].intent = Some(Intent { mv: pounce, target: Some(0) });
    end_round(&c, &mut s);
    assert!(!s.enemies[0].flags.contains_key("bayun_perch"), "down off his post, in reach");
    s.enemies[0].intent = Some(Intent { mv: purr, target: None });
    end_round(&c, &mut s);
    assert!(s.enemies[0].flags.contains_key("bayun_perch"), "back up again");
}
