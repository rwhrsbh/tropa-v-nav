//! Balance simulator (design doc, section 12): the autopilot plays many runs
//! per hero and per party and reports win rate, how far runs get, and how
//! cards fare. Cards picked under 5% of the time, or whose decks win over 70%,
//! are flagged for a second look.
//!
//!   cargo run --release -p nav_core --bin sim -- [--runs N] [--circle C] [--party 4]
//!
//! The autopilot is simple (block when threatened, otherwise hit), so absolute
//! win rates say more about the bot than about players. Compare heroes and
//! cards with each other, not with zero.

use nav_core::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Default, Clone)]
struct CardStat {
    offered: u32,
    taken: u32,
    /// Runs whose final deck held this card, and how many of those were won.
    in_deck: u32,
    wins: u32,
}

#[derive(Default)]
struct Tally {
    runs: u32,
    wins: u32,
    floors: u64,
    /// Runs cut short because a fight passed 300 rounds with no end in sight.
    stalled: u32,
    /// Runs that ended in each act (1..=4), lost.
    died_in: [u32; 5],
    cards: BTreeMap<Id, CardStat>,
}

/// Plays one run. Card rewards are taken at random (or skipped one time in four),
/// so that pick rates mean something.
fn play(c: &Content, heroes: &[Id], seed: u64, circle: u8, tally: &mut Tally) {
    let Some(mut run) = Run::new(c, heroes, seed, circle) else { return };
    let mut dice = Rng::new(seed ^ 0x5EED_CAFE);
    for _ in 0..400_000 {
        if matches!(run.stage, Stage::Victory | Stage::Defeat) {
            break;
        }
        if run.combat.as_ref().is_some_and(|k| k.round > 300) {
            tally.stalled += 1;
            break;
        }
        let mut moved = false;
        for p in 0..run.players.len() as u8 {
            let action = match &run.stage {
                Stage::Reward { rewards } if !rewards[p as usize].done && !rewards[p as usize].cards.is_empty() => {
                    let offered = &rewards[p as usize].cards;
                    for id in offered {
                        tally.cards.entry(id.clone()).or_default().offered += 1;
                    }
                    if dice.chance(25) {
                        Some(RunAction::TakeCard(None))
                    } else {
                        let i = dice.below(offered.len() as u32) as u8;
                        tally.cards.entry(offered[i as usize].clone()).or_default().taken += 1;
                        Some(RunAction::TakeCard(Some(i)))
                    }
                }
                _ => run_bot(c, &run, p),
            };
            if let Some(a) = action {
                if run.apply(c, p, &a).is_ok() {
                    moved = true;
                }
            }
        }
        if !moved {
            eprintln!("seed {seed}: stuck in {:?}", run.stage);
            if std::env::var("SIM_WHY").is_ok() {
                for p in 0..run.players.len() as u8 {
                    let a = run_bot(c, &run, p);
                    let r = a.as_ref().map(|a| run.clone().apply(c, p, a).map(|_| ()));
                    eprintln!("  p{p} {}: {:?} -> {:?}", run.players[p as usize].hero, a, r);
                }
                if let Some(k) = &run.combat {
                    for h in &k.heroes {
                        eprintln!("  {} hp {} ready {} picking {:?} energy {} hand {:?}", h.def, h.hp, h.ready, h.picking, h.energy, h.hand.iter().map(|c| (&c.def, c.alien)).collect::<Vec<_>>());
                    }
                }
            }
            break;
        }
    }
    let won = run.stage == Stage::Victory;
    tally.runs += 1;
    tally.wins += won as u32;
    tally.floors += run.floors_cleared as u64;
    if !won {
        tally.died_in[(run.act as usize).min(4)] += 1;
    }
    for pl in &run.players {
        let mut seen: Vec<&Id> = pl.deck.iter().map(|(id, _)| id).collect();
        seen.sort();
        seen.dedup();
        for id in seen {
            let s = tally.cards.entry(id.clone()).or_default();
            s.in_deck += 1;
            s.wins += won as u32;
        }
    }
}

fn report(label: &str, t: &Tally) {
    let pct = |a: u32, b: u32| if b == 0 { 0.0 } else { 100.0 * a as f32 / b as f32 };
    println!(
        "{label:<34} win {:>5.1}%   floors {:>4.1}   died in act 1/2/3/final: {}/{}/{}/{}   stalled {}",
        pct(t.wins, t.runs),
        t.floors as f32 / t.runs.max(1) as f32,
        t.died_in[1],
        t.died_in[2],
        t.died_in[3],
        t.died_in[4],
        t.stalled
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str, default: u64| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(default);
    let runs = flag("--runs", 1000);
    let circle = flag("--circle", 0) as u8;
    let party = flag("--party", 4).clamp(2, 4) as usize;

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let c = Content::load_dir(&dir).unwrap_or_else(|e| panic!("content: {e}"));
    let heroes: Vec<Id> = c.heroes.keys().cloned().collect();
    // `--only ivan,lisa` plays just that party, for chasing a problem
    if let Some(only) = args.iter().position(|a| a == "--only").and_then(|i| args.get(i + 1)) {
        let party: Vec<Id> = only.split(',').map(Id::from).collect();
        let mut t = Tally::default();
        for seed in 0..runs {
            play(&c, &party, seed, circle, &mut t);
        }
        report(only, &t);
        return;
    }
    println!("{runs} runs each, Circle {circle}\n");

    let mut all = Tally::default();
    println!("-- solo --");
    for h in &heroes {
        let mut t = Tally::default();
        for seed in 0..runs {
            play(&c, std::slice::from_ref(h), seed, circle, &mut t);
        }
        report(h, &t);
        for (id, s) in t.cards {
            let a = all.cards.entry(id).or_default();
            a.offered += s.offered;
            a.taken += s.taken;
            a.in_deck += s.in_deck;
            a.wins += s.wins;
        }
        all.runs += t.runs;
        all.wins += t.wins;
    }

    // every party of the chosen size would be too many for four; sample them by rotation
    println!("\n-- parties of {party} --");
    let mut combos: Vec<Vec<Id>> = Vec::new();
    for start in 0..heroes.len() {
        combos.push((0..party).map(|k| heroes[(start + k) % heroes.len()].clone()).collect());
    }
    if party == 2 {
        combos.clear();
        for a in 0..heroes.len() {
            for b in a + 1..heroes.len() {
                combos.push(vec![heroes[a].clone(), heroes[b].clone()]);
            }
        }
    }
    let per = (runs / 4).max(20);
    for combo in &combos {
        let mut t = Tally::default();
        for seed in 0..per {
            play(&c, combo, seed, circle, &mut t);
        }
        report(&combo.join("+"), &t);
    }

    println!("\n-- cards to look at (solo runs) --");
    let base = if all.runs == 0 { 0.0 } else { 100.0 * all.wins as f32 / all.runs as f32 };
    let mut flagged = 0;
    for (id, s) in &all.cards {
        if s.offered < 30 {
            continue;
        }
        let pick = 100.0 * s.taken as f32 / s.offered as f32;
        let win = if s.in_deck == 0 { 0.0 } else { 100.0 * s.wins as f32 / s.in_deck as f32 };
        let mut why = Vec::new();
        if pick < 5.0 {
            why.push(format!("picked {pick:.1}%"));
        }
        if win > 70.0 {
            why.push(format!("decks with it win {win:.1}%"));
        }
        if !why.is_empty() {
            flagged += 1;
            println!("  {id:<26} {}", why.join(", "));
        }
    }
    if flagged == 0 {
        println!("  none (the bot takes offered cards at random, so only win rates can flag here)");
    }
    println!("\noverall solo win rate {base:.1}%");

    // the cards that most often sit in winning decks, for a sense of what carries
    let mut best: Vec<(&Id, f32, u32)> = all.cards.iter().filter(|(_, s)| s.in_deck >= 40).map(|(id, s)| (id, 100.0 * s.wins as f32 / s.in_deck as f32, s.in_deck)).collect();
    best.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    println!("\n-- strongest and weakest cards by deck win rate --");
    for (id, win, n) in best.iter().take(8) {
        println!("  + {id:<26} {win:>5.1}%  ({n} decks)");
    }
    for (id, win, n) in best.iter().rev().take(8) {
        println!("  - {id:<26} {win:>5.1}%  ({n} decks)");
    }
}
