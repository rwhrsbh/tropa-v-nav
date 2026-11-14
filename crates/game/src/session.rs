//! The fight in progress as the client sees it: the authoritative state, a
//! display copy that lags behind while events are being animated, and the
//! queue between them.

use bevy::prelude::*;
use nav_core::Event;
use nav_core::*;
use std::collections::VecDeque;

#[derive(Resource)]
pub struct Db(pub Content);

#[derive(Resource)]
pub struct Session {
    /// The real state. Hand, energy and legality checks read this.
    pub truth: Combat,
    /// What the unit widgets show; catches up with `truth` event by event.
    pub shown: Combat,
    pub me: u8,
    pub queue: VecDeque<Event>,
    pub wait: f32,
    pub error: Option<(String, f32)>,
}

/// Combat actions the local player wants to take this frame.
#[derive(Resource, Default)]
pub struct Outbox(pub Vec<CombatAction>);

/// Everything else the local player wants: votes, card picks, the fire.
#[derive(Resource, Default)]
pub struct RunOutbox(pub Vec<RunAction>);

/// Events released to the screen this frame, for floaters and animations.
#[derive(Resource, Default)]
pub struct Cues(pub Vec<Event>);

/// Animation speed multiplier (1x / 1.5x / 2x). Never touches game state.
#[derive(Resource)]
pub struct AnimSpeed(pub f32);

impl Session {
    /// A fight as it stands in the run, with only the cosmetic part of `opening` left to play.
    pub fn from_state(state: Combat, me: u8, opening: impl IntoIterator<Item = Event>) -> Session {
        Session {
            shown: state.clone(),
            truth: state,
            me,
            // the opening already happened in `shown`; replay only what is pure show
            queue: opening.into_iter().filter(|e| matches!(e, Event::RoundStart(_))).collect(),
            wait: 0.0,
            error: None,
        }
    }

    pub fn my_hero(&self) -> &Hero {
        &self.truth.heroes[self.me as usize]
    }

    pub fn idle(&self) -> bool {
        self.queue.is_empty()
    }
}

/// How long the screen lingers on an event before showing the next.
fn delay(e: &Event) -> f32 {
    match e {
        Event::CardPlayed { .. } => 0.28,
        // the turned-up card is held up to be read before it does anything
        Event::PikeTurned { .. } => 0.7,
        Event::Damage { attack: true, .. } => 0.32,
        Event::Damage { .. } => 0.22,
        Event::EnemyMove { .. } => 0.35,
        Event::EnemySkipped { .. } => 0.4,
        Event::EnemyPhase => 0.35,
        Event::Block { .. } | Event::Heal { .. } => 0.14,
        Event::Downed { .. } | Event::Raised { .. } => 0.6,
        Event::EnemyDied { .. } => 0.25,
        Event::RuneFired { .. } | Event::Shifted { .. } => 0.25,
        Event::RoundStart(_) => 0.3,
        Event::Ended { .. } => 0.5,
        _ => 0.0,
    }
}

/// Folds one event into the display copy.
fn show(shown: &mut Combat, truth: &Combat, e: &Event) {
    let set = |shown: &mut Combat, u: UnitRef, hp: Option<i32>, block: Option<i32>| match u {
        UnitRef::Hero(i) => {
            if let Some(h) = shown.heroes.get_mut(i as usize) {
                hp.map(|v| h.hp = v);
                block.map(|v| h.block = v);
            }
        }
        UnitRef::Enemy(i) => {
            if let Some(en) = shown.enemies.iter_mut().find(|x| x.uid == i) {
                hp.map(|v| en.hp = v);
                block.map(|v| en.block = v);
            }
        }
    };
    match e {
        Event::Damage { dst, hp, block, .. } => set(shown, *dst, Some(*hp), Some(*block)),
        Event::Block { unit, total, .. } => set(shown, *unit, None, Some(*total)),
        Event::Heal { unit, hp, .. } => {
            set(shown, *unit, Some(*hp), None);
            // bones that got up
            if let UnitRef::Enemy(uid) = unit {
                if let Some(en) = shown.enemies.iter_mut().find(|x| x.uid == *uid) {
                    en.flags.remove(nav_core::state::BONES);
                }
            }
        }
        Event::Collapsed { enemy } => {
            if let Some(en) = shown.enemies.iter_mut().find(|x| x.uid == *enemy) {
                en.hp = 0;
                en.block = 0;
                en.statuses.clear();
                en.flags.insert(nav_core::state::BONES.into(), 1);
            }
        }
        Event::Status { unit, status, total, .. } => {
            let map = match unit {
                UnitRef::Hero(i) => shown.heroes.get_mut(*i as usize).map(|h| &mut h.statuses),
                UnitRef::Enemy(i) => shown.enemies.iter_mut().find(|x| x.uid == *i).map(|x| &mut x.statuses),
            };
            if let Some(map) = map {
                if *total == 0 {
                    map.remove(status);
                } else {
                    map.insert(*status, *total);
                }
            }
        }
        Event::Maimed { player, limbs, arm } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.lost_limbs = *limbs;
                h.lost_arms |= *arm;
            }
        }
        Event::Beheaded { player } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.beheaded = true;
            }
        }
        Event::Downed { player } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.downed = true;
                h.hp = 0;
                h.block = 0;
            }
        }
        Event::Raised { player, hp, .. } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.downed = false;
                h.hp = *hp;
            }
        }
        Event::EnemyDied { enemy } | Event::EnemyFled { enemy } => {
            if let Some(en) = shown.enemies.iter_mut().find(|x| x.uid == *enemy) {
                en.alive = false;
                en.intent = None;
            }
        }
        Event::Summoned { enemy, .. } => {
            // one that fell and grows back (a head of the Gorynych) stands up where it lay
            if let (Some(en), Some(real)) = (shown.enemies.iter_mut().find(|x| x.uid == *enemy && !x.alive), truth.enemy(*enemy)) {
                en.alive = true;
                en.hp = real.hp;
                en.intent = None;
            }
            if shown.enemy(*enemy).is_none() {
                if let Some(en) = truth.enemy(*enemy) {
                    let mut en = en.clone();
                    en.hp = en.max_hp;
                    en.alive = true;
                    en.intent = None;
                    shown.enemies.push(en);
                }
            }
        }
        Event::Intent { enemy, mv, target } => {
            if let Some(en) = shown.enemies.iter_mut().find(|x| x.uid == *enemy) {
                en.intent = Some(Intent { mv: *mv, target: *target });
            }
        }
        Event::Taunt { player } => {
            for en in shown.enemies.iter_mut() {
                if let Some(i) = &mut en.intent {
                    if i.target.is_some() {
                        i.target = Some(*player);
                    }
                }
            }
        }
        Event::EnemyMove { enemy, .. } | Event::EnemySkipped { enemy, .. } => {
            if let Some(en) = shown.enemies.iter_mut().find(|x| x.uid == *enemy) {
                en.intent = None;
            }
        }
        Event::RoundStart(n) => shown.round = *n,
        Event::RuneCarved { player, rune } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.runes.push(*rune);
            }
        }
        Event::RuneDischarged { player, rune } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                if let Some(i) = h.runes.iter().position(|r| r == rune) {
                    h.runes.remove(i);
                }
            }
        }
        Event::Shifted { player, form } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.form = *form;
            }
        }
        Event::MaskChanged { player, mask } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.mask = Some(*mask);
            }
        }
        _ => {}
    }
}

/// Hands the local player's wishes to the protocol node; the host decides,
/// and the answer comes back as a note (see `net::notes`).
pub fn submit(mut outbox: ResMut<Outbox>, mut run_outbox: ResMut<RunOutbox>, db: Res<Db>, mut net: Option<ResMut<crate::net::Net>>) {
    let Some(net) = net.as_mut() else {
        outbox.0.clear();
        run_outbox.0.clear();
        return;
    };
    for action in outbox.0.drain(..) {
        net.node.submit(&db.0, RunAction::Combat(action));
    }
    for action in run_outbox.0.drain(..) {
        net.node.submit(&db.0, action);
    }
}

pub fn playback(mut session: ResMut<Session>, mut cues: ResMut<Cues>, time: Res<Time>, speed: Res<AnimSpeed>) {
    cues.0.clear();
    if let Some((_, ttl)) = &mut session.error {
        *ttl -= time.delta_secs();
        if *ttl <= 0.0 {
            session.error = None;
        }
    }
    if session.queue.is_empty() {
        return;
    }
    session.wait -= time.delta_secs() * speed.0;
    while session.wait <= 0.0 {
        let Some(e) = session.queue.pop_front() else { break };
        let s = &mut *session;
        show(&mut s.shown, &s.truth, &e);
        s.wait += delay(&e);
        cues.0.push(e);
    }
    if session.queue.is_empty() {
        // whatever the events didn't carry is corrected here
        session.shown = session.truth.clone();
        session.wait = 0.0;
    }
}
