//! Rules of "Тропа в Навь". No rendering, no I/O beyond content loading,
//! no randomness outside the seeded [`rng::Rng`].

pub mod bot;
pub mod combat;
pub mod data;
mod hooks;
pub mod rng;
pub mod run;
pub mod state;
pub mod stats;

pub use combat::{apply, reduce, start, start_with, STALL_ROUND};
pub use data::*;
pub use rng::Rng;
pub use run::*;
pub use state::*;

impl HeroSetup {
    /// A fresh hero with the starting deck and relic.
    pub fn fresh(c: &Content, hero: &str) -> Option<HeroSetup> {
        let d = c.heroes.get(hero)?;
        Some(HeroSetup {
            def: d.id.clone(),
            hp: d.max_hp,
            max_hp: d.max_hp,
            deck: d.deck.iter().map(|id| (id.clone(), false)).collect(),
            relics: vec![d.relic.clone()],
            falls: 0,
            potions: vec![],
            lost_limbs: 0,
            lost_arms: 0,
            beheaded: false,
            walks_headless: false,
        })
    }
}
