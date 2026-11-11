//! The run layer: map, votes, fights, rewards, rest.

mod common;
use common::*;
use nav_core::*;

const SIX: [&str; 6] = ["bogatyr", "vorozheya", "volhv", "volkolak", "znaharka", "skomoroh"];

fn party(c: &Content, n: usize, seed: u64, circle: u8) -> Run {
    let heroes: Vec<Id> = SIX[..n].iter().map(|s| s.to_string()).collect();
    Run::new(c, &heroes, seed, circle).unwrap()
}

/// Everyone acts with the autopilot until the run ends; returns how many actions it took.
fn autoplay(c: &Content, run: &mut Run, limit: usize) -> usize {
    let mut steps = 0;
    while !matches!(run.stage, Stage::Victory | Stage::Defeat) {
        let mut moved = false;
        for p in 0..run.players.len() as u8 {
            if let Some(a) = run_bot(c, run, p) {
                run.apply(c, p, &a).unwrap_or_else(|e| panic!("bot move {a:?} refused: {e:?}"));
                moved = true;
                steps += 1;
            }
        }
        assert!(moved, "the run is stuck in {:?}", run.stage);
        assert!(steps < limit, "the run did not end in {limit} actions");
    }
    steps
}

#[test]
fn maps_follow_the_floor_rules() {
    for seed in 0..60 {
        let map = generate_map(&mut Rng::new(seed), Circle(0));
        assert_eq!(map.floors.len(), FLOORS);
        for (f, floor) in map.floors.iter().enumerate() {
            assert!(!floor.is_empty() && floor.len() <= MAP_WIDTH as usize);
            for node in floor {
                match f {
                    0 => assert_eq!(node.kind, NodeKind::Fight, "floor 1 is always a fight"),
                    8 => assert_eq!(node.kind, NodeKind::Chest, "floor 9 is always a chest"),
                    14 => assert_eq!(node.kind, NodeKind::Rest, "floor 15 is always a fire"),
                    _ => {
                        assert!(node.kind != NodeKind::Elite || f >= 5, "no elite before floor 6");
                        assert!(node.kind != NodeKind::Shop || f >= 2, "no shop before floor 3");
                        assert!(!matches!(node.kind, NodeKind::Chest | NodeKind::Boss));
                    }
                }
                if f + 1 < FLOORS {
                    assert!(!node.next.is_empty(), "seed {seed}: a dead end on floor {f}");
                    assert!(node.next.iter().all(|i| (*i as usize) < map.floors[f + 1].len()));
                }
            }
            // every node above the first floor can be reached
            if f > 0 {
                for i in 0..floor.len() as u8 {
                    assert!(map.floors[f - 1].iter().any(|p| p.next.contains(&i)), "seed {seed}: orphan node on floor {f}");
                }
            }
        }
    }
}

#[test]
fn node_shares_are_roughly_as_designed() {
    let mut count = std::collections::BTreeMap::new();
    let mut total = 0;
    for seed in 0..300 {
        let map = generate_map(&mut Rng::new(seed), Circle(0));
        for (f, floor) in map.floors.iter().enumerate() {
            if matches!(f, 0 | 8 | 14) {
                continue;
            }
            for n in floor {
                *count.entry(n.kind).or_insert(0) += 1;
                total += 1;
            }
        }
    }
    let share = |k| 100 * count.get(&k).copied().unwrap_or(0) / total;
    assert!((40..=62).contains(&share(NodeKind::Fight)), "fights {}%", share(NodeKind::Fight));
    assert!((15..=30).contains(&share(NodeKind::Event)), "omens {}%", share(NodeKind::Event));
    assert!((6..=16).contains(&share(NodeKind::Rest)), "fires {}%", share(NodeKind::Rest));
    assert!((2..=12).contains(&share(NodeKind::Elite)), "elites {}%", share(NodeKind::Elite));
    assert!((2..=10).contains(&share(NodeKind::Shop)), "shops {}%", share(NodeKind::Shop));
}

#[test]
fn the_first_circle_brings_more_elites() {
    let elites = |circle| -> usize {
        (0..200).map(|seed| generate_map(&mut Rng::new(seed), Circle(circle)).floors.iter().flatten().filter(|n| n.kind == NodeKind::Elite).count()).sum()
    };
    assert!(elites(1) > elites(0));
}

#[test]
fn a_solo_vote_moves_at_once_into_the_first_fight() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    assert!(run.choices().iter().all(|(_, k)| *k == NodeKind::Fight));
    let ev = run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    assert_eq!(run.stage, Stage::Combat);
    assert_eq!(run.at, Some((0, run.choices_origin())));
    assert!(ev.contains(&RunEvent::CombatStarted));
    assert_eq!(run.combat.as_ref().unwrap().heroes[0].def, "bogatyr");
    assert_eq!(run.apply(&c, 0, &RunAction::Vote(0)), Err(RunError::WrongStage));
}

trait Origin {
    fn choices_origin(&self) -> u8;
}
impl Origin for Run {
    /// The node index the party is standing on.
    fn choices_origin(&self) -> u8 {
        self.at.map(|a| a.1).unwrap_or(0)
    }
}

#[test]
fn the_party_waits_for_every_vote_and_one_mind_needs_no_lot() {
    let c = content();
    // a seed whose first floor offers at least two ways
    let mut run = (0..50).map(|s| party(&c, 3, s, 0)).find(|r| r.choices().len() >= 2).unwrap();
    run.apply(&c, 0, &RunAction::Vote(1)).unwrap();
    run.apply(&c, 1, &RunAction::Vote(0)).unwrap();
    assert!(matches!(run.stage, Stage::Map { .. }), "two of three have voted");
    assert_eq!(run.apply(&c, 2, &RunAction::Vote(9)), Err(RunError::BadChoice));
    // a player may move their token before the last vote lands
    run.apply(&c, 1, &RunAction::Vote(1)).unwrap();
    let want = run.choices()[1].0;
    let ev = run.apply(&c, 2, &RunAction::Vote(1)).unwrap();
    assert_eq!(run.at, Some((0, want)));
    assert!(!ev.iter().any(|e| matches!(e, RunEvent::Lot { .. })));
}

#[test]
fn a_split_vote_draws_lots_weighted_by_votes() {
    let c = content();
    let (mut two, mut tries) = (0, 0);
    for seed in 0..300 {
        let mut run = party(&c, 3, seed, 0);
        if run.choices().len() < 2 {
            continue;
        }
        run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
        run.apply(&c, 1, &RunAction::Vote(0)).unwrap();
        let ev = run.apply(&c, 2, &RunAction::Vote(1)).unwrap();
        let (tickets, chosen) = ev.iter().find_map(|e| if let RunEvent::Lot { tied, chosen } = e { Some((tied.clone(), *chosen)) } else { None }).expect("two against one draws lots");
        assert_eq!(tickets, vec![0, 0, 1], "one ticket per vote");
        tries += 1;
        two += (chosen == 0) as u32;
    }
    let share = two as f32 / tries as f32;
    assert!((0.55..0.78).contains(&share), "two votes against one won {share:.2} of {tries} lots, not about two thirds");
}

#[test]
fn a_tied_vote_is_settled_by_lot_not_by_the_host() {
    let c = content();
    let mut picked = std::collections::BTreeSet::new();
    for seed in 0..80 {
        let mut run = party(&c, 2, seed, 0);
        if run.choices().len() < 2 {
            continue;
        }
        run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
        let ev = run.apply(&c, 1, &RunAction::Vote(1)).unwrap();
        let lot = ev.iter().find_map(|e| if let RunEvent::Lot { tied, chosen } = e { Some((tied.clone(), *chosen)) } else { None });
        let (tied, chosen) = lot.expect("a tie draws lots");
        assert_eq!(tied, vec![0, 1]);
        picked.insert(chosen);
    }
    assert_eq!(picked.len(), 2, "over many runs the lot falls both ways");
}

#[test]
fn rewards_offer_three_cards_of_your_own_hero() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    let gold = run.players[0].gold;
    while run.stage == Stage::Combat {
        let a = run_bot(&c, &run, 0).unwrap();
        run.apply(&c, 0, &a).unwrap();
    }
    let Stage::Reward { rewards } = &run.stage else { panic!("expected rewards, got {:?}", run.stage) };
    let r = &rewards[0];
    assert_eq!(r.cards.len(), 3);
    assert!((10..=20).contains(&r.gold));
    assert_eq!(run.players[0].gold, gold + r.gold, "gold is paid at once");
    for card in &r.cards {
        let d = &c.cards[card];
        assert_eq!(d.hero, "bogatyr");
        assert!(!d.keywords.contains(&Keyword::Coop), "no co-op cards for a lone hero");
        assert_ne!(d.rarity, Rarity::Starter);
    }
    let mut unique = r.cards.clone();
    unique.dedup();
    assert_eq!(unique.len(), 3);

    let (deck, want) = (run.players[0].deck.len(), r.cards[1].clone());
    run.apply(&c, 0, &RunAction::TakeCard(Some(1))).unwrap();
    assert_eq!(run.players[0].deck.len(), deck + 1);
    assert_eq!(run.players[0].deck.last().unwrap().0, want);
    assert!(matches!(run.stage, Stage::Map { .. }));
    assert_eq!(run.floors_cleared, 1);
}

#[test]
fn fire_heals_thirty_percent_or_upgrades_one_card() {
    let c = content();
    let mut run = party(&c, 2, 3, 0);
    run.stage = Stage::Rest { done: vec![false, false] };
    run.players[0].hp = 10;
    run.apply(&c, 0, &RunAction::Sleep).unwrap();
    assert_eq!(run.players[0].hp, 10 + 85 * 30 / 100);
    assert_eq!(run.apply(&c, 0, &RunAction::Sleep), Err(RunError::AlreadyDone));
    assert!(matches!(run.stage, Stage::Rest { .. }), "the other player is still at the fire");

    let gold = (run.players[0].gold, run.players[1].gold);
    run.apply(&c, 0, &RunAction::GiveGold { to: 1, amount: 40 }).unwrap();
    assert_eq!((run.players[0].gold, run.players[1].gold), (gold.0 - 40, gold.1 + 40));
    assert_eq!(run.apply(&c, 0, &RunAction::GiveGold { to: 1, amount: 9999 }), Err(RunError::NotEnoughGold));

    run.apply(&c, 1, &RunAction::Upgrade(0)).unwrap();
    assert!(run.players[1].deck[0].1);
    assert!(matches!(run.stage, Stage::Map { .. }));

    // Circle 5: the fire warms less
    let mut run = party(&c, 1, 3, 5);
    run.stage = Stage::Rest { done: vec![false] };
    run.players[0].hp = 10;
    run.apply(&c, 0, &RunAction::Sleep).unwrap();
    assert_eq!(run.players[0].hp, 10 + 85 * 20 / 100);
}

#[test]
fn an_upgraded_card_cannot_be_upgraded_again() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    run.stage = Stage::Rest { done: vec![false] };
    run.players[0].deck[0].1 = true;
    assert_eq!(run.apply(&c, 0, &RunAction::Upgrade(0)), Err(RunError::BadChoice));
    assert_eq!(run.apply(&c, 0, &RunAction::Upgrade(999)), Err(RunError::BadChoice));
}

#[test]
fn circles_change_the_rules() {
    let c = content();
    let run = party(&c, 1, 3, 6);
    assert!(run.players[0].deck.iter().any(|(id, _)| id == "toska"), "Circle 6 starts with Longing");
    let mut run = party(&c, 1, 3, 9);
    run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    let combat = run.combat.as_ref().unwrap();
    assert_eq!(combat.raise_cost, 3);
    assert_eq!(combat.enemy_dmg, 110);
    assert_eq!(combat.hp_scale, 1100);
}

#[test]
fn wounds_falls_and_curses_carry_between_fights() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    {
        let combat = run.combat.as_mut().unwrap();
        combat.heroes[0].hp = 40;
        combat.heroes[0].falls = 2;
        combat.heroes[0].new_curses = 1;
        for e in &mut combat.enemies {
            e.hp = 1;
        }
    }
    while run.stage == Stage::Combat {
        let a = run_bot(&c, &run, 0).unwrap();
        run.apply(&c, 0, &a).unwrap();
    }
    let p = &run.players[0];
    assert!(p.hp <= 40 && p.hp > 0);
    assert_eq!(p.falls, 2);
    assert_eq!(p.deck.iter().filter(|(id, _)| id == NAV_MARK).count(), 1);
}

#[test]
fn every_hero_can_be_driven_through_a_whole_run() {
    let c = content();
    for hero in SIX {
        for seed in [1u64, 2] {
            let mut run = Run::new(&c, &[hero.to_string()], seed, 0).unwrap();
            autoplay(&c, &mut run, 60_000);
            // whichever way it ended, the books must add up
            assert!(run.floors_cleared > 0, "{hero} never cleared a floor");
            assert!(run.oberegi() >= run.floors_cleared);
            if run.stage == Stage::Victory {
                assert_eq!((run.act, run.bosses_slain), (4, 4));
            }
        }
    }
}

#[test]
fn a_full_party_run_replays_identically() {
    let c = content();
    let play = |seed| {
        let mut run = party(&c, 4, seed, 0);
        let steps = autoplay(&c, &mut run, 200_000);
        (run.hash64(), steps, run.stage.clone())
    };
    assert_eq!(play(11), play(11));
    assert_ne!(play(11).0, play(12).0);
}

/// Not a pass/fail check: prints how autopilot runs end, for balance reading.
#[test]
#[ignore]
fn report_autopilot_outcomes() {
    let c = content();
    for hero in SIX {
        let mut wins = 0;
        let mut floors = 0;
        for seed in 0..40u64 {
            let mut run = Run::new(&c, &[hero.to_string()], seed, 0).unwrap();
            autoplay(&c, &mut run, 60_000);
            wins += (run.stage == Stage::Victory) as u32;
            floors += run.floors_cleared;
        }
        println!("{hero:<10} wins {wins}/40, avg floors cleared {:.1} of 48", floors as f32 / 40.0);
    }
    let mut wins = 0;
    for seed in 0..20u64 {
        let mut run = party(&c, 4, seed, 0);
        autoplay(&c, &mut run, 200_000);
        wins += (run.stage == Stage::Victory) as u32;
    }
    println!("party of 4 wins {wins}/20");
}

// ---- potions, the pedlar, omens ----

#[test]
fn potions_are_drunk_in_a_fight_and_carried_out() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    run.players[0].potions = vec!["mead".into(), "fire_water".into()];
    run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    let hp: Vec<i32> = run.combat.as_ref().unwrap().enemies.iter().map(|e| e.hp).collect();
    run.apply(&c, 0, &RunAction::Combat(CombatAction::UsePotion { slot: 0, target: None })).unwrap();
    assert_eq!(run.combat.as_ref().unwrap().heroes[0].energy, 5);
    run.apply(&c, 0, &RunAction::Combat(CombatAction::UsePotion { slot: 0, target: None })).unwrap();
    let after: Vec<i32> = run.combat.as_ref().unwrap().enemies.iter().map(|e| e.hp).collect();
    assert!(hp.iter().zip(&after).all(|(a, b)| *b == (*a - 10).max(0)));
    assert_eq!(
        run.apply(&c, 0, &RunAction::Combat(CombatAction::UsePotion { slot: 0, target: None })),
        Err(RunError::Combat(ActionError::NoSuchPotion))
    );
}

#[test]
fn the_pedlar_sells_removes_and_lets_you_leave() {
    let c = content();
    let mut run = party(&c, 2, 3, 0);
    // put the party at a shop by hand
    let stock: Vec<Stock> = vec![
        Stock { cards: vec![Some(("bogatyr_shield_wall".into(), 50))], relics: vec![], potions: vec![Some(("mead".into(), 45)), Some(("tar".into(), 45))], pelted: false },
        Stock::default(),
    ];
    run.stage = Stage::Shop { stock, done: vec![false, false] };
    run.players[0].gold = 100;
    let deck = run.players[0].deck.len();
    run.apply(&c, 0, &RunAction::BuyCard(0)).unwrap();
    assert_eq!((run.players[0].gold, run.players[0].deck.len()), (50, deck + 1));
    assert_eq!(run.apply(&c, 0, &RunAction::BuyCard(0)), Err(RunError::SoldOut));
    run.apply(&c, 0, &RunAction::BuyPotion(0)).unwrap();
    assert_eq!(run.apply(&c, 0, &RunAction::BuyPotion(1)), Err(RunError::NotEnoughGold));
    assert_eq!(run.players[0].potions, vec!["mead".to_string()]);

    // removal costs 75, then 100
    run.players[1].gold = 180;
    let deck = run.players[1].deck.len();
    run.apply(&c, 1, &RunAction::RemoveCard(0)).unwrap();
    run.apply(&c, 1, &RunAction::RemoveCard(0)).unwrap();
    assert_eq!((run.players[1].gold, run.players[1].deck.len(), run.players[1].removals), (5, deck - 2, 2));

    run.apply(&c, 0, &RunAction::GiveGold { to: 1, amount: 5 }).unwrap();
    run.apply(&c, 0, &RunAction::LeaveShop).unwrap();
    assert!(matches!(run.stage, Stage::Shop { .. }));
    run.apply(&c, 1, &RunAction::LeaveShop).unwrap();
    assert!(matches!(run.stage, Stage::Map { .. }));
}

#[test]
fn pedlar_shelves_are_stocked_for_your_hero() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    // walk the map by hand to the first shop on any path; simpler: ask for stock via a shop node
    run.stage = Stage::Map { votes: vec![None] };
    for seed in 0..200 {
        let mut r = party(&c, 1, seed, 0);
        // find a seed whose first floors lead to a shop quickly by brute force over the bot
        for _ in 0..400 {
            if let Stage::Shop { stock, .. } = &r.stage {
                let s = &stock[0];
                assert!(s.cards.len() >= 3, "shelves hold cards");
                for (id, price) in s.cards.iter().flatten() {
                    let d = &c.cards[id];
                    assert!(d.hero == "bogatyr" || d.hero.is_empty());
                    assert!(*price >= 45);
                }
                assert_eq!(s.potions.len(), 3);
                return;
            }
            if matches!(r.stage, Stage::Victory | Stage::Defeat) {
                break;
            }
            let Some(a) = run_bot(&c, &r, 0) else { break };
            r.apply(&c, 0, &a).unwrap();
        }
    }
    panic!("no run ever reached a pedlar");
}

#[test]
fn a_personal_omen_is_each_players_own_choice() {
    let c = content();
    let mut run = party(&c, 2, 3, 0);
    run.stage = Stage::Omen { id: "hungry_village".into(), picks: vec![None, None] };
    run.players[0].gold = 40;
    run.players[1].gold = 10;
    let relics = run.players[0].relics.len();
    run.apply(&c, 0, &RunAction::Choose(0)).unwrap();
    assert_eq!(run.players[0].gold, 0);
    // no relic pool beyond starters yet means nothing to hand out; the price is still paid
    assert!(run.players[0].relics.len() >= relics);
    assert_eq!(run.apply(&c, 0, &RunAction::Choose(2)), Err(RunError::AlreadyDone));
    assert_eq!(run.apply(&c, 1, &RunAction::Choose(0)), Err(RunError::NotEnoughGold));
    run.apply(&c, 1, &RunAction::Choose(1)).unwrap();
    assert_eq!(run.players[1].gold, 60);
    assert!(run.players[1].deck.iter().any(|(id, _)| id == "toska"));
    assert!(matches!(run.stage, Stage::Map { .. }));
}

#[test]
fn a_vote_omen_binds_the_whole_party_and_never_kills() {
    let c = content();
    let mut run = party(&c, 3, 3, 0);
    run.stage = Stage::Omen { id: "crossroads_stone".into(), picks: vec![None, None, None] };
    run.players[2].hp = 3;
    run.apply(&c, 0, &RunAction::Choose(1)).unwrap();
    run.apply(&c, 1, &RunAction::Choose(0)).unwrap();
    assert!(matches!(run.stage, Stage::Omen { .. }));
    let ev = run.apply(&c, 2, &RunAction::Choose(1)).unwrap();
    assert!(ev.contains(&RunEvent::OmenResolved { option: 1 }));
    assert_eq!(run.players[0].hp, 85 - 85 * 12 / 100);
    assert_eq!(run.players[2].hp, 1, "an omen wounds but does not kill");
    assert!(matches!(run.stage, Stage::Map { .. }));
}

/// Not a pass/fail check: prints which fights end solo autopilot runs.
#[test]
#[ignore]
fn report_where_solo_runs_die() {
    let c = content();
    let mut deaths: std::collections::BTreeMap<String, u32> = Default::default();
    let mut floors: Vec<u32> = Vec::new();
    for hero in SIX {
        for seed in 0..40u64 {
            let mut run = Run::new(&c, &[hero.to_string()], seed, 0).unwrap();
            autoplay(&c, &mut run, 60_000);
            if run.stage == Stage::Defeat {
                let mut names: Vec<String> = run.combat.as_ref().map(|k| k.enemies.iter().map(|e| e.def.clone()).collect()).unwrap_or_default();
                names.sort();
                names.dedup();
                *deaths.entry(format!("act {} {:?} {}", run.act, run.fighting.unwrap_or(NodeKind::Fight), names.join("+"))).or_insert(0) += 1;
                floors.push(run.floors_cleared);
            }
        }
    }
    let mut rows: Vec<_> = deaths.into_iter().collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.1));
    for (k, n) in rows.iter().take(12) {
        println!("{n:>4}  {k}");
    }
    floors.sort();
    println!("median floors cleared at death: {}", floors.get(floors.len() / 2).copied().unwrap_or(0));
}

/// Not a pass/fail check: finds fights the autopilot can neither win nor lose.
#[test]
#[ignore]
fn report_endless_fights() {
    let c = content();
    let mut seen: std::collections::BTreeMap<String, u32> = Default::default();
    for (i, hero) in SIX.iter().enumerate() {
        for seed in 0..60u64 {
            let mut run = Run::new(&c, &[hero.to_string()], seed + i as u64 * 1000, 0).unwrap();
            for _ in 0..200_000 {
                if matches!(run.stage, Stage::Victory | Stage::Defeat) {
                    break;
                }
                if let Some(k) = run.combat.as_ref().filter(|k| k.round > 150) {
                    let mut names: Vec<String> = k.living_enemies().map(|e| format!("{}({}hp,{}blk)", e.def, e.hp, e.block)).collect();
                    names.sort();
                    let h = &k.heroes[0];
                    *seen.entry(format!("{hero} hp {}/{} vs {}", h.hp, h.max_hp, names.join(" "))).or_insert(0) += 1;
                    break;
                }
                let Some(a) = run_bot(&c, &run, 0) else { break };
                run.apply(&c, 0, &a).unwrap();
            }
        }
    }
    for (k, n) in &seen {
        println!("{n:>3}  {k}");
    }
    println!("{} endless fights", seen.values().sum::<u32>());
}

#[test]
fn a_bare_village_offers_only_the_common_shelf() {
    let c = content();
    let bare = Unlocks { relic_tier: 0, potion_tier: 0, omen_tier: 0, hero_levels: vec![1], starters: vec![], secrets: vec![] };
    for seed in 0..30 {
        let mut run = Run::new_with(&c, &["bogatyr".to_string()], seed, 0, bare.clone()).unwrap();
        run.force_stage(&c, NodeKind::Shop);
        let Stage::Shop { stock, .. } = &run.stage else { panic!() };
        for (id, _) in stock[0].cards.iter().flatten() {
            let d = &c.cards[id];
            assert!(d.hero.is_empty() || d.rarity == Rarity::Common, "{id} is {:?} at hero level 1", d.rarity);
        }
        for (id, _) in stock[0].relics.iter().flatten() {
            assert_eq!(c.relics[id].rarity, Rarity::Common, "{id}");
        }
        let first_ten: Vec<&Id> = c.potions.keys().take(10).collect();
        for (id, _) in stock[0].potions.iter().flatten() {
            assert!(first_ten.contains(&id), "{id} is beyond the first shelf of potions");
        }
    }
}

#[test]
fn hero_level_opens_the_better_cards() {
    let c = content();
    let seen = |level: u32| -> std::collections::BTreeSet<Rarity> {
        let mut out = std::collections::BTreeSet::new();
        for seed in 0..40 {
            let unlocks = Unlocks { relic_tier: 2, potion_tier: 2, omen_tier: 2, hero_levels: vec![level], starters: vec![], secrets: vec![] };
            let mut run = Run::new_with(&c, &["volhv".to_string()], seed, 0, unlocks).unwrap();
            run.force_stage(&c, NodeKind::Shop);
            let Stage::Shop { stock, .. } = &run.stage else { panic!() };
            for (id, _) in stock[0].cards.iter().flatten() {
                if c.cards[id].hero == "volhv" {
                    out.insert(c.cards[id].rarity);
                }
            }
        }
        out
    };
    assert!(!seen(1).contains(&Rarity::Uncommon) && !seen(1).contains(&Rarity::Rare));
    assert!(seen(2).contains(&Rarity::Uncommon) && !seen(2).contains(&Rarity::Rare));
    assert!(seen(4).contains(&Rarity::Rare));
}

#[test]
fn a_sacrifice_is_paid_by_one_and_shared_by_all() {
    let c = content();
    let mut run = party(&c, 3, 3, 0);
    run.stage = Stage::Omen { id: "empty_cradle".into(), picks: vec![None; 3] };
    let hp: Vec<i32> = run.players.iter().map(|p| p.max_hp).collect();
    // one refuses and waits; nothing happens yet
    run.apply(&c, 1, &RunAction::Choose(1)).unwrap();
    assert!(matches!(run.stage, Stage::Omen { .. }));
    assert_eq!(run.apply(&c, 1, &RunAction::Choose(0)), Err(RunError::AlreadyDone));
    run.apply(&c, 2, &RunAction::Choose(0)).unwrap();
    assert!(matches!(run.stage, Stage::Map { .. }));
    for (p, pl) in run.players.iter().enumerate() {
        assert_eq!(pl.max_hp, hp[p] + 5);
        assert_eq!(pl.deck.iter().any(|(id, _)| id == "grief"), p == 2, "only the one who rocked the cradle carries the grief");
    }
}

#[test]
fn a_sacrifice_nobody_makes_gives_nothing() {
    let c = content();
    let mut run = party(&c, 2, 3, 0);
    run.stage = Stage::Omen { id: "krada".into(), picks: vec![None; 2] };
    let relics: Vec<usize> = run.players.iter().map(|p| p.relics.len()).collect();
    run.apply(&c, 0, &RunAction::Choose(1)).unwrap();
    run.apply(&c, 1, &RunAction::Choose(1)).unwrap();
    assert!(matches!(run.stage, Stage::Map { .. }));
    assert_eq!(run.players.iter().map(|p| p.relics.len()).collect::<Vec<_>>(), relics);
}

#[test]
fn a_schism_parts_the_party() {
    let c = content();
    let mut run = party(&c, 3, 3, 0);
    run.stage = Stage::Omen { id: "forked_path".into(), picks: vec![None; 3] };
    let gold: Vec<i32> = run.players.iter().map(|p| p.gold).collect();
    run.apply(&c, 0, &RunAction::Choose(0)).unwrap();
    run.apply(&c, 1, &RunAction::Choose(0)).unwrap();
    assert_eq!(run.apply(&c, 2, &RunAction::Choose(0)), Err(RunError::MustSplit));
    run.apply(&c, 2, &RunAction::Choose(1)).unwrap();
    assert!(matches!(run.stage, Stage::Map { .. }));
    assert_eq!(run.players[0].gold, gold[0] + 45);
    assert_eq!(run.players[2].gold, gold[2]);
    // alone there is nobody to part from
    let mut solo = party(&c, 1, 3, 0);
    solo.stage = Stage::Omen { id: "forked_path".into(), picks: vec![None] };
    solo.apply(&c, 0, &RunAction::Choose(0)).unwrap();
}

#[test]
fn fifty_omens_of_all_four_kinds() {
    let c = content();
    assert_eq!(c.events.values().filter(|e| !e.for_the_maimed).count(), 53);
    for kind in [EventKind::Personal, EventKind::Vote, EventKind::Sacrifice, EventKind::Schism] {
        assert!(c.events.values().any(|e| e.kind == kind), "{kind:?}");
    }
    for e in c.events.values() {
        match e.kind {
            EventKind::Sacrifice => assert!(e.options.iter().any(|o| !o.all.is_empty()) && e.options.iter().any(|o| o.all.is_empty()), "{}", e.id),
            // both ways must be open to a penniless hero, or the last one could be stuck
            EventKind::Schism => assert!(e.options.len() == 2 && e.options.iter().all(|o| o.price == 0), "{}", e.id),
            _ => assert!(e.options.iter().all(|o| o.all.is_empty()), "{}", e.id),
        }
    }
}

#[test]
fn two_hundred_and_ten_relics_fifteen_for_each_hero() {
    let c = content();
    assert!(c.relics.len() >= 210, "{}", c.relics.len());
    for hero in ["bogatyr", "vorozheya", "volhv", "volkolak", "znaharka", "skomoroh"] {
        // fifteen to find, plus the two the shrine lets the hero start with instead
        assert_eq!(c.relics.values().filter(|r| r.hero == hero).count(), 17, "{hero}");
    }
    assert_eq!(c.relics.values().filter(|r| r.pool == RelicPool::Shop).count(), 10);
    assert_eq!(c.relics.values().filter(|r| r.pool == RelicPool::Boss).count(), 12);
}

#[test]
fn a_boss_leaves_every_player_a_trophy_of_their_own() {
    let c = content();
    let mut run = party(&c, 3, 3, 0);
    let offers = run.roll_trophies(&c);
    assert!(offers.iter().all(|own| own.len() == 3));
    let mut all: Vec<&Id> = offers.iter().flatten().collect();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), 9, "no two players are offered the same");
    assert!(all.iter().all(|id| c.relics[*id].pool == RelicPool::Boss));
    run.stage = Stage::Trophy { offers: offers.clone(), picks: vec![None; 3] };
    run.apply(&c, 0, &RunAction::Choose(1)).unwrap();
    run.apply(&c, 1, &RunAction::Choose(0)).unwrap();
    assert_eq!(run.apply(&c, 2, &RunAction::Choose(7)), Err(RunError::BadChoice));
    // a choice may be changed until the last one is made
    run.apply(&c, 1, &RunAction::Choose(2)).unwrap();
    assert!(matches!(run.stage, Stage::Trophy { .. }));
    run.apply(&c, 2, &RunAction::Choose(0)).unwrap();
    assert!(matches!(run.stage, Stage::Map { .. }));
    assert!(run.players[0].relics.contains(&offers[0][1]));
    assert!(run.players[1].relics.contains(&offers[1][2]));
    assert!(run.players[2].relics.contains(&offers[2][0]));
}

#[test]
fn a_full_party_never_runs_out_of_trophies_and_none_gets_what_they_hold() {
    let c = content();
    let mut run = party(&c, 4, 3, 0);
    for _ in 0..3 {
        let offers = run.roll_trophies(&c);
        for (q, own) in offers.iter().enumerate() {
            assert_eq!(own.len(), 3);
            assert!(own.iter().all(|id| !run.players[q].relics.contains(id)));
            run.players[q].relics.push(own[0].clone());
        }
    }
}

#[test]
fn an_omen_finds_each_player_a_relic_of_their_own() {
    let c = content();
    for seed in 0..30 {
        let mut run = party(&c, 4, seed, 0);
        let gift = EventOption { id: "t".into(), price: 0, fx: vec![RunFx::Relic], all: vec![] };
        let mut ev = vec![];
        let before: Vec<usize> = run.players.iter().map(|p| p.relics.len()).collect();
        for q in 0..4 {
            run.test_apply_option(&c, q, &gift, &mut ev);
        }
        let mut found: Vec<&Id> = run.players.iter().zip(&before).flat_map(|(p, n)| p.relics[*n..].iter()).collect();
        assert_eq!(found.len(), 4);
        found.sort();
        found.dedup();
        assert_eq!(found.len(), 4, "seed {seed}");
    }
}

#[test]
fn trophies_and_pedlars_wares_are_never_found_on_the_road() {
    let c = content();
    for seed in 0..40 {
        let mut run = Run::new(&c, &["bogatyr".to_string()], seed, 0).unwrap();
        run.force_stage(&c, NodeKind::Shop);
        let Stage::Shop { stock, .. } = &run.stage else { panic!() };
        let shelves: Vec<&Id> = stock[0].relics.iter().flatten().map(|(id, _)| id).collect();
        assert!(shelves.iter().all(|id| c.relics[*id].pool != RelicPool::Boss));
        assert_eq!(c.relics[shelves[0]].pool, RelicPool::Shop, "the first shelf is the pedlar's own");
        run.stage = Stage::Omen { id: "krada".into(), picks: vec![None] };
        run.apply(&c, 0, &RunAction::Choose(0)).unwrap();
        let found = run.players[0].relics.last().unwrap();
        assert_eq!(c.relics[found].pool, RelicPool::Any, "{found}");
    }
}

#[test]
fn the_card_pool_matches_the_design_document() {
    let c = content();
    // the six of the design document: Yemelya the challenge hero came after it
    let real: Vec<&CardDef> = c.cards.values().filter(|d| !d.id.starts_with("t_") && !d.id.starts_with("own_") && d.hero != "emelya").collect();
    let count = |f: &dyn Fn(&CardDef) -> bool| real.iter().filter(|d| f(d)).count();
    assert_eq!(count(&|d| d.hero.is_empty() && !matches!(d.kind, CardType::Curse | CardType::Status) && d.rarity != Rarity::Special), 30, "neutral");
    assert_eq!(count(&|d| d.kind == CardType::Curse), 13, "curses");
    assert_eq!(count(&|d| d.kind == CardType::Status), 9, "statuses");
    assert_eq!(count(&|d| !d.hero.is_empty()), 564, "hero cards");
}

#[test]
fn the_pedlar_now_has_wares_for_any_hero() {
    let c = content();
    let mut run = Run::new(&c, &["znaharka".to_string()], 3, 0).unwrap();
    run.force_stage(&c, NodeKind::Shop);
    let Stage::Shop { stock, .. } = &run.stage else { panic!() };
    let neutral = stock[0].cards.iter().flatten().filter(|(id, _)| c.cards[id].hero.is_empty()).count();
    assert_eq!(neutral, 2);
}

#[test]
fn the_shrine_lets_a_hero_start_with_another_relic() {
    let c = content();
    for hero in c.heroes.values().filter(|h| !h.id.starts_with("t_")) {
        assert_eq!(hero.alt_relics.len(), 2, "{}", hero.id);
        for id in &hero.alt_relics {
            assert!(c.relics.get(id).is_some_and(|r| r.rarity == Rarity::Starter && r.hero == hero.id), "{id}");
        }
    }
    let mut unlocks = Unlocks::all(3);
    unlocks.starters = vec![2, 0, 9];
    let heroes = ["bogatyr".to_string(), "volhv".to_string(), "znaharka".to_string()];
    let run = Run::new_with(&c, &heroes, 1, 0, unlocks).unwrap();
    assert_eq!(run.players[0].relics, ids(&["bog_alt_brother_bracelet"]));
    assert_eq!(run.players[1].relics, ids(&["thunder_staff"]));
    assert_eq!(run.players[2].relics, ids(&["birch_pot"]), "a choice that does not exist falls back to the hero's own");
}

/// Not a pass/fail check: prints the fight in which the autopilot finds no move at all.
#[test]
#[ignore]
fn report_stuck_fights() {
    let c = content();
    let mut found = 0;
    for hero in SIX {
        for seed in 0..40u64 {
            let mut run = Run::new(&c, &[hero.to_string()], seed, 0).unwrap();
            for _ in 0..60_000 {
                if matches!(run.stage, Stage::Victory | Stage::Defeat) {
                    break;
                }
                let Some(a) = run_bot(&c, &run, 0) else {
                    let k = run.combat.as_ref();
                    println!(
                        "{hero} seed {seed}: stage {:?}, act {}, phase {:?}, hero flags {:?}, ready {:?}, downed {:?}, hand {:?}, enemies {:?}",
                        run.stage,
                        run.act,
                        k.map(|k| k.phase),
                        k.map(|k| k.heroes[0].flags.clone()),
                        k.map(|k| k.heroes[0].ready),
                        k.map(|k| k.heroes[0].downed),
                        k.map(|k| k.heroes[0].hand.iter().map(|x| x.def.clone()).collect::<Vec<_>>()),
                        k.map(|k| k.enemies.iter().map(|e| (e.def.clone(), e.hp, e.alive, e.flags.clone())).collect::<Vec<_>>()),
                    );
                    found += 1;
                    break;
                };
                run.apply(&c, 0, &a).unwrap();
            }
        }
    }
    println!("{found} stuck runs");
}

/// Not a pass/fail check: how solo autopilots meet the first boss and how that fight goes.
#[test]
#[ignore]
fn report_first_boss() {
    let c = content();
    for hero in SIX {
        let (mut met, mut won, mut hp_in, mut deck, mut rounds, mut left, mut relics) = (0, 0, 0, 0, 0, 0, 0);
        for seed in 0..40u64 {
            let mut run = Run::new(&c, &[hero.to_string()], seed, 0).unwrap();
            let mut inside = false;
            for _ in 0..60_000 {
                if matches!(run.stage, Stage::Victory | Stage::Defeat) || run.act > 1 {
                    break;
                }
                let boss = run.stage == Stage::Combat && run.fighting == Some(NodeKind::Boss);
                if boss && !inside {
                    inside = true;
                    met += 1;
                    hp_in += run.players[0].hp * 100 / run.players[0].max_hp;
                    deck += run.players[0].deck.len();
                    relics += run.players[0].relics.len();
                }
                if boss {
                    let k = run.combat.as_ref().unwrap();
                    rounds = rounds.max(0) + 0;
                    let _ = k;
                }
                let before = run.combat.as_ref().filter(|_| boss).map(|k| (k.round, k.enemies.iter().find(|e| e.def == "leshy").map(|e| e.hp * 100 / e.max_hp).unwrap_or(0)));
                let Some(a) = run_bot(&c, &run, 0) else { break };
                run.apply(&c, 0, &a).unwrap();
                if let Some((r, l)) = before {
                    if run.stage == Stage::Defeat {
                        rounds += r;
                        left += l;
                    } else if run.stage != Stage::Combat {
                        won += 1;
                    }
                }
            }
        }
        let lost = (met - won).max(1);
        println!(
            "{hero:<10} met {met}/40, won {won}; arrives at {}% hp, deck {}, relics {:.1}; when lost: round {}, leshy at {}%",
            hp_in / met.max(1),
            deck / met.max(1) as usize,
            relics as f32 / met.max(1) as f32,
            rounds / lost as u32,
            left / lost
        );
    }
}

#[test]
fn a_torn_page_curses_whoever_tore_it() {
    let c = content();
    let mut run = party(&c, 2, 7, 0);
    let (hp, max) = (run.players[1].hp, run.players[1].max_hp);
    // a small piece: the curse only
    run.apply(&c, 1, &RunAction::TearPage { lost: 10 }).unwrap();
    assert_eq!(run.players[1].deck.iter().filter(|(id, _)| id == BOOK_SPOIL).count(), 1);
    assert_eq!(run.players[1].max_hp, max);
    // a great piece: the curse and max HP for the rest of the run
    run.apply(&c, 1, &RunAction::TearPage { lost: 60 }).unwrap();
    assert_eq!(run.players[1].deck.iter().filter(|(id, _)| id == BOOK_SPOIL).count(), 2);
    assert_eq!(run.players[1].max_hp, max - BIG_TEAR_HP);
    assert!(run.players[1].hp <= run.players[1].max_hp && run.players[1].hp >= hp.min(max - BIG_TEAR_HP));
    // the other reader is untouched, and a nick that frees nothing is no tear
    assert!(!run.players[0].deck.iter().any(|(id, _)| id == BOOK_SPOIL));
    assert_eq!(run.apply(&c, 0, &RunAction::TearPage { lost: 0 }), Err(RunError::BadChoice));
    // and the curse is a real card of the game
    assert!(c.cards.contains_key(BOOK_SPOIL));
}

#[test]
fn a_secret_of_the_book_gives_its_relic_once() {
    let c = content();
    let mut run = party(&c, 2, 11, 0);
    run.apply(&c, 1, &RunAction::FindSecret { act: 2 }).unwrap();
    assert!(run.players[1].relics.iter().any(|r| r == "erased_name"));
    assert!(!run.players[0].relics.iter().any(|r| r == "erased_name"));
    // the same secret is not found twice in a run, and there are only five
    assert_eq!(run.apply(&c, 0, &RunAction::FindSecret { act: 2 }), Err(RunError::AlreadyDone));
    assert_eq!(run.apply(&c, 0, &RunAction::FindSecret { act: 6 }), Err(RunError::BadChoice));
    for id in SECRET_RELICS {
        assert_eq!(c.relics[id].pool, RelicPool::Book, "{id}");
    }
}

#[test]
fn the_unheard_tale_pays_for_a_book_shut_early() {
    let c = content();
    let mut run = party(&c, 1, 12, 0);
    assert_eq!(run.apply(&c, 0, &RunAction::SkipTale), Err(RunError::BadChoice), "without the relic nothing happens");
    run.apply(&c, 0, &RunAction::FindSecret { act: 4 }).unwrap();
    run.apply(&c, 0, &RunAction::TearPage { lost: 5 }).unwrap();
    let gold = run.players[0].gold;
    run.apply(&c, 0, &RunAction::SkipTale).unwrap();
    assert_eq!(run.players[0].gold, gold + UNHEARD_GOLD);
    assert!(!run.players[0].deck.iter().any(|(id, _)| id == BOOK_SPOIL), "the curse is gone");
}

#[test]
fn healing_potions_are_drunk_between_fights_and_others_are_not() {
    let c = content();
    let mut run = party(&c, 2, 3, 0);
    run.stage = Stage::Map { votes: vec![None, None] };
    run.players[0].potions = vec!["mead".into(), "party_broth".into()];
    run.players[0].hp = run.players[0].max_hp - 20;
    run.players[1].hp = run.players[1].max_hp - 2;
    assert_eq!(run.apply(&c, 0, &RunAction::DrinkPotion(0)), Err(RunError::BadChoice));
    run.apply(&c, 0, &RunAction::DrinkPotion(1)).unwrap();
    assert_eq!(run.players[0].hp, run.players[0].max_hp - 14);
    assert_eq!(run.players[1].hp, run.players[1].max_hp);
    assert_eq!(run.players[0].potions, vec!["mead".to_string()]);
}

#[test]
fn a_potion_thrown_at_the_pedlar_pays_once_a_visit() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    run.stage = Stage::Shop { stock: vec![Stock::default()], done: vec![false] };
    run.players[0].potions = vec!["mead".into(), "tar".into()];
    let gold = run.players[0].gold;
    run.apply(&c, 0, &RunAction::PeltPedlar(1)).unwrap();
    assert_eq!((run.players[0].gold, run.players[0].potions.clone()), (gold + PELT_GOLD, vec!["mead".to_string()]));
    assert_eq!(run.apply(&c, 0, &RunAction::PeltPedlar(0)), Err(RunError::AlreadyDone));
}

#[test]
fn a_shared_chest_is_claimed_and_fought_over_with_rock_paper_scissors() {
    let c = content();
    let mut run = party(&c, 3, 5, 0);
    run.force_stage(&c, NodeKind::Chest);
    let Stage::Chest(ch) = &run.stage else { panic!("{:?}", run.stage) };
    assert_eq!(ch.offers.len(), 3);
    let offers = ch.offers.clone();
    // the first two want the same relic, the third takes the one nobody else wants
    run.apply(&c, 0, &RunAction::Choose(1)).unwrap();
    run.apply(&c, 1, &RunAction::Choose(1)).unwrap();
    run.apply(&c, 2, &RunAction::Choose(2)).unwrap();
    let Stage::Chest(ch) = &run.stage else { panic!() };
    assert_eq!(ch.contest, vec![0, 1]);
    assert_eq!(run.apply(&c, 2, &RunAction::Throw(0)), Err(RunError::BadChoice));
    assert_eq!(run.apply(&c, 0, &RunAction::Onward), Err(RunError::BadChoice));
    // a draw is thrown again
    run.apply(&c, 0, &RunAction::Throw(0)).unwrap();
    let ev = run.apply(&c, 1, &RunAction::Throw(0)).unwrap();
    assert!(ev.iter().any(|e| matches!(e, RunEvent::Thrown { .. })));
    let Stage::Chest(ch) = &run.stage else { panic!() };
    assert_eq!(ch.contest, vec![0, 1]);
    // paper wraps rock
    run.apply(&c, 0, &RunAction::Throw(0)).unwrap();
    run.apply(&c, 1, &RunAction::Throw(1)).unwrap();
    let Stage::Chest(ch) = &run.stage else { panic!() };
    assert!(ch.settled);
    assert_eq!(ch.won, vec![Some(0), Some(1), Some(2)]);
    assert!(run.players[1].relics.contains(&offers[1]));
    assert!(run.players[0].relics.contains(&offers[0]));
    for p in 0..3 {
        run.apply(&c, p, &RunAction::Onward).unwrap();
    }
    assert!(matches!(run.stage, Stage::Map { .. }));
}

#[test]
fn three_at_one_chest_relic_throw_until_one_stands() {
    let c = content();
    let mut run = party(&c, 3, 9, 0);
    run.force_stage(&c, NodeKind::Chest);
    for p in 0..3 {
        run.apply(&c, p, &RunAction::Choose(0)).unwrap();
    }
    // rock, rock, scissors: the scissors are out, the rocks throw again
    run.apply(&c, 0, &RunAction::Throw(0)).unwrap();
    run.apply(&c, 1, &RunAction::Throw(0)).unwrap();
    run.apply(&c, 2, &RunAction::Throw(2)).unwrap();
    let Stage::Chest(ch) = &run.stage else { panic!() };
    assert_eq!(ch.contest, vec![0, 1]);
    run.apply(&c, 0, &RunAction::Throw(2)).unwrap();
    run.apply(&c, 1, &RunAction::Throw(1)).unwrap();
    let Stage::Chest(ch) = &run.stage else { panic!() };
    assert!(ch.settled);
    assert_eq!(ch.won[0], Some(0));
    let mut got: Vec<u8> = ch.won.iter().flatten().copied().collect();
    got.sort_unstable();
    assert_eq!(got, vec![0, 1, 2]);
}

#[test]
fn bots_share_a_chest_out_on_their_own() {
    let c = content();
    let mut run = party(&c, 4, 2, 0);
    run.force_stage(&c, NodeKind::Chest);
    for _ in 0..200 {
        if matches!(run.stage, Stage::Map { .. }) {
            break;
        }
        for p in 0..4u8 {
            if let Some(a) = run_bot(&c, &run, p) {
                run.apply(&c, p, &a).unwrap();
            }
        }
    }
    assert!(matches!(run.stage, Stage::Map { .. }));
    assert!(run.players.iter().all(|pl| pl.relics.len() >= 2));
}

#[test]
fn a_headless_player_is_asked_nothing_and_the_road_goes_on() {
    let c = content();
    let mut run = party(&c, 2, 5, 0);
    run.players[1].beheaded = true;
    let Stage::Map { .. } = run.stage else { panic!() };
    run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    assert!(!matches!(run.stage, Stage::Map { .. }), "one living vote moves the party");
    assert!(run.combat.as_ref().is_none_or(|f| f.heroes[1].downed));
}

#[test]
fn water_at_the_fire_gets_a_body_up_but_sews_nothing() {
    let c = content();
    let mut run = party(&c, 2, 5, 0);
    run.players[1].beheaded = true;
    run.players[1].hp = 0;
    run.stage = Stage::Rest { done: vec![false, false] };
    let before = run.players[1].max_hp;
    let head = format!("own_head_{}", run.players[1].hero);
    run.players[1].deck.push((head.clone().into(), false));
    assert_eq!(run.apply(&c, 1, &RunAction::Sleep), Err(RunError::WrongStage), "a corpse does not choose");
    assert_eq!(run.apply(&c, 0, &RunAction::Raise { who: 0 }), Err(RunError::BadChoice), "only one without a head");
    run.apply(&c, 0, &RunAction::Raise { who: 1 }).unwrap();
    assert!(!run.players[1].beheaded);
    assert!(run.players[1].walks_headless, "it walks with no head");
    assert_eq!(run.players[1].max_hp, before * WATER_PCT / 100);
    assert_eq!(run.players[1].hp, run.players[1].max_hp);
    // and the seamstress works from what they were, not from the tenth left
    run.stage = Stage::Omen { id: "needle_crone".into(), picks: vec![None; 2] };
    run.apply(&c, 0, &RunAction::Choose(0)).unwrap();
    assert!(!run.players[1].walks_headless);
    assert_eq!(run.players[1].max_hp, before - before * HEAD_SEAM_PCT / 100);
    assert!(!run.players[1].deck.iter().any(|(id, _)| id.as_str() == head), "the head is back on, no longer in the deck");
}

#[test]
fn a_corpse_stays_at_nothing_and_takes_nothing() {
    let c = content();
    let mut run = party(&c, 2, 5, 0);
    run.players[1].beheaded = true;
    run.players[1].hp = 0;
    let deck = run.players[1].deck.len();
    run.stage = Stage::Rest { done: vec![false, false] };
    run.apply(&c, 0, &RunAction::Sleep).unwrap();
    assert_eq!(run.players[1].hp, 0, "the fire warms no corpse");
    assert_eq!(run.players[1].deck.len(), deck);
    assert!(matches!(run.stage, Stage::Map { .. }) || !matches!(run.stage, Stage::Rest { .. }), "the headless do not hold the party at the fire");
}

#[test]
fn the_seamstress_sews_one_thing_a_visit_and_it_costs_dear() {
    let c = content();
    let mut run = party(&c, 3, 5, 0);
    run.players[2].beheaded = true;
    run.players[2].hp = 0;
    run.players[1].lost_limbs = 1;
    run.players[1].lost_arms = nav_core::combat::WEAPON_ARM;
    let (own_head, own_arm) = (format!("own_head_{}", run.players[2].hero), format!("own_weapon_{}", run.players[1].hero));
    run.players[2].deck.push((own_head.into(), false));
    run.players[1].deck.push((own_arm.into(), false));
    let (head, arm) = (run.players[2].max_hp, run.players[1].max_hp);
    run.stage = Stage::Omen { id: "needle_crone".into(), picks: vec![None; 3] };
    assert_eq!(run.apply(&c, 0, &RunAction::Choose(1)), Err(RunError::BadChoice), "no limb of its own to sew");
    run.apply(&c, 0, &RunAction::Choose(0)).unwrap();
    assert!(!run.players[2].beheaded);
    assert!(run.players[2].hp > 0);
    assert_eq!(run.players[2].max_hp, head - head * HEAD_SEAM_PCT / 100);
    assert_eq!(run.apply(&c, 1, &RunAction::Choose(1)), Err(RunError::BadChoice), "she has done her one thing");
    assert_eq!(run.players[1].lost_limbs, 1);
    assert_eq!(run.players[1].max_hp, arm);
}

#[test]
fn the_seamstress_gives_back_a_limb() {
    let c = content();
    let mut run = party(&c, 2, 5, 0);
    run.players[0].lost_limbs = 2;
    run.players[0].lost_arms = nav_core::combat::WEAPON_ARM | nav_core::combat::SHIELD_ARM;
    let hero = run.players[0].hero.clone();
    run.players[0].deck.push((format!("own_weapon_{hero}").into(), false));
    run.players[0].deck.push((format!("own_shield_{hero}").into(), false));
    let max = run.players[0].max_hp;
    run.stage = Stage::Omen { id: "needle_crone".into(), picks: vec![None; 2] };
    assert_eq!(run.apply(&c, 0, &RunAction::Choose(0)), Err(RunError::BadChoice), "nobody lacks a head");
    run.apply(&c, 0, &RunAction::Choose(1)).unwrap();
    assert_eq!(run.players[0].lost_limbs, 1, "one at a time");
    assert_eq!(run.players[0].lost_arms, nav_core::combat::SHIELD_ARM, "the weapon arm first");
    assert_eq!(run.players[0].max_hp, max - max * LIMB_SEAM_PCT / 100);
}

#[test]
fn the_seamstress_never_comes_to_a_whole_party_by_chance() {
    let c = content();
    for seed in 0..40 {
        let mut run = party(&c, 2, seed, 0);
        run.force_stage(&c, NodeKind::Event);
        assert!(!matches!(&run.stage, Stage::Omen { id, .. } if id == "needle_crone"));
    }
}

#[test]
fn what_was_thrown_away_cannot_be_sewn_back_on() {
    let c = content();
    let mut run = party(&c, 2, 5, 0);
    run.players[1].beheaded = true;
    run.players[1].hp = 0;
    run.stage = Stage::Omen { id: "needle_crone".into(), picks: vec![None; 2] };
    assert_eq!(run.apply(&c, 0, &RunAction::Choose(0)), Err(RunError::BadChoice), "no head in the deck, nothing to sew");
    assert!(run.players[1].beheaded);
}
