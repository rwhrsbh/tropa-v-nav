//! Checks every foe against what the game needs to show and run it: its picture, the picture and
//! words of every intent and mark it puts on a hero, every hook its moves call, every foe it calls
//! up and card it slips in, and that its move order points at moves it has. Test-only.

#[cfg(test)]
mod tests {
    use crate::art::Art;
    use crate::l10n::{L10n, Lang};
    use nav_core::{Content, Effect};
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    fn walk<'a>(list: &'a [Effect], out: &mut Vec<&'a Effect>) {
        for e in list {
            out.push(e);
            match e {
                Effect::If(_, a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                Effect::Repeat(_, a) | Effect::Once(_, a) | Effect::OncePerTurn(_, a) | Effect::Every(_, _, a) | Effect::EveryThisTurn(_, _, a) => walk(a, out),
                _ => {}
            }
        }
    }

    #[test]
    fn every_foe_has_what_it_needs() {
        let root = root();
        let c = Content::load_dir(&root.join("data")).expect("content");
        let art = Art::new(root.clone());
        let langs: Vec<L10n> = [Lang::Ru, Lang::En].into_iter().map(|l| L10n::load(&art, l)).collect();
        let hooks_src = std::fs::read_to_string(root.join("../crates/core/src/hooks.rs")).unwrap() + &std::fs::read_to_string(root.join("../crates/core/src/combat.rs")).unwrap();
        let pic = |stem: &str| art.find(stem, "png", "gemini").is_some();
        let mut bad: BTreeSet<String> = BTreeSet::new();
        let mut flags: BTreeSet<String> = BTreeSet::new();
        for e in c.enemies.values() {
            if !pic(&format!("enemies/{}", e.id)) {
                bad.insert(format!("{}: no picture enemies/{}", e.id, e.id));
            }
            for l in &langs {
                if !l.has(&format!("enemy-{}", e.id)) {
                    bad.insert(format!("{}: no name enemy-{} in {:?}", e.id, e.id, l.lang));
                }
            }
            let n = e.moves.len();
            for (what, list) in [("opening", &e.opening), ("pattern", &e.pattern), ("phase2", &e.phase2)] {
                for i in list.iter().filter(|i| **i >= n) {
                    bad.insert(format!("{}: {what} points at move {i}, it has {n}", e.id));
                }
            }
            if e.pattern.is_empty() && n > 1 {
                // random picks: fine, but say so if nothing at all orders the moves
            }
            flags.extend(e.flags.iter().cloned());
            let mut all = Vec::new();
            for mv in &e.moves {
                if !pic(&format!("intents/{:?}", mv.intent)) {
                    bad.insert(format!("{}/{}: no intent picture intents/{:?}", e.id, mv.id, mv.intent));
                }
                for l in &langs {
                    if !l.has(&format!("move-{}", mv.id)) {
                        bad.insert(format!("{}/{}: no move name move-{} in {:?}", e.id, mv.id, mv.id, l.lang));
                    }
                }
                walk(&mv.effects, &mut all);
            }
            walk(&e.on_death, &mut all);
            for fx in all {
                match fx {
                    Effect::Apply(st, ..) | Effect::Remove(st, _) | Effect::Consume(st, _) => {
                        if !pic(&format!("status/{st:?}")) {
                            bad.insert(format!("{}: no status picture status/{st:?}", e.id));
                        }
                    }
                    Effect::Hook(h) => {
                        if !hooks_src.contains(&format!("\"{h}\"")) {
                            bad.insert(format!("{}: hook {h} is handled nowhere", e.id));
                        }
                    }
                    Effect::Summon(id, _) => {
                        if !c.enemies.contains_key(id) {
                            bad.insert(format!("{}: summons unknown {id}", e.id));
                        }
                    }
                    Effect::AddCard { card, .. } => {
                        if !c.cards.contains_key(card) {
                            bad.insert(format!("{}: slips in unknown card {card}", e.id));
                        }
                    }
                    Effect::Flag(f, _) if !f.starts_with("turn_") => {
                        flags.insert(f.clone());
                    }
                    _ => {}
                }
            }
        }
        // every mark a foe can carry or lay that the status row shows needs a picture and words
        for f in &flags {
            for l in &langs {
                if l.has(&format!("trait-{f}")) && !pic(&format!("status/{f}")) {
                    bad.insert(format!("flag {f}: shown as trait-{f} but no picture status/{f}"));
                }
            }
        }
        // every trait the row can show (hero marks set by hooks included)
        let ftl = std::fs::read_to_string(root.join("i18n/ru/rules.ftl")).unwrap_or_default() + &std::fs::read_to_string(root.join("i18n/ru/ui.ftl")).unwrap_or_default();
        for line in ftl.lines() {
            if let Some(key) = line.strip_prefix("trait-").and_then(|r| r.split_once(" =")).map(|(k, _)| k.trim()) {
                // lost limbs, the last breath and the house rules carry pictures of their own
                let own = key.starts_with("walks-") || key.starts_with("lost-") || key.starts_with("last-") || key.starts_with("house_rule-");
                if !key.ends_with("-help") && !own && !pic(&format!("status/{key}")) {
                    bad.insert(format!("trait-{key}: no picture status/{key}"));
                }
            }
        }
        for enc in c.encounters.values() {
            for id in enc.enemies.iter().chain(&enc.solo).chain(&enc.extra3) {
                if !c.enemies.contains_key(id) {
                    bad.insert(format!("encounter {}: unknown foe {id}", enc.id));
                }
            }
        }
        assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.into_iter().collect::<Vec<_>>().join("\n"));
    }
}
