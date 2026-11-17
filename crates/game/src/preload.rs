//! Pictures of the places the party may go next are read in before anyone votes, so a fight never
//! opens on a backdrop still loading: the fights an open node can lead to, the elites and the boss
//! when one lies ahead. What is no longer ahead is let go.

use crate::art::Art;
use crate::net::Net;
use crate::session::Db;
use bevy::prelude::*;
use nav_core::{Content, EncounterKind, NodeKind, Run};
use std::collections::HashMap;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, keep_ahead);
}

/// What is held ready, and the place in the run it was worked out for.
#[derive(Default)]
struct Ahead {
    place: Option<(u64, u8, Option<(u8, u8)>)>,
    held: HashMap<String, Handle<Image>>,
    /// Still to be read in, a few each frame.
    queue: Vec<String>,
}

/// Pictures asked for in one frame.
const PER_FRAME: usize = 2;

fn keep_ahead(mut ahead: Local<Ahead>, net: Option<Res<Net>>, db: Res<Db>, art: Res<Art>, assets: Res<AssetServer>) {
    let Some(run) = net.as_ref().and_then(|n| n.node.state.as_ref()) else {
        // no run, nothing to hold on to
        if ahead.place.is_some() {
            *ahead = Ahead::default();
        }
        return;
    };
    let ahead = &mut *ahead;
    // worked out while the party stands at the map, so nothing is being read in as a fight opens
    if matches!(run.stage, nav_core::Stage::Map { .. }) && ahead.place != Some((run.seed, run.act, run.at)) {
        ahead.place = Some((run.seed, run.act, run.at));
        let wanted = pictures_ahead(run, &db.0);
        ahead.held.retain(|stem, _| wanted.contains(stem));
        ahead.queue = wanted.into_iter().filter(|s| !ahead.held.contains_key(s)).collect();
    }
    for _ in 0..PER_FRAME {
        let Some(stem) = ahead.queue.pop() else { break };
        if let Some(handle) = art.image(&assets, &stem) {
            ahead.held.insert(stem, handle);
        }
    }
}

/// Every picture the next step of the road may need: its backdrops, and the foes met there.
fn pictures_ahead(run: &Run, c: &Content) -> Vec<String> {
    let act = run.act.min(3);
    let mut out = vec![format!("bg/act{act}_battle"), format!("map/sheet_act{act}")];
    for (_, kind) in run.choices() {
        let foes = |kind: EncounterKind, out: &mut Vec<String>| {
            for e in c.encounters.values().filter(|e| e.act == run.act && e.kind == kind) {
                for def in e.enemies.iter().chain(&e.solo).chain(&e.extra3).chain(&e.extra4) {
                    out.push(format!("bg/at_{def}"));
                    if let Some((kin, _)) = def.rsplit_once('_') {
                        out.push(format!("bg/at_{kin}"));
                    }
                    out.push(format!("enemies/{def}"));
                }
            }
        };
        match kind {
            NodeKind::Fight => {
                foes(EncounterKind::Normal, &mut out);
                out.extend(["", "_b", "_c", "_d", "_e"].iter().map(|v| format!("bg/act{act}_battle{v}")));
            }
            NodeKind::Elite => {
                foes(EncounterKind::Elite, &mut out);
                out.push(format!("bg/elite_act{act}"));
            }
            NodeKind::Boss => {
                foes(EncounterKind::Boss, &mut out);
                out.push(if run.act >= 4 { "bg/koschei".to_string() } else { format!("bg/boss_act{act}") });
                // and past the boss, the next act's road
                let on = (run.act + 1).min(3);
                out.push(format!("bg/act{on}_battle"));
                out.push(format!("map/sheet_act{on}"));
            }
            NodeKind::Shop => {
                out.push(format!("bg/shop_act{act}"));
                out.push("npc/pedlar".to_string());
                out.push("npc/counter".to_string());
            }
            NodeKind::Rest | NodeKind::Chest | NodeKind::Event => out.extend(["", "_b", "_c", "_d", "_e"].iter().map(|v| format!("bg/act{act}_battle{v}"))),
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nav_core::Run;

    fn content() -> Content {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
        Content::load_dir(&root).expect("content")
    }

    fn at_boss(c: &Content) -> Run {
        let mut run = Run::new(c, &["bogatyr".to_string()], 1, 0).unwrap();
        let last = run.map.floors.len() - 1;
        run.at = Some((last as u8, 0));
        run
    }

    #[test]
    fn what_lies_ahead_is_what_the_open_nodes_lead_to() {
        let c = content();
        let run = Run::new(&c, &["bogatyr".to_string()], 1, 0).unwrap();
        let ahead = pictures_ahead(&run, &c);
        assert!(ahead.contains(&"bg/act1_battle".to_string()));
        // the first floor is all fights: their haunts are ready, the boss's is not
        assert!(run.choices().iter().all(|(_, k)| *k != NodeKind::Boss));
        assert!(ahead.iter().any(|s| s.starts_with("bg/at_")), "{ahead:?}");
        assert!(!ahead.contains(&"bg/at_leshy".to_string()), "the boss is not ahead yet");
    }

    #[test]
    fn before_the_boss_his_arena_and_the_next_act_are_ready() {
        let c = content();
        let run = at_boss(&c);
        let ahead = pictures_ahead(&run, &c);
        assert!(ahead.contains(&"bg/boss_act1".to_string()));
        assert!(ahead.contains(&"bg/act2_battle".to_string()));
        assert!(ahead.contains(&"bg/at_leshy".to_string()), "{ahead:?}");
    }

    #[test]
    fn no_picture_is_asked_for_twice() {
        let c = content();
        let run = Run::new(&c, &["bogatyr".to_string()], 3, 0).unwrap();
        let ahead = pictures_ahead(&run, &c);
        let mut once = ahead.clone();
        once.dedup();
        assert_eq!(ahead, once);
    }
}
