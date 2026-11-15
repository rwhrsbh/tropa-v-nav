mod common;
use common::*;
use nav_core::*;

/// A run played by the autopilot leaves a full tally: its fights, the cards played with what they
/// did, and every card offered with whether it was taken.
#[test]
fn a_run_tallies_its_fights_cards_and_offers() {
    let c = content();
    let mut run = Run::new(&c, &["bogatyr".to_string(), "volhv".to_string()], 11, 0).unwrap();
    let mut steps = 0;
    while !matches!(run.stage, Stage::Victory | Stage::Defeat) && steps < 4000 && run.stats.0.fights.len() < 6 {
        for p in 0..run.players.len() as u8 {
            if let Some(a) = run_bot(&c, &run, p) {
                let _ = run.apply(&c, p, &a);
            }
        }
        steps += 1;
    }
    let st = &run.stats.0;
    assert_eq!(st.players.len(), 2);
    assert!(st.fights.len() >= 3, "fights: {}", st.fights.len());
    let f = &st.fights[0];
    assert!(f.over && f.won && f.rounds > 0 && !f.enemies.is_empty(), "{f:?}");
    assert!(st.players[0].cards.values().any(|u| u.played > 0 && u.damage > 0), "cards: {:?}", st.players[0].cards);
    assert!(st.players.iter().all(|p| p.damage_dealt > 0));
    let cards: Vec<_> = st.offers.iter().filter(|o| o.kind == "card" && o.source == "reward").collect();
    assert!(cards.len() >= 6, "offers: {}", cards.len());
    assert!(cards.iter().any(|o| o.taken));
    assert!(cards.iter().any(|o| !o.taken), "a reward offers three and one is taken");
}
