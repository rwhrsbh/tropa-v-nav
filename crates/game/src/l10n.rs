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

thread_local! {
    /// Set while a card in the hand is told: a blind range reads as its two ends, not as "?".
    static SHOW_RANGES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Tells what is written inside with blind ranges shown as their ends, as the hand shows them.
pub fn with_ranges<T>(f: impl FnOnce() -> T) -> T {
    let was = SHOW_RANGES.with(|c| c.replace(true));
    let out = f();
    SHOW_RANGES.with(|c| c.set(was));
    out
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

    /// A status amount as a player reads it: Fear is kept in tenths of a percent and read as a
    /// percent (250 is 25%, 75 is 7,5%).
    pub fn status_amount(&self, st: nav_core::StatusKind, n: i32) -> String {
        if st != nav_core::StatusKind::Fear {
            return n.to_string();
        }
        if n % 10 == 0 {
            return format!("{}%", n / 10);
        }
        let sep = if self.lang.code() == "en" { "." } else { "," };
        format!("{}{sep}{}%", n / 10, (n % 10).abs())
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

    /// What a varying amount counts, as a noun phrase: "3 × Valor", "cards in hand".
    fn amount(&self, a: &Amount) -> String {
        self.amount_in(a, &|m| m.to_string())
    }

    /// The same, with the multiplier written by `mult`: Fear counts in tenths, so its "10 ×" reads "1 ×".
    fn amount_in(&self, a: &Amount, mult: &dyn Fn(i32) -> String) -> String {
        let scaled = |m: i32, what: String| if mult(m) == "1" { what } else { format!("{} × {what}", mult(m)) };
        match a {
            Amount::N(n) => n.to_string(),
            Amount::X(m) => scaled(*m, "X".into()),
            Amount::Stat(st, who, m) => {
                let name = self.t(&format!("status-{st:?}"));
                let key = format!("amount-stat-{who:?}");
                scaled(*m, if self.has(&key) { self.tf(&key, &[("s", name.into())]) } else { name })
            }
            Amount::Consumed(m) => scaled(*m, self.t("amount-consumed")),
            Amount::PlayedThisRound(m) => scaled(*m, self.t("amount-played")),
            Amount::Runes(kind, m) => scaled(*m, self.tf("amount-runes", &[("r", self.t(&format!("rune-{kind:?}")).into())])),
            Amount::Curses(_, m) => scaled(*m, self.t("amount-curses")),
            Amount::Pack(base, _) => base.to_string(),
            Amount::MissingHp(pct) => self.tf("amount-missing", &[("pct", (*pct).into())]),
            Amount::Discarded(m) => scaled(*m, self.t("amount-discarded")),
            Amount::DiscardedThisTurn(m) => scaled(*m, self.t("amount-discarded-turn")),
            Amount::HandSize(m) => scaled(*m, self.t("amount-hand")),
            Amount::ExhaustPile(m) => scaled(*m, self.t("amount-exhausted")),
            Amount::Hoard(base, _) => base.to_string(),
            // a number of chance: its ends in the hand, a question mark anywhere else
            Amount::Roll(lo, hi) => {
                if SHOW_RANGES.with(|c| c.get()) {
                    format!("{lo}–{hi}")
                } else {
                    "?".into()
                }
            }
            Amount::Flag(name, m) => scaled(*m, self.t(&format!("amount-flag-{name}"))),
            Amount::Part(st, by) => self.tf("amount-part", &[("s", self.t(&format!("status-{st:?}")).into()), ("n", (*by).into())]),
            Amount::Sum(list) => list.iter().map(|a| self.amount_in(a, mult)).collect::<Vec<_>>().join(" + "),
            Amount::AtLeast(a, n) => self.tf("amount-atleast", &[("a", self.amount_in(a, mult).into()), ("n", (*n).into())]),
            Amount::AliensInHand(m) => scaled(*m, self.t("amount-aliens")),
            Amount::MaxBlock => self.t("amount-maxblock"),
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
            Cond::Not(inner) if **inner == Cond::Coop => self.t("cond-solo"),
            Cond::Not(inner) if **inner == Cond::InPlate => self.t("cond-not-plate"),
            Cond::Not(inner) => self.tf("cond-not", &[("c", self.cond(inner).into())]),
            Cond::Flag(name, n) => self.tf(&format!("cond-flag-{name}"), &[("n", (*n).into())]),
            Cond::InPlate => self.t("cond-plate"),
            Cond::Attacking => self.t("cond-attacking"),
            Cond::Shielded => self.t("cond-shielded"),
            Cond::HpAtMost(n) => self.tf("cond-hp-at-most", &[("n", (*n).into())]),
            Cond::Flying => self.t("cond-flying"),
            Cond::AlienInHand => self.t("cond-alien-in-hand"),
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

    /// A status's name as the object of a verb ("Наложите Немоту"); the plain name where the
    /// language has no such form.
    fn status_obj(&self, st: StatusKind) -> String {
        let key = format!("status-{st:?}-acc");
        if self.has(&key) {
            self.t(&key)
        } else {
            self.t(&format!("status-{st:?}"))
        }
    }

    /// A clause put after a colon ("At the start of your turn: ..."), begun in lower case unless
    /// it opens with a game term or a name, which keep their capital.
    fn clause(&self, text: String) -> String {
        let first = text.split([' ', '.', ':', ',']).next().unwrap_or("");
        const TERMS: [&str; 34] = [
            "kw-Guard", "kw-Taunt", "kw-Invoke", "kw-Shift", "kw-Echo", "kw-Link", "kw-PikeWill", "kw-Respond", "side-Human", "side-Beast",
            "status-Strength", "status-Drowse", "status-TempStrength", "status-Fortitude", "status-Vulnerable", "status-Weak", "status-Rot", "status-Hex",
            "status-Bleed", "status-Fear", "status-Thorns", "status-HuntMark", "status-Valor", "status-Stun", "status-Mist", "status-Chained", "status-Gloom",
            "status-Forge", "status-Plate", "status-Luck", "status-Evade", "status-Charm", "status-Flattery", "kw-Alien",
        ];
        if TERMS.iter().any(|k| self.has(k) && self.t(k).split(' ').next() == Some(first)) {
            return text;
        }
        let mut chars = text.chars();
        match chars.next() {
            Some(c) => c.to_lowercase().chain(chars).collect(),
            None => text,
        }
    }

    fn effects(&self, list: &[Effect]) -> String {
        // statuses laid on the same one twice in a row read as one, summed
        let mut merged: Vec<Effect> = Vec::new();
        for e in list {
            if let (Effect::Apply(st, Amount::N(a), t), Some(Effect::Apply(pst, Amount::N(pa), pt))) = (e, merged.last_mut()) {
                if st == pst && t == pt {
                    *pa += a;
                    continue;
                }
            }
            merged.push(e.clone());
        }
        let lines: Vec<String> = merged.iter().map(|e| self.effect(e)).filter(|s| !s.is_empty()).collect();
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
        // a fixed number is said as it is; a varying one through the `-x` phrasing ("deal damage: 3 × Valor")
        let fixed = |a: &Amount| matches!(a, Amount::N(_) | Amount::Roll(..));
        let n = |a: &Amount| -> FluentValue {
            match a {
                Amount::N(n) => (*n).into(),
                other => self.amount(other).into(),
            }
        };
        let say = |name: &str, a: &Amount, t: Option<Tgt>| -> String {
            if fixed(a) {
                self.fx(name, t, &[("n", n(a))])
            } else {
                self.fx(&format!("{name}-x"), t, &[("n", n(a))])
            }
        };
        let body = |list: &[Effect]| -> FluentValue { self.clause(self.effects(list)).into() };
        let named = |st: &StatusKind| -> [(&str, FluentValue); 2] { [("s", self.t(&format!("status-{st:?}")).into()), ("so", self.status_obj(*st).into())] };
        match e {
            Effect::Damage(a, t) => say("damage", a, Some(*t)),
            Effect::Hits(a, k, t) => self.fx("hits", Some(*t), &[("n", n(a)), ("k", (*k).into())]),
            Effect::Block(a, t) => say("block", a, Some(*t)),
            Effect::Guard(a, t) => self.fx("guard", Some(*t), &[("n", n(a))]),
            Effect::Apply(st, a, t) => {
                // a status laid as much again as there already is: doubled
                if let Amount::Stat(of, _, 1) = a {
                    if of == st {
                        return self.fx("apply-double", Some(*t), &named(st));
                    }
                }
                // cards and relics give Fear in whole percent (the fight keeps it in tenths)
                let amount: FluentValue = match a {
                    Amount::N(v) if *st == StatusKind::Fear => format!("{v}%").into(),
                    other if *st == StatusKind::Fear => self.amount_in(other, &|m| format!("{m}%")).into(),
                    _ => n(a),
                };
                // laid on the one aimed at: a blessing is for an ally, a curse for a foe
                let name = if *t == Tgt::Target && !st.debuff() { "apply-ally" } else { "apply" };
                let name = if fixed(a) { name.to_string() } else { format!("{name}-x") };
                let [s, so] = named(st);
                self.fx(&name, Some(*t), &[("n", amount), s, so])
            }
            Effect::Remove(st, t) => self.fx("remove", Some(*t), &named(st)),
            Effect::Consume(st, t) => self.fx("consume", Some(*t), &named(st)),
            Effect::Cleanse(t) => self.fx("cleanse", Some(*t), &[]),
            Effect::Draw(Amount::Discarded(1)) => self.t("fx-draw-discarded"),
            Effect::Draw(a) => say("draw", a, None),
            Effect::Energy(a, t) => say("energy", a, Some(*t)),
            Effect::Heal(a, t) => say("heal", a, Some(*t)),
            Effect::LoseHp(a, t) => say("losehp", a, Some(*t)),
            Effect::Revive(pct, _) => self.fx("revive", None, &[("n", (*pct).into())]),
            Effect::AddCard { card, pile, to, count } => self.fx(
                &format!("addcard-{pile:?}"),
                Some(*to),
                &[("c", self.card_name(card, false).into()), ("n", (*count).into())],
            ),
            Effect::Taunt => self.t("fx-taunt"),
            Effect::If(c, then, otherwise) => {
                let mut s = self.tf("fx-if", &[("c", self.cond(c).into()), ("then", body(then))]);
                if !otherwise.is_empty() {
                    s.push(' ');
                    s.push_str(&self.tf("fx-else", &[("else", body(otherwise))]));
                }
                s
            }
            Effect::Repeat(Amount::Discarded(1), list) => self.tf("fx-repeat-discarded", &[("body", body(list))]),
            Effect::Repeat(a, list) => self.tf("fx-repeat", &[("n", n(a)), ("body", body(list))]),
            Effect::Rune(kind, t) => self.fx("rune", Some(*t), &[("r", self.t(&format!("rune-{kind:?}")).into())]),
            Effect::Invoke => self.t("fx-invoke"),
            Effect::Shift => self.t("fx-shift"),
            Effect::Mask(m) => self.tf("fx-mask", &[("m", self.t(&format!("mask-{m:?}")).into())]),
            Effect::Summon(..) | Effect::StealGold(_) | Effect::StealGoldPct(_) | Effect::ScatterHoard | Effect::Flee => String::new(),
            Effect::ConsumeCurses(t) => self.fx("consumecurses", Some(*t), &[]),
            Effect::Flag(name, _) => self.t(&format!("flag-{name}")),
            Effect::Once(_, list) => self.tf("fx-once", &[("body", body(list))]),
            Effect::OncePerTurn(_, list) => self.tf("fx-onceturn", &[("body", body(list))]),
            Effect::Every(_, k, list) => self.tf("fx-every", &[("k", (*k).into()), ("body", body(list))]),
            Effect::EveryThisTurn(_, k, list) => self.tf("fx-everyturn", &[("k", (*k).into()), ("body", body(list))]),
            Effect::MaxHp(a, _) => self.fx("maxhp", None, &[("n", n(a))]),
            Effect::Echo(t) => self.fx("echo", Some(*t), &[]),
            Effect::Herb(herb) => self.tf("fx-herb", &[("h", self.t(&format!("herb-{herb}")).into())]),
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
            Effect::Forge(a) => say("forge", a, None),
            Effect::Mark(Amount::N(n)) if *n >= 10 => self.t("fx-mark-all"),
            Effect::Mark(a) => say("mark", a, None),
            Effect::Plate(Amount::N(n)) if *n < 0 => self.fx("plate-spend", None, &[("n", (-*n).into())]),
            Effect::Plate(a) => say("plate", a, None),
            Effect::Reforge(a, from, _) => {
                if let Amount::Stat(..) = a {
                    self.t("fx-reforge-all-strength")
                } else {
                    self.fx("reforge", Some(*from), &[("n", n(a))])
                }
            }
            Effect::Upgrade(Amount::N(k), t) if *k >= 10 => self.fx("upgrade-all", Some(*t), &[]),
            Effect::Upgrade(a, t) => self.fx("upgrade", Some(*t), &[("n", n(a))]),
            Effect::Set(name, k, t) => {
                let key = format!("set-{name}-{t:?}");
                let key = if self.has(&key) { key } else { format!("set-{name}") };
                self.tf(&key, &[("n", (*k).into())])
            }
            Effect::DrawFor(a, t) => say("drawfor", a, Some(*t)),
            Effect::Transmute(all, t) => self.fx(if *all { "transmute-all" } else { "transmute" }, Some(*t), &[]),
            Effect::Pick { pile, from, then, top } => {
                let key = format!("fx-pick-{then:?}-{pile:?}");
                let key = if *from == Tgt::Me && self.has(&format!("{key}-Me")) { format!("{key}-Me") } else { key };
                self.tf(&key, &[("n", (*top as i32).into())])
            }
            Effect::Luck { foe, friend, keep } => {
                let mut s = if friend.is_empty() {
                    self.tf("fx-luck-evil", &[("foe", body(foe))])
                } else {
                    self.tf("fx-luck", &[("foe", body(foe)), ("friend", body(friend))])
                };
                s.push(' ');
                s.push_str(&self.t(if *keep { "fx-luck-keep" } else { "fx-luck-burn" }));
                s
            }
            Effect::DiscardRandom(a) => say("discardrandom", a, None),
            Effect::Alien { pile, count, .. } => self.fx(&format!("alien-{pile:?}"), None, &[("n", (*count).into())]),
            Effect::Stash(k) => self.fx("stash", None, &[("n", (*k).into())]),
        }
    }

    /// The whole text printed on a card in `form`: a werewolf card with two sides tells the
    /// other one too, on a line of its own, so the turn holds no surprise.
    pub fn full_card_text(&self, id: &str, def: &CardDef, upgraded: bool, form: nav_core::Form) -> String {
        if (def.hero != "volkolak" && def.hero != "kuzma") || !def.two_sided(upgraded) {
            return self.card_text(id, def, &def.view(upgraded, form));
        }
        // each side is played only in its own form: what a side does "as the beast" is settled
        // on it, and the line says only what happens (powers ask later, and keep the condition)
        fn settle(list: &[Effect], form: nav_core::Form) -> Vec<Effect> {
            let mut out = Vec::new();
            for e in list {
                match e {
                    Effect::If(Cond::Form(f), then, otherwise) => out.extend(settle(if *f == form { then } else { otherwise }, form)),
                    Effect::If(c, then, otherwise) => out.push(Effect::If(c.clone(), settle(then, form), settle(otherwise, form))),
                    other => out.push(other.clone()),
                }
            }
            out
        }
        let side_view = |f: nav_core::Form| {
            let mut v = def.view(upgraded, f);
            v.effects = settle(&v.effects, f);
            v.on_discard = settle(&v.on_discard, f);
            v
        };
        let other = if form == nav_core::Form::Beast { nav_core::Form::Human } else { nav_core::Form::Beast };
        let mine = side_view(form);
        let theirs = side_view(other);
        let text = self.card_text(id, def, &mine);
        // the keywords both sides share are said once, on this side
        let side = self.card_body(id, def, &theirs, |k| !mine.keywords.contains(k));
        // Kuzma's two sides are the forge and the plate, not a man and a beast
        let label = if def.hero == "kuzma" { format!("side-plate-{other:?}") } else { format!("side-{other:?}") };
        format!("{text}\n{}: {side}", self.t(&label))
    }

    /// The text of this very card: a marked card of chance tells the numbers thrown for it, one
    /// made blind tells its moved ends; anything else reads as `full_card_text`.
    pub fn instance_text(&self, card: &nav_core::Card, def: &CardDef, form: nav_core::Form) -> String {
        let marked = def.keywords.contains(&Keyword::Marked) && card.blind.is_none() && !card.rolls.is_empty();
        if !marked && card.blind.is_none() {
            return self.full_card_text(&card.def, def, card.upgraded, form);
        }
        let mut view = def.view(card.upgraded, form);
        if marked {
            view.effects = nav_core::craft::settle_rolls(&view.effects, &card.rolls);
        } else if let Some(by) = card.blind {
            view.effects = nav_core::craft::bump_rolls(&view.effects, by);
            // made blind, it is no longer marked
            view.keywords.retain(|k| *k != Keyword::Marked);
        }
        self.card_text(&card.def, def, &view)
    }

    /// Rules text of a card. A hand-written `card-<id>-desc` overrides the generated one.
    pub fn card_text(&self, id: &str, def: &CardDef, view: &CardView) -> String {
        self.card_body(id, def, view, |_| true)
    }

    /// The card's text in lines: what it does when played (and what its powers do), then what
    /// it does while held at the turn's end, then when thrown away ("Discard: ..."), and last the
    /// keywords `say` lets through.
    fn card_body(&self, id: &str, def: &CardDef, view: &CardView, say: impl Fn(&Keyword) -> bool) -> String {
        // an upgraded card may have words of its own (`card-<id>-up-desc`)
        let upgraded = def.up.is_some() && def.view(true, nav_core::Form::Human).effects == view.effects && def.view(false, nav_core::Form::Human).effects != view.effects;
        let up_key = format!("card-{id}-up-desc");
        let custom = if upgraded && self.has(&up_key) { up_key } else { format!("card-{id}-desc") };
        let mut lines = Vec::new();
        if self.has(&custom) {
            lines.push(self.t(&custom));
        } else {
            let mut first = Vec::new();
            let body = self.effects(&view.effects);
            if !body.is_empty() {
                first.push(body);
            }
            for p in &view.passives {
                first.push(self.tf(&format!("trigger-{:?}", p.on), &[("fx", self.clause(self.effects(&p.effects)).into())]));
            }
            if !first.is_empty() {
                lines.push(first.join(" "));
            }
            if !def.in_hand_end.is_empty() {
                lines.push(self.tf("trigger-InHand", &[("fx", self.clause(self.effects(&def.in_hand_end)).into())]));
            }
            if !view.on_discard.is_empty() {
                lines.push(self.tf("trigger-OnDiscard", &[("fx", self.clause(self.effects(&view.on_discard)).into())]));
            }
        }
        let keywords: Vec<String> = view.keywords.iter().filter(|k| **k != Keyword::Coop && say(k)).map(|k| format!("{}.", self.t(&format!("kw-{k:?}")))).collect();
        if !keywords.is_empty() {
            lines.push(keywords.join(" "));
        }
        let rune = self.t("rune-pushed-out");
        let text = once_at_the_end(lines.join("\n"), &rune);
        // the line the sentence was struck from may end in a space now
        text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
    }

    /// Keywords and statuses a card mentions, for its hover tooltips.
    pub fn card_glossary(&self, view: &CardView) -> Vec<(String, String)> {
        let mut keys: Vec<String> = view.keywords.iter().map(|k| format!("kw-{k:?}")).collect();
        if view.drowse > 0 {
            keys.push("kw-DrowseCost".into());
        }
        // what a varying amount counts is explained too: a status, a rune, the curses
        fn counts(a: &Amount, keys: &mut Vec<String>) {
            match a {
                Amount::Stat(st, ..) => keys.push(format!("status-{st:?}")),
                Amount::Runes(kind, _) => {
                    keys.push(format!("rune-{kind:?}"));
                    keys.push("kw-Rune".into());
                }
                Amount::Curses(..) => keys.push("kw-Curse".into()),
                Amount::Consumed(_) => keys.push("kw-Respond".into()),
                Amount::Discarded(_) | Amount::DiscardedThisTurn(_) => keys.push("kw-Discard".into()),
                Amount::Roll(..) => keys.push("kw-Range".into()),
                Amount::Part(st, _) => keys.push(format!("status-{st:?}")),
                Amount::AliensInHand(_) => keys.push("kw-Alien".into()),
                Amount::Sum(list) => list.iter().for_each(|a| counts(a, keys)),
                Amount::AtLeast(a, _) => counts(a, keys),
                _ => {}
            }
        }
        fn cond(c: &Cond, keys: &mut Vec<String>) {
            match c {
                Cond::Has(st, ..) => keys.push(format!("status-{st:?}")),
                Cond::Link(_) => keys.push("kw-Link".into()),
                Cond::Mask(m) => keys.push(format!("mask-{m:?}")),
                Cond::Form(_) => keys.push("kw-Shift".into()),
                Cond::Not(inner) => cond(inner, keys),
                Cond::InPlate => keys.push("status-Plate".into()),
                Cond::AlienInHand => keys.push("kw-Alien".into()),
                Cond::Flag(name, _) if name.contains("alien") => keys.push("kw-Alien".into()),
                Cond::Flag(name, _) if name.contains("forge") => keys.push("kw-Forged".into()),
                _ => {}
            }
        }
        fn walk(list: &[Effect], keys: &mut Vec<String>) {
            for e in list {
                match e {
                    Effect::Damage(a, _) | Effect::Hits(a, ..) | Effect::Block(a, _) | Effect::Heal(a, _) | Effect::Draw(a) | Effect::Energy(a, _) | Effect::Repeat(a, _) => counts(a, keys),
                    _ => {}
                }
                match e {
                    Effect::Apply(st, a, _) => {
                        keys.push(format!("status-{st:?}"));
                        counts(a, keys);
                    }
                    // a curse drawn off a foe is a Respond; one's own Valor or sleep is simply spent
                    Effect::Consume(st, t) => {
                        if *t != Tgt::Me {
                            keys.push("kw-Respond".into());
                        }
                        keys.push(format!("status-{st:?}"));
                    }
                    Effect::ConsumeCurses(_) => {
                        keys.push("kw-Respond".into());
                        keys.push("kw-Curse".into());
                    }
                    Effect::Guard(..) => keys.push("kw-Guard".into()),
                    Effect::Taunt => keys.push("kw-Taunt".into()),
                    Effect::Invoke => keys.push("kw-Invoke".into()),
                    Effect::Shift => keys.push("kw-Shift".into()),
                    Effect::Rune(kind, _) => {
                        keys.push(format!("rune-{kind:?}"));
                        keys.push("kw-Rune".into());
                    }
                    Effect::Mask(m) => keys.push(format!("mask-{m:?}")),
                    Effect::If(c, a, b) => {
                        cond(c, keys);
                        walk(a, keys);
                        walk(b, keys);
                    }
                    Effect::Repeat(_, a) | Effect::Once(_, a) | Effect::OncePerTurn(_, a) | Effect::Every(_, _, a) | Effect::EveryThisTurn(_, _, a) => walk(a, keys),
                    Effect::Echo(_) => keys.push("kw-Echo".into()),
                    Effect::Herb(herb) => {
                        keys.push(format!("herb-{herb}"));
                        keys.push("kw-Brew".into());
                    }
                    Effect::Brew => keys.push("kw-Brew".into()),
                    Effect::PikeWill => keys.push("kw-PikeWill".into()),
                    Effect::Discard(_) | Effect::DiscardHand => keys.push("kw-Discard".into()),
                    Effect::ExhaustCards(_) => keys.push("kw-Exhaust".into()),
                    Effect::Hook(name) if name == "kolyada" => keys.push("kw-Rune".into()),
                    Effect::Forge(a) | Effect::Mark(a) => {
                        keys.push("kw-Forged".into());
                        keys.push("status-Forge".into());
                        counts(a, keys);
                    }
                    Effect::Plate(_) => keys.push("status-Plate".into()),
                    Effect::Reforge(..) => keys.push("kw-Reforge".into()),
                    Effect::Upgrade(..) => keys.push("kw-UpgradeFight".into()),
                    Effect::Set(name, ..) if name == "bracers" => keys.push("kw-Bracers".into()),
                    Effect::Transmute(..) => keys.push("kw-Transmute".into()),
                    Effect::Pick { then, .. } => {
                        if *then == nav_core::PickThen::Alien {
                            keys.push("kw-Alien".into());
                        }
                        if *then == nav_core::PickThen::Transmute {
                            keys.push("kw-Transmute".into());
                        }
                    }
                    Effect::Luck { foe, friend, .. } => {
                        keys.push("kw-LuckCard".into());
                        keys.push("status-Luck".into());
                        walk(foe, keys);
                        walk(friend, keys);
                    }
                    Effect::Alien { .. } => keys.push("kw-Alien".into()),
                    Effect::Stash(_) => keys.push("kw-Stash".into()),
                    Effect::Hook(name) if name.contains("borrow") || name.contains("trick") => keys.push("kw-Borrow".into()),
                    Effect::Hook(name) if name == "steal_move" || name.contains("alien") || name == "gifts" || name == "gossip" || name == "promise" || name == "swipe_top" || name == "copy_hand" => keys.push("kw-Alien".into()),
                    Effect::Hook(name) if name == "random_herb" => keys.push("kw-Brew".into()),
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
    #[ignore]
    fn dump_card_texts() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let c = nav_core::Content::load_dir(&root.join("data")).expect("content");
        let lang = if std::env::var("DUMP_EN").is_ok() { Lang::En } else if std::env::var("DUMP_OLD").is_ok() { Lang::Old } else { Lang::Ru };
        let l = L10n::load(&crate::art::Art::new(root.clone()), lang);
        let mut ids: Vec<&String> = c.cards.keys().collect();
        ids.sort();
        let mut out = String::new();
        for id in ids {
            let def = &c.cards[id];
            for up in [false, true] {
                let t = l.full_card_text(id, def, up, nav_core::Form::Human);
                out.push_str(&format!("{id}{} | {}\n", if up { "+" } else { "" }, t.replace('\n', " / ")));
            }
        }
        std::fs::write(std::env::var("DUMP_TO").unwrap(), out).unwrap();
    }

    #[test]
    fn a_card_the_pike_plays_from_explains_the_pike() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let c = nav_core::Content::load_dir(&root.join("data")).expect("content");
        for lang in [Lang::Old, Lang::Ru, Lang::En] {
            let l = L10n::load(&crate::art::Art::new(root.clone()), lang);
            let name = l.t("kw-PikeWill");
            for def in c.cards.values() {
                let view = def.view(false, nav_core::Form::Human);
                let pike = format!("{:?}{:?}", view.effects, view.passives).contains("PikeWill");
                if pike {
                    assert!(l.card_glossary(&view).iter().any(|(n, _)| *n == name), "{lang:?} {}", def.id);
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
