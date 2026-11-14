//! Fluent-backed strings (old Russian, ru, en) and card text generated from effect data,
//! so the words on a card can never drift from what it does.

use bevy::prelude::*;
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use nav_core::*;

mod old_spelling;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    /// Russian told the old way: the tale's own voice, and the default for Russian speakers.
    /// Only what `i18n/ru-old` rewrites differs; every other string is the modern Russian one.
    Old,
    Ru,
    En,
}

impl Lang {
    /// Folder under `i18n/` and `voice/`.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Old => "ru-old",
            Lang::Ru => "ru",
            Lang::En => "en",
        }
    }
    /// The language whose strings fill in whatever this one leaves unsaid.
    fn base(self) -> Option<Lang> {
        (self == Lang::Old).then_some(Lang::Ru)
    }
    /// The language speech is played in: the player's own choice, or else the text's.
    pub fn voice(self, choice: u8) -> Lang {
        match choice {
            1 => Lang::Old,
            2 => Lang::Ru,
            3 => Lang::En,
            _ => self,
        }
    }
    /// The next language the menu button switches to.
    /// As `Settings::text_lang` keeps it.
    pub fn saved(self) -> u8 {
        match self {
            Lang::Old => 1,
            Lang::Ru => 2,
            Lang::En => 3,
        }
    }

    pub fn from_saved(n: u8) -> Option<Lang> {
        match n {
            1 => Some(Lang::Old),
            2 => Some(Lang::Ru),
            3 => Some(Lang::En),
            _ => None,
        }
    }

    /// From the word for it in `config.ron` and the installer's note.
    pub fn from_word(word: &str) -> Option<Lang> {
        match word.trim() {
            "en" => Some(Lang::En),
            "ru" => Some(Lang::Ru),
            "old" => Some(Lang::Old),
            _ => None,
        }
    }

    pub fn other(self) -> Lang {
        match self {
            Lang::Old => Lang::Ru,
            Lang::Ru => Lang::En,
            Lang::En => Lang::Old,
        }
    }
}

#[derive(Resource)]
pub struct L10n {
    pub lang: Lang,
    bundle: FluentBundle<FluentResource>,
}

impl L10n {
    pub fn load(art: &crate::art::Art, lang: Lang) -> L10n {
        // plural rules come from the base language: the old speech counts as Russian does
        let id: unic_langid::LanguageIdentifier = lang.base().unwrap_or(lang).code().parse().expect("valid language id");
        let mut bundle = FluentBundle::new_concurrent(vec![id]);
        // no bidi isolation marks: they render as boxes in our fonts
        bundle.set_use_isolating(false);
        for (from, overriding) in [(lang.base(), false), (Some(lang), lang.base().is_some())] {
            let Some(from) = from else { continue };
            let dir = format!("i18n/{}", from.code());
            for name in art.list(&dir, "ftl") {
                let file = format!("{dir}/{name}");
                let Some(src) = art.read_text(&file) else { continue };
                let res = match FluentResource::try_new(src) {
                    Ok(r) => r,
                    Err((r, errs)) => {
                        warn!("{file}: {} fluent syntax errors", errs.len());
                        r
                    }
                };
                if overriding {
                    bundle.add_resource_overriding(res);
                } else if let Err(errs) = bundle.add_resource(res) {
                    warn!("{file}: {} duplicate keys", errs.len());
                }
            }
        }
        L10n { lang, bundle }
    }

    /// A status amount as a player reads it: Fear is kept in tenths of a percent.
    pub fn status_amount(&self, st: nav_core::StatusKind, n: i32) -> String {
        if st != nav_core::StatusKind::Fear || n % 10 == 0 {
            return if st == nav_core::StatusKind::Fear { (n / 10).to_string() } else { n.to_string() };
        }
        let sep = if self.lang.code() == "en" { "." } else { "," };
        format!("{}{sep}{}", n / 10, (n % 10).abs())
    }

    pub fn has(&self, key: &str) -> bool {
        self.bundle.has_message(key)
    }

    pub fn t(&self, key: &str) -> String {
        self.tf(key, &[])
    }

    /// Looks up `key` with arguments. A missing key comes back as the key itself, visibly.
    pub fn tf(&self, key: &str, args: &[(&str, FluentValue)]) -> String {
        let Some(pattern) = self.bundle.get_message(key).and_then(|m| m.value()) else {
            return key.to_string();
        };
        let mut fa = FluentArgs::new();
        for (k, v) in args {
            fa.set(*k, v.clone());
        }
        let mut errs = vec![];
        let text = self.bundle.format_pattern(pattern, Some(&fa), &mut errs);
        // the old speech is printed as the old books printed it, whatever its source wrote
        if self.lang == Lang::Old {
            old_spelling::convert(&text)
        } else {
            text.into_owned()
        }
    }

    pub fn n(&self, key: &str, n: i32) -> String {
        self.tf(key, &[("n", FluentValue::from(n))])
    }

    pub fn card_name(&self, id: &str, upgraded: bool) -> String {
        let name = self.t(&format!("card-{id}"));
        if upgraded {
            format!("{name}+")
        } else {
            name
        }
    }

    fn amount(&self, a: &Amount) -> String {
        let scaled = |m: i32, what: String| if m == 1 { what } else { format!("{m}×{what}") };
        match a {
            Amount::N(n) => n.to_string(),
            Amount::X(m) => scaled(*m, "X".into()),
            Amount::Stat(st, _, m) => scaled(*m, self.t(&format!("status-{st:?}"))),
            Amount::Consumed(m) => scaled(*m, self.t("amount-consumed")),
            Amount::PlayedThisRound(m) => scaled(*m, self.t("amount-played")),
            Amount::Runes(kind, m) => scaled(*m, self.t(&format!("amount-rune-{kind:?}"))),
            Amount::Curses(_, m) => scaled(*m, self.t("amount-curses")),
            Amount::Pack(base, _) => base.to_string(),
            Amount::MissingHp(pct) => self.tf("amount-missing", &[("pct", (*pct).into())]),
            Amount::Discarded(m) => scaled(*m, self.t("amount-discarded")),
            Amount::DiscardedThisTurn(m) => scaled(*m, self.t("amount-discarded-turn")),
            Amount::HandSize(m) => scaled(*m, self.t("amount-hand")),
            Amount::ExhaustPile(m) => scaled(*m, self.t("amount-exhausted")),
            Amount::Hoard(base, _) => base.to_string(),
        }
    }

    fn cond(&self, c: &Cond) -> String {
        match c {
            Cond::Has(st, who, n) => self.tf(
                &format!("cond-has-{who:?}"),
                &[("s", self.t(&format!("status-{st:?}")).into()), ("n", (*n).into())],
            ),
            Cond::Link(kind) => self.tf("cond-link", &[("t", self.t(&format!("type-{kind:?}")).into())]),
            Cond::Fatal => self.t("cond-fatal"),
            Cond::Form(f) => self.t(&format!("cond-form-{f:?}")),
            Cond::Mask(m) => self.t(&format!("cond-mask-{m:?}")),
            Cond::Coop => self.t("cond-coop"),
            Cond::Not(inner) => self.tf("cond-not", &[("c", self.cond(inner).into())]),
        }
    }

    /// `fx-<effect>-<target>` with a fallback to `fx-<effect>`.
    fn fx(&self, name: &str, tgt: Option<Tgt>, args: &[(&str, FluentValue)]) -> String {
        if let Some(t) = tgt {
            let key = format!("fx-{name}-{t:?}");
            if self.has(&key) {
                return self.tf(&key, args);
            }
        }
        self.tf(&format!("fx-{name}"), args)
    }

    fn effects(&self, list: &[Effect]) -> String {
        let lines: Vec<String> = list.iter().map(|e| self.effect(e)).filter(|s| !s.is_empty()).collect();
        // the same thing done again and again reads once, with how many times
        let mut out: Vec<String> = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let mut k = 1;
            while i + k < lines.len() && lines[i + k] == lines[i] {
                k += 1;
            }
            out.push(if k == 1 { lines[i].clone() } else { self.times(&lines[i], k) });
            i += k;
        }
        out.join(" ")
    }

    /// One line of a card's text done `k` times: "Deal 3 damage 2 times."
    fn times(&self, line: &str, k: usize) -> String {
        let body = line.trim_end().trim_end_matches('.');
        self.tf("fx-times", &[("body", body.to_string().into()), ("k", (k as i32).into())])
    }

    fn effect(&self, e: &Effect) -> String {
        let n = |a: &Amount| -> FluentValue {
            match a {
                Amount::N(n) => (*n).into(),
                other => self.amount(other).into(),
            }
        };
        match e {
            Effect::Damage(a, t) => self.fx("damage", Some(*t), &[("n", n(a))]),
            Effect::Hits(a, k, t) => self.fx("hits", Some(*t), &[("n", n(a)), ("k", (*k).into())]),
            Effect::Block(a, t) => self.fx("block", Some(*t), &[("n", n(a))]),
            Effect::Guard(a, t) => self.fx("guard", Some(*t), &[("n", n(a))]),
            Effect::Apply(st, a, t) => {
                self.fx("apply", Some(*t), &[("n", n(a)), ("s", self.t(&format!("status-{st:?}")).into())])
            }
            Effect::Remove(st, t) => self.fx("remove", Some(*t), &[("s", self.t(&format!("status-{st:?}")).into())]),
            Effect::Consume(st, t) => self.fx("consume", Some(*t), &[("s", self.t(&format!("status-{st:?}")).into())]),
            Effect::Cleanse(t) => self.fx("cleanse", Some(*t), &[]),
            Effect::Draw(a) => self.fx("draw", None, &[("n", n(a))]),
            Effect::Energy(a, t) => self.fx("energy", Some(*t), &[("n", n(a))]),
            Effect::Heal(a, t) => self.fx("heal", Some(*t), &[("n", n(a))]),
            Effect::LoseHp(a, t) => self.fx("losehp", Some(*t), &[("n", n(a))]),
            Effect::Revive(pct, _) => self.fx("revive", None, &[("n", (*pct).into())]),
            Effect::AddCard { card, pile, to, count } => self.fx(
                &format!("addcard-{pile:?}"),
                Some(*to),
                &[("c", self.card_name(card, false).into()), ("n", (*count).into())],
            ),
            Effect::Taunt => self.t("fx-taunt"),
            Effect::If(c, then, otherwise) => {
                let mut s = self.tf("fx-if", &[("c", self.cond(c).into()), ("then", self.effects(then).into())]);
                if !otherwise.is_empty() {
                    s.push(' ');
                    s.push_str(&self.tf("fx-else", &[("else", self.effects(otherwise).into())]));
                }
                s
            }
            Effect::Repeat(a, body) => self.tf("fx-repeat", &[("n", n(a)), ("body", self.effects(body).into())]),
            Effect::Rune(kind, t) => self.fx("rune", Some(*t), &[("r", self.t(&format!("rune-{kind:?}")).into()), ("what", self.t(&format!("rune-{kind:?}-short")).into())]),
            Effect::Invoke => self.t("fx-invoke"),
            Effect::Shift => self.t("fx-shift"),
            Effect::Mask(m) => self.tf("fx-mask", &[("m", self.t(&format!("mask-{m:?}")).into())]),
            Effect::Summon(..) | Effect::StealGold(_) | Effect::StealGoldPct(_) | Effect::ScatterHoard | Effect::Flee => String::new(),
            Effect::ConsumeCurses(t) => self.fx("consumecurses", Some(*t), &[]),
            Effect::Flag(name, _) => self.t(&format!("flag-{name}")),
            Effect::Once(_, body) => self.tf("fx-once", &[("body", self.effects(body).into())]),
            Effect::OncePerTurn(_, body) => self.tf("fx-onceturn", &[("body", self.effects(body).into())]),
            Effect::Every(_, k, body) => self.tf("fx-every", &[("k", (*k).into()), ("body", self.effects(body).into())]),
            Effect::EveryThisTurn(_, k, body) => self.tf("fx-everyturn", &[("k", (*k).into()), ("body", self.effects(body).into())]),
            Effect::MaxHp(a, _) => self.fx("maxhp", None, &[("n", n(a))]),
            Effect::Echo(t) => self.fx("echo", Some(*t), &[]),
            Effect::Herb(herb) => self.tf("fx-herb", &[("h", self.t(&format!("herb-{herb}")).into()), ("spill", self.t(&format!("herb-{herb}-spill")).into())]),
            Effect::Brew => self.t("fx-brew"),
            Effect::Provoke => self.t("fx-provoke"),
            Effect::Discard(a) => self.fx("discard", None, &[("n", n(a))]),
            Effect::DiscardHand => self.t("fx-discardhand"),
            Effect::ExhaustCards(a) => self.fx("exhaustcards", None, &[("n", n(a))]),
            Effect::Recall(a) => self.fx("recall", None, &[("n", n(a))]),
            Effect::PikeWill => self.t("fx-pikewill"),
            // a hook with nothing to say to the player says nothing, not its key
            Effect::Hook(name) if !self.has(&format!("hook-{name}")) => String::new(),
            Effect::Hook(name) => self.t(&format!("hook-{name}")),
        }
    }

    /// Rules text of a card. A hand-written `card-<id>-desc` overrides the generated one.
    pub fn card_text(&self, id: &str, def: &CardDef, view: &CardView) -> String {
        let custom = format!("card-{id}-desc");
        let mut parts = Vec::new();
        if self.has(&custom) {
            parts.push(self.t(&custom));
        } else {
            let body = self.effects(&view.effects);
            if !body.is_empty() {
                parts.push(body);
            }
            for p in &view.passives {
                parts.push(self.tf(&format!("trigger-{:?}", p.on), &[("fx", self.effects(&p.effects).into())]));
            }
            if !def.in_hand_end.is_empty() {
                parts.push(self.tf("trigger-InHand", &[("fx", self.effects(&def.in_hand_end).into())]));
            }
            if !view.on_discard.is_empty() {
                parts.push(self.tf("trigger-OnDiscard", &[("fx", self.effects(&view.on_discard).into())]));
            }
        }
        for k in &view.keywords {
            if *k != Keyword::Coop {
                parts.push(format!("{}.", self.t(&format!("kw-{k:?}"))));
            }
        }
        once_at_the_end(parts.join(" "), &self.t("rune-pushed-out"))
    }

    /// Keywords and statuses a card mentions, for its hover tooltips.
    pub fn card_glossary(&self, view: &CardView) -> Vec<(String, String)> {
        let mut keys: Vec<String> = view.keywords.iter().map(|k| format!("kw-{k:?}")).collect();
        if view.drowse > 0 {
            keys.push("kw-DrowseCost".into());
        }
        fn walk(list: &[Effect], keys: &mut Vec<String>) {
            for e in list {
                match e {
                    Effect::Apply(st, ..) | Effect::Consume(st, _) => keys.push(format!("status-{st:?}")),
                    Effect::Damage(Amount::Stat(st, ..), _) => keys.push(format!("status-{st:?}")),
                    Effect::Guard(..) => keys.push("kw-Guard".into()),
                    Effect::Taunt => keys.push("kw-Taunt".into()),
                    Effect::Invoke => keys.push("kw-Invoke".into()),
                    Effect::Shift => keys.push("kw-Shift".into()),
                    Effect::Rune(kind, _) => {
                        keys.push(format!("rune-{kind:?}"));
                        keys.push("kw-Rune".into());
                    }
                    Effect::Mask(m) => keys.push(format!("mask-{m:?}")),
                    Effect::If(Cond::Link(_), a, b) => {
                        keys.push("kw-Link".into());
                        walk(a, keys);
                        walk(b, keys);
                    }
                    Effect::If(_, a, b) => {
                        walk(a, keys);
                        walk(b, keys);
                    }
                    Effect::Repeat(_, a) | Effect::Once(_, a) | Effect::OncePerTurn(_, a) | Effect::Every(_, _, a) | Effect::EveryThisTurn(_, _, a) => walk(a, keys),
                    Effect::ConsumeCurses(_) => keys.push("kw-Curse".into()),
                    Effect::Echo(_) => keys.push("kw-Echo".into()),
                    Effect::Herb(herb) => {
                        keys.push(format!("herb-{herb}"));
                        keys.push("kw-Brew".into());
                    }
                    Effect::Brew => keys.push("kw-Brew".into()),
                    Effect::PikeWill => keys.push("kw-PikeWill".into()),
                    Effect::Discard(_) | Effect::DiscardHand | Effect::Damage(Amount::Discarded(_) | Amount::DiscardedThisTurn(_), _) => keys.push("kw-Discard".into()),
                    Effect::ExhaustCards(_) => keys.push("kw-Exhaust".into()),
                    Effect::Hook(name) if name == "kolyada" => keys.push("kw-Rune".into()),
                    Effect::Hook(name) if name == "random_herb" => keys.push("kw-Brew".into()),
                    Effect::Damage(Amount::Runes(kind, _), _) | Effect::Block(Amount::Runes(kind, _), _) => {
                        keys.push(format!("rune-{kind:?}"));
                        keys.push("kw-Rune".into());
                    }
                    _ => {}
                }
            }
        }
        walk(&view.effects, &mut keys);
        for p in &view.passives {
            if matches!(p.on, nav_core::Trigger::Discarded) {
                keys.push("kw-Discard".into());
            }
            walk(&p.effects, &mut keys);
        }
        if !view.on_discard.is_empty() {
            keys.push("kw-Discard".into());
            walk(&view.on_discard, &mut keys);
        }
        keys.dedup();
        let mut seen = Vec::new();
        keys.retain(|k| {
            let fresh = !seen.contains(k);
            seen.push(k.clone());
            fresh
        });
        keys.into_iter()
            .filter(|k| self.has(&format!("{k}-help")))
            .map(|k| (self.t(&k), self.t(&format!("{k}-help"))))
            .collect()
    }
}

/// A sentence said of every rune on a card is said once, last of them.
fn once_at_the_end(text: String, sentence: &str) -> String {
    let Some(last) = text.rfind(sentence) else { return text };
    let (before, rest) = text.split_at(last);
    let kept = before.replace(&format!(" {sentence}"), "");
    format!("{kept}{rest}")
}

#[cfg(test)]
mod once_tests {
    use super::{once_at_the_end, L10n, Lang};

    #[test]
    fn no_card_shows_a_string_key() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let c = nav_core::Content::load_dir(&root.join("data")).expect("content");
        for lang in [Lang::Old, Lang::Ru, Lang::En] {
            let l = L10n::load(&crate::art::Art::new(root.clone()), lang);
            for def in c.cards.values() {
                for up in [false, true] {
                    let t = l.card_text(&def.id, def, &def.view(up, nav_core::Form::Human));
                    assert!(!t.contains("hook-") && !t.contains("fx-") && !t.contains("trigger-"), "{}: {t}", def.id);
                }
            }
        }
    }

    #[test]
    fn a_blow_struck_again_reads_once_with_how_many_times() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let c = nav_core::Content::load_dir(&root.join("data")).expect("content");
        for lang in [Lang::Ru, Lang::En] {
            let l = L10n::load(&crate::art::Art::new(root.clone()), lang);
            for def in c.cards.values() {
                let view = def.view(false, nav_core::Form::Human);
                let twice = view.effects.windows(2).any(|w| w[0] == w[1] && matches!(w[0], nav_core::Effect::Damage(..)));
                if twice {
                    let t = l.card_text(&def.id, def, &view);
                    println!("{lang:?} {}: {t}", def.id);
                    assert!(t.matches(&l.effect(&view.effects[0])).count() <= 1, "{}: {t}", def.id);
                }
            }
        }
    }

    #[test]
    fn the_sentence_stays_only_after_the_last_rune() {
        let t = "A: 6. Pushed. B: 4. Pushed. Exhaust.".to_string();
        assert_eq!(once_at_the_end(t, "Pushed."), "A: 6. B: 4. Pushed. Exhaust.");
        assert_eq!(once_at_the_end("A: 6. Pushed.".to_string(), "Pushed."), "A: 6. Pushed.");
        assert_eq!(once_at_the_end("Deal 6.".to_string(), "Pushed."), "Deal 6.");
    }
}
