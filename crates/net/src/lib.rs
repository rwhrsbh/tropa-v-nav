//! Co-op protocol, independent of any transport.
//!
//! The host is the single source of truth. A client sends the action it wants;
//! the host validates it against its own state, applies it, and broadcasts it
//! with a running number. Every machine applies the same numbered actions to
//! the same deterministic rules, so they all hold the same run. Whenever the
//! run moves on (a new round, a new stage, a new floor) the host sends a hash;
//! a client that disagrees asks for a full snapshot.
//!
//! A [`Node`] is one machine's view. It never touches the network: it takes
//! incoming messages through [`Node::handle`] and leaves outgoing ones in
//! [`Node::take_out`]. Steam, or the in-memory [`Hub`], moves them.

use nav_core::*;
use serde::{Deserialize, Serialize};

pub type Peer = u64;
pub const MAX_PLAYERS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub peer: Peer,
    pub name: String,
    pub hero: Option<Id>,
    /// The player's level with that hero; it decides which of their cards may turn up.
    #[serde(default)]
    pub level: u32,
    /// Which starting relic they chose: 0 the hero's own, 1 or 2 the shrine's.
    #[serde(default)]
    pub starter: u8,
    pub ready: bool,
    pub connected: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lobby {
    /// Slot order: index here is the hero slot in combat.
    pub players: Vec<LobbyPlayer>,
    pub host: Peer,
    /// Circle of Nav, 0..=10.
    pub circle: u8,
    /// Optional turn timer in seconds (off by default).
    pub turn_timer: Option<u32>,
    pub started: bool,
}

impl Lobby {
    pub fn slot_of(&self, peer: Peer) -> Option<u8> {
        self.players.iter().position(|p| p.peer == peer).map(|i| i as u8)
    }
    /// Everyone picked a distinct hero and pressed ready.
    pub fn can_start(&self) -> bool {
        !self.players.is_empty() && self.players.iter().all(|p| p.ready && p.hero.is_some())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartInfo {
    pub seed: u64,
    /// Heroes in slot order.
    pub heroes: Vec<Id>,
    pub circle: u8,
    pub unlocks: Unlocks,
}

/// What build of the game this is (`set_build`): a party is played only by one build, since a
/// phone and a desktop on different versions would deal the same run out differently.
static BUILD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Names the build this game is, said in every hello.
pub fn set_build(fingerprint: u64) {
    BUILD.store(fingerprint, std::sync::atomic::Ordering::Relaxed);
}

fn build() -> u64 {
    BUILD.load(std::sync::atomic::Ordering::Relaxed)
}

/// Pencil changes are cosmetic, ordered by the reliable transport, and owned by their sender.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InkChange {
    Begin([f32; 2]),
    Point([f32; 2]),
    Rub { from: [f32; 2], to: [f32; 2] },
    Clear,
    Snapshot(Vec<Vec<[f32; 2]>>),
}

impl InkChange {
    fn valid(&self) -> bool {
        let point = |p: &[f32; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= 100_000.0);
        match self {
            Self::Begin(p) | Self::Point(p) => point(p),
            Self::Rub { from, to } => point(from) && point(to),
            Self::Clear => true,
            Self::Snapshot(strokes) => strokes.iter().flatten().all(point),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Msg {
    /// Client -> host on joining or rejoining, with the client's build.
    Hello { name: String, build: u64 },
    /// Host -> a client whose build is not the host's: it is not let in.
    OtherBuild,
    /// Host -> everyone whenever the lobby changes.
    Lobby(Lobby),
    /// A hero and the player's level with them.
    PickHero(Option<Id>, u32),
    PickStarter(u8),
    SetReady(bool),
    /// Host -> everyone: the run begins from this exact setup.
    Start(StartInfo),
    /// Client -> host: "I want to do this". `seq` lets the client match a refusal.
    Want { seq: u32, action: RunAction },
    /// Host -> everyone: action number `n` in the shared log.
    Did { n: u32, player: u8, action: RunAction },
    Refused { seq: u32, error: RunError },
    /// Host -> everyone whenever the run moves on.
    Hash { n: u32, hash: u64 },
    /// Client -> host: my hash differs, or I just rejoined.
    Resync,
    /// Host -> one client: the whole state after `n` actions.
    Snapshot { n: u32, lobby: Lobby, state: Box<Run> },
    /// Quick signal without chat, pinned to a unit.
    Ping { kind: u8, at: UnitRef },
    /// Everyone: my vote on the story page `page` (its text key): read on, or skip the scene.
    StoryVote { page: String, skip: bool },
    /// Everyone: my hand over the book while `page` is read together: where it is in the scene,
    /// and which point of the leaf it holds, if it holds the leaf.
    BookHand { page: String, at: [f32; 3], hold: Option<u16> },
    /// Everyone: the card I hold, where it is on the stage and where I aim it; `None`, let go.
    CardHeld { card: Option<(String, bool)>, at: [f32; 2], aim: Option<[f32; 2]> },
    /// Everyone: the leaf I hold on `page` tore along this edge of its grid.
    BookTear { page: String, edge: u16 },
    /// Everyone: I have turned the leaf of `page` over; read on.
    BookTurn { page: String },
    /// Everyone: I carry torn paper across the book of `page`: `piece` 0 is the leaf in play,
    /// another a scrap (by the order scraps were set aside); held by `point`, or let go (`None`).
    BookCarry { page: String, piece: u32, point: Option<u16>, at: [f32; 3] },
    /// Everyone: where my cursor is over the path map, in stage pixels.
    MapPointer { at: [f32; 2] },
    MapInk { seed: u64, act: u8, change: InkChange },
    MapInkRequest { seed: u64, act: u8 },
    /// Everyone: hurry up, I have long since chosen this node of the map.
    Nudge { node: u8 },
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        ron::to_string(self).expect("messages always serialise").into_bytes()
    }
    pub fn decode(bytes: &[u8]) -> Option<Msg> {
        ron::from_str(std::str::from_utf8(bytes).ok()?).ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum To {
    One(Peer),
    /// Everyone in the lobby except the sender.
    All,
}

/// Things the UI should react to.
#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    LobbyChanged,
    Started,
    /// An action was applied; these are its rule events.
    Applied { player: u8, events: Vec<RunEvent> },
    Refused(RunError),
    /// State was replaced wholesale; redraw without animation.
    Resynced,
    Ping { from: u8, kind: u8, at: UnitRef },
    /// A player voted on a story page, this player included.
    StoryVote { from: u8, page: String, skip: bool },
    /// Another player's hand over the book, the leaf they tore, the leaf they turned.
    BookHand { from: u8, page: String, at: [f32; 3], hold: Option<u16> },
    /// The card another player holds; `None`, let go.
    CardHeld { from: u8, card: Option<(String, bool)>, at: [f32; 2], aim: Option<[f32; 2]> },
    BookTear { from: u8, page: String, edge: u16 },
    BookTurn { from: u8, page: String },
    BookCarry { from: u8, page: String, piece: u32, point: Option<u16>, at: [f32; 3] },
    /// Another player's cursor over the map, and their nudge at the node they chose.
    MapPointer { from: u8, at: [f32; 2] },
    MapInk { from: u8, seed: u64, act: u8, change: InkChange },
    MapInkRequest { from: Peer, seed: u64, act: u8 },
    Nudge { from: u8, node: u8 },
    HostChanged,
    /// The host plays another build of the game and did not let us in.
    OtherBuild,
}

pub struct Node {
    pub me: Peer,
    /// Host: relics of the tale's book whose secrets the host has ever found; they join the
    /// run's pools like the village's unlocks.
    pub secrets: Vec<Id>,
    pub name: String,
    pub lobby: Lobby,
    pub state: Option<Run>,
    /// Actions applied so far.
    pub n: u32,
    seq: u32,
    /// Wants sent to the host and not yet answered, resent if the host changes.
    pending: Vec<(u32, RunAction)>,
    last_mark: u64,
    out: Vec<(To, Msg)>,
    notes: Vec<Note>,
    /// How many messages the host has been heard from: a client that stops hearing it has lost
    /// the connection, whatever the transport says.
    pub heard: u64,
    /// Host: how long each seat has been disconnected, in seconds.
    pub disconnect_timer: [f32; MAX_PLAYERS],
    /// Grace period before autopilot acts for a disconnected player.
    pub reconnect_grace: f32,
}

pub const RECONNECT_GRACE_PERIOD: f32 = 120.0;

impl Node {
    /// Opens a lobby as its host.
    pub fn host(me: Peer, name: &str) -> Node {
        let mut node = Node::blank(me, name);
        node.lobby.host = me;
        node.lobby.players.push(LobbyPlayer { peer: me, name: name.into(), hero: None, level: 1, starter: 0, ready: false, connected: true });
        node
    }

    /// Reopens a saved solo run as its host.
    pub fn resume(me: Peer, name: &str, hero: Id, run: Run) -> Node {
        let mut node = Node::host(me, name);
        node.lobby.players[0].hero = Some(hero);
        node.lobby.players[0].ready = true;
        node.lobby.circle = run.circle;
        node.lobby.started = true;
        node.last_mark = mark(&run);
        node.state = Some(run);
        node.notes.push(Note::Resynced);
        node
    }

    /// Reopens a saved party run as its host. Every other seat waits, played by the autopilot, for
    /// its player to come back; one who comes back on another address takes the first empty seat.
    pub fn resume_party(me: Peer, name: &str, mut lobby: Lobby, n: u32, run: Run) -> Node {
        let mut node = Node::blank(me, name);
        let old = lobby.host;
        for p in &mut lobby.players {
            p.connected = false;
        }
        let seat = lobby.players.iter().position(|p| p.peer == old).unwrap_or(0);
        if let Some(p) = lobby.players.get_mut(seat) {
            p.peer = me;
            p.name = name.into();
            p.connected = true;
        }
        lobby.host = me;
        lobby.started = true;
        node.lobby = lobby;
        node.n = n;
        node.last_mark = mark(&run);
        node.state = Some(run);
        node.notes.push(Note::Resynced);
        node
    }

    /// Says hello to the host again after the connection was lost: the host gives the seat back
    /// and sends the whole run as it now stands.
    pub fn rejoin(&mut self) {
        if !self.is_host() {
            self.out.push((To::One(self.lobby.host), Msg::Hello { name: self.name.clone(), build: build() }));
        }
    }

    /// Host: tells everyone where the run stands, so a client hears it is still connected and one
    /// that missed actions asks for the whole run.
    pub fn heartbeat(&mut self) {
        if !self.is_host() {
            return;
        }
        if let Some(state) = &self.state {
            self.out.push((To::All, Msg::Hash { n: self.n, hash: state.hash64() }));
        }
    }

    /// Advances timers for disconnected players.
    pub fn tick(&mut self, dt: f32) {
        if self.is_host() {
            let players = &self.lobby.players;
            let timers = &mut self.disconnect_timer;
            for (i, p) in players.iter().enumerate() {
                if !p.connected {
                    timers[i] += dt;
                } else {
                    timers[i] = 0.0;
                }
            }
        }
    }

    /// Joins the lobby hosted by `host`.
    pub fn join(me: Peer, name: &str, host: Peer) -> Node {
        let mut node = Node::blank(me, name);
        node.lobby.host = host;
        node.out.push((To::One(host), Msg::Hello { name: name.into(), build: build() }));
        node
    }

    fn blank(me: Peer, name: &str) -> Node {
        Node {
            me,
            secrets: Vec::new(),
            name: name.into(),
            lobby: Lobby::default(),
            state: None,
            n: 0,
            seq: 0,
            pending: Vec::new(),
            last_mark: 0,
            out: Vec::new(),
            notes: Vec::new(),
            heard: 0,
            disconnect_timer: [0.0; MAX_PLAYERS],
            reconnect_grace: RECONNECT_GRACE_PERIOD,
        }
    }

    pub fn is_host(&self) -> bool {
        self.lobby.host == self.me
    }
    pub fn my_slot(&self) -> Option<u8> {
        self.lobby.slot_of(self.me)
    }
    pub fn take_out(&mut self) -> Vec<(To, Msg)> {
        std::mem::take(&mut self.out)
    }
    pub fn take_notes(&mut self) -> Vec<Note> {
        std::mem::take(&mut self.notes)
    }

    fn broadcast_lobby(&mut self) {
        self.out.push((To::All, Msg::Lobby(self.lobby.clone())));
        self.notes.push(Note::LobbyChanged);
    }

    // ---- lobby, local side ----

    pub fn pick_hero(&mut self, hero: Option<Id>, level: u32) {
        if self.is_host() {
            self.host_pick(self.me, hero, level);
        } else {
            self.out.push((To::One(self.lobby.host), Msg::PickHero(hero, level)));
        }
    }

    pub fn pick_starter(&mut self, starter: u8) {
        if self.is_host() {
            self.host_starter(self.me, starter);
        } else {
            self.out.push((To::One(self.lobby.host), Msg::PickStarter(starter)));
        }
    }

    pub fn set_ready(&mut self, ready: bool) {
        if self.is_host() {
            self.host_ready(self.me, ready);
        } else {
            self.out.push((To::One(self.lobby.host), Msg::SetReady(ready)));
        }
    }

    fn host_pick(&mut self, peer: Peer, hero: Option<Id>, level: u32) {
        if self.lobby.started {
            return;
        }
        // heroes are unique in a party: the colour is the player's identity
        let taken = hero.as_ref().is_some_and(|h| self.lobby.players.iter().any(|p| p.peer != peer && p.hero.as_ref() == Some(h)));
        if taken {
            return;
        }
        if let Some(p) = self.lobby.players.iter_mut().find(|p| p.peer == peer) {
            p.hero = hero;
            p.level = level.clamp(1, 10);
            p.ready = false;
            self.broadcast_lobby();
        }
    }

    fn host_starter(&mut self, peer: Peer, starter: u8) {
        if self.lobby.started {
            return;
        }
        if let Some(p) = self.lobby.players.iter_mut().find(|p| p.peer == peer) {
            p.starter = starter.min(2);
            self.broadcast_lobby();
        }
    }

    fn host_ready(&mut self, peer: Peer, ready: bool) {
        if self.lobby.started {
            return;
        }
        if let Some(p) = self.lobby.players.iter_mut().find(|p| p.peer == peer) {
            p.ready = ready && p.hero.is_some();
            self.broadcast_lobby();
        }
    }

    /// Host only: choose the Circle of Nav for the run about to start.
    pub fn set_circle(&mut self, circle: u8) {
        if self.is_host() && !self.lobby.started && self.lobby.circle != circle.min(10) {
            self.lobby.circle = circle.min(10);
            self.broadcast_lobby();
        }
    }

    /// Host only: the optional turn timer in seconds (`None` is off, the default).
    pub fn set_turn_timer(&mut self, seconds: Option<u32>) {
        if self.is_host() && self.lobby.turn_timer != seconds {
            self.lobby.turn_timer = seconds;
            self.broadcast_lobby();
        }
    }

    /// Host only: begin the run. Returns false if the lobby isn't ready.
    /// `village` is the host's: its smithy, hut and tavern decide the shared pools.
    pub fn start(&mut self, c: &Content, seed: u64, village: (u8, u8, u8)) -> bool {
        if !self.is_host() || !self.lobby.can_start() || self.lobby.started {
            return false;
        }
        let heroes: Option<Vec<Id>> = self.lobby.players.iter().map(|p| p.hero.clone().filter(|h| c.heroes.contains_key(h))).collect();
        let Some(heroes) = heroes else { return false };
        self.lobby.started = true;
        let unlocks = Unlocks { relic_tier: village.0, potion_tier: village.1, omen_tier: village.2, hero_levels: self.lobby.players.iter().map(|p| p.level.max(1)).collect(), starters: self.lobby.players.iter().map(|p| p.starter).collect(), secrets: self.secrets.clone() };
        let info = StartInfo { seed, heroes, circle: self.lobby.circle, unlocks };
        self.out.push((To::All, Msg::Lobby(self.lobby.clone())));
        self.out.push((To::All, Msg::Start(info.clone())));
        self.begin(c, &info);
        true
    }

    fn begin(&mut self, c: &Content, info: &StartInfo) {
        let Some(state) = Run::new_with(c, &info.heroes, info.seed, info.circle, info.unlocks.clone()) else { return };
        self.last_mark = mark(&state);
        self.state = Some(state);
        self.n = 0;
        self.pending.clear();
        self.lobby.started = true;
        self.notes.push(Note::Started);
    }

    // ---- actions ----

    /// The local player wants to act.
    pub fn submit(&mut self, c: &Content, action: RunAction) {
        let Some(slot) = self.my_slot() else { return };
        if self.is_host() {
            if let Err(e) = self.host_apply(c, slot, action) {
                self.notes.push(Note::Refused(e));
            }
        } else {
            self.seq += 1;
            self.pending.push((self.seq, action.clone()));
            self.out.push((To::One(self.lobby.host), Msg::Want { seq: self.seq, action }));
        }
    }

    /// Host: validate, apply, broadcast.
    fn host_apply(&mut self, c: &Content, slot: u8, action: RunAction) -> Result<(), RunError> {
        let state = self.state.as_mut().ok_or(RunError::WrongStage)?;
        let events = state.apply(c, slot, &action)?;
        self.n += 1;
        self.out.push((To::All, Msg::Did { n: self.n, player: slot, action }));
        self.notes.push(Note::Applied { player: slot, events });
        let state = self.state.as_ref().unwrap();
        if mark(state) != self.last_mark {
            self.last_mark = mark(state);
            self.out.push((To::All, Msg::Hash { n: self.n, hash: state.hash64() }));
        }
        Ok(())
    }

    /// Host: play one autopilot move for a disconnected player, if any is due.
    /// Call it on a timer; returns whether a move was made.
    pub fn autopilot(&mut self, c: &Content) -> bool {
        if !self.is_host() {
            return false;
        }
        let Some(state) = &self.state else { return false };
        let due = self.lobby.players.iter().enumerate().find_map(|(i, p)| {
            if p.connected || self.disconnect_timer[i] < self.reconnect_grace {
                None
            } else {
                run_bot(c, state, i as u8).map(|a| (i as u8, a))
            }
        });
        let Some((slot, action)) = due else { return false };
        self.host_apply(c, slot, action).is_ok()
    }

    pub fn ping(&mut self, kind: u8, at: UnitRef) {
        if let Some(from) = self.my_slot() {
            self.out.push((To::All, Msg::Ping { kind, at }));
            self.notes.push(Note::Ping { from, kind, at });
        }
    }

    /// Votes to turn the story page everyone is reading, or to skip the rest of the scene.
    pub fn story_vote(&mut self, page: &str, skip: bool) {
        if let Some(from) = self.my_slot() {
            self.out.push((To::All, Msg::StoryVote { page: page.into(), skip }));
            self.notes.push(Note::StoryVote { from, page: page.into(), skip });
        }
    }

    /// Shows the others the card I hold.
    pub fn card_held(&mut self, card: Option<(String, bool)>, at: [f32; 2], aim: Option<[f32; 2]>) {
        self.out.push((To::All, Msg::CardHeld { card, at, aim }));
    }

    /// Shows the others my hand over the book of the page read together.
    pub fn book_hand(&mut self, page: &str, at: [f32; 3], hold: Option<u16>) {
        self.out.push((To::All, Msg::BookHand { page: page.into(), at, hold }));
    }

    /// Tells the others the leaf I hold tore along `edge`.
    pub fn book_tear(&mut self, page: &str, edge: u16) {
        self.out.push((To::All, Msg::BookTear { page: page.into(), edge }));
    }

    /// Tells the others I turned the leaf of `page`: everyone reads on.
    pub fn book_turn(&mut self, page: &str) {
        self.out.push((To::All, Msg::BookTurn { page: page.into() }));
    }

    /// Shows the others the torn paper I carry over the book, or that I let it go.
    pub fn book_carry(&mut self, page: &str, piece: u32, point: Option<u16>, at: [f32; 3]) {
        self.out.push((To::All, Msg::BookCarry { page: page.into(), piece, point, at }));
    }

    /// Shows the others where my cursor is over the map.
    pub fn map_pointer(&mut self, at: [f32; 2]) {
        self.out.push((To::All, Msg::MapPointer { at }));
    }

    pub fn map_ink(&mut self, to: To, change: InkChange) {
        if let Some(run) = &self.state {
            if change.valid() {
                self.out.push((to, Msg::MapInk { seed: run.seed, act: run.act, change }));
            }
        }
    }

    pub fn request_map_ink(&mut self) {
        if let Some(run) = &self.state {
            self.out.push((To::All, Msg::MapInkRequest { seed: run.seed, act: run.act }));
        }
    }

    /// Hurries the others along, from the map node I chose.
    pub fn nudge(&mut self, node: u8) {
        self.out.push((To::All, Msg::Nudge { node }));
    }

    // ---- membership ----

    /// The transport saw this peer leave the lobby.
    pub fn peer_left(&mut self, peer: Peer) {
        if self.lobby.started {
            if let Some((i, p)) = self.lobby.players.iter_mut().enumerate().find(|(_, p)| p.peer == peer) {
                p.connected = false;
                self.disconnect_timer[i] = 0.0;
            }
        } else {
            self.lobby.players.retain(|p| p.peer != peer);
        }
        if self.is_host() {
            self.broadcast_lobby();
        } else {
            self.notes.push(Note::LobbyChanged);
        }
    }

    /// The transport says the lobby has a new owner (the old host left).
    pub fn host_changed(&mut self, c: &Content, new_host: Peer) {
        if self.lobby.host == new_host {
            return;
        }
        self.lobby.host = new_host;
        self.notes.push(Note::HostChanged);
        if self.is_host() {
            // everyone holds the full state, so the new host simply carries on
            self.broadcast_lobby();
            // what this machine asked the old host for, it now decides itself
            if let Some(slot) = self.my_slot() {
                for (_, action) in std::mem::take(&mut self.pending) {
                    let _ = self.host_apply(c, slot, action);
                }
            }
            if let Some(state) = &self.state {
                self.out.push((To::All, Msg::Hash { n: self.n, hash: state.hash64() }));
            }
        } else {
            // whatever the old host never answered goes to the new one
            for (seq, action) in self.pending.clone() {
                self.out.push((To::One(new_host), Msg::Want { seq, action }));
            }
        }
    }

    // ---- incoming ----

    pub fn handle(&mut self, c: &Content, from: Peer, msg: Msg) {
        let host = self.is_host();
        if from == self.lobby.host {
            self.heard += 1;
        }
        if host && !matches!(msg, Msg::OtherBuild) {
            if let Some((i, p)) = self.lobby.players.iter_mut().enumerate().find(|(_, p)| p.peer == from) {
                if !p.connected {
                    p.connected = true;
                    self.disconnect_timer[i] = 0.0;
                    self.broadcast_lobby();
                }
            }
        }
        match msg {
            Msg::Hello { build: theirs, .. } if host && theirs != build() => {
                self.out.push((To::One(from), Msg::OtherBuild));
            }
            Msg::OtherBuild if !host => self.notes.push(Note::OtherBuild),
            Msg::Hello { name, .. } if host => {
                if let Some((i, p)) = self.lobby.players.iter_mut().enumerate().find(|(_, p)| p.peer == from) {
                    // a returning player takes the wheel back from the autopilot
                    p.connected = true;
                    self.disconnect_timer[i] = 0.0;
                    p.name = name;
                } else if let Some((i, p)) = self.lobby.players.iter_mut().enumerate().find(|(_, p)| self.lobby.started && !p.connected) {
                    // a saved party picked up again, or a player back on another address: the
                    // first seat with no one at it is theirs
                    p.peer = from;
                    p.connected = true;
                    self.disconnect_timer[i] = 0.0;
                    p.name = name;
                } else if !self.lobby.started && self.lobby.players.len() < MAX_PLAYERS {
                    self.lobby.players.push(LobbyPlayer { peer: from, name, hero: None, level: 1, starter: 0, ready: false, connected: true });
                } else {
                    return;
                }
                self.broadcast_lobby();
                if let Some(state) = &self.state {
                    self.out.push((To::One(from), Msg::Snapshot { n: self.n, lobby: self.lobby.clone(), state: Box::new(state.clone()) }));
                }
            }
            Msg::PickHero(hero, level) if host => self.host_pick(from, hero, level),
            Msg::PickStarter(starter) if host => self.host_starter(from, starter),
            Msg::SetReady(ready) if host => self.host_ready(from, ready),
            Msg::Want { seq, action } if host => {
                let Some(slot) = self.lobby.slot_of(from) else { return };
                if let Err(error) = self.host_apply(c, slot, action) {
                    self.out.push((To::One(from), Msg::Refused { seq, error }));
                }
            }
            Msg::Resync if host => {
                if let Some(state) = &self.state {
                    self.out.push((To::One(from), Msg::Snapshot { n: self.n, lobby: self.lobby.clone(), state: Box::new(state.clone()) }));
                }
            }
            Msg::Ping { kind, at } => {
                if let Some(slot) = self.lobby.slot_of(from) {
                    self.notes.push(Note::Ping { from: slot, kind, at });
                }
            }
            Msg::StoryVote { page, skip } => {
                if let Some(slot) = self.lobby.slot_of(from) {
                    self.notes.push(Note::StoryVote { from: slot, page, skip });
                }
            }
            Msg::CardHeld { card, at, aim } => {
                if let Some(slot) = self.lobby.slot_of(from) {
                    self.notes.push(Note::CardHeld { from: slot, card, at, aim });
                }
            }
            Msg::BookHand { page, at, hold } => {
                if let Some(slot) = self.lobby.slot_of(from) {
                    self.notes.push(Note::BookHand { from: slot, page, at, hold });
                }
            }
            Msg::BookTear { page, edge } => {
                if let Some(slot) = self.lobby.slot_of(from) {
                    self.notes.push(Note::BookTear { from: slot, page, edge });
                }
            }
            Msg::BookTurn { page } => {
                if let Some(slot) = self.lobby.slot_of(from) {
                    self.notes.push(Note::BookTurn { from: slot, page });
                }
            }
            Msg::BookCarry { page, piece, point, at } => {
                if let Some(slot) = self.lobby.slot_of(from) {
                    self.notes.push(Note::BookCarry { from: slot, page, piece, point, at });
                }
            }
            Msg::MapPointer { at } => {
                if let Some(slot) = self.lobby.slot_of(from) {
                    self.notes.push(Note::MapPointer { from: slot, at });
                }
            }
            Msg::MapInk { seed, act, change } => {
                if from != self.me && change.valid() && self.state.as_ref().is_some_and(|r| r.seed == seed && r.act == act) {
                    if let Some(slot) = self.lobby.slot_of(from) {
                        self.notes.push(Note::MapInk { from: slot, seed, act, change });
                    }
                }
            }
            Msg::MapInkRequest { seed, act } => {
                if from != self.me && self.lobby.slot_of(from).is_some() && self.state.as_ref().is_some_and(|r| r.seed == seed && r.act == act) {
                    self.notes.push(Note::MapInkRequest { from, seed, act });
                }
            }
            Msg::Nudge { node } => {
                if let Some(slot) = self.lobby.slot_of(from) {
                    self.notes.push(Note::Nudge { from: slot, node });
                }
            }

            // everything below is only believed when it comes from the host
            _ if from != self.lobby.host => {}
            Msg::Lobby(lobby) => {
                self.lobby = lobby;
                self.notes.push(Note::LobbyChanged);
            }
            Msg::Start(info) => self.begin(c, &info),
            Msg::Did { n, player, action } => {
                if n != self.n + 1 {
                    // a gap or a repeat: our log no longer lines up with the host's
                    if n > self.n {
                        self.out.push((To::One(self.lobby.host), Msg::Resync));
                    }
                    return;
                }
                let Some(state) = self.state.as_mut() else { return };
                match state.apply(c, player, &action) {
                    Ok(events) => {
                        self.n = n;
                        if Some(player) == self.lobby.slot_of(self.me) {
                            // confirmed in order, so the oldest pending want is this one
                            if let Some(i) = self.pending.iter().position(|(_, a)| *a == action) {
                                self.pending.remove(i);
                            }
                        }
                        self.notes.push(Note::Applied { player, events });
                    }
                    // the host accepted what our state refuses: we have drifted
                    Err(_) => self.out.push((To::One(self.lobby.host), Msg::Resync)),
                }
            }
            Msg::Refused { seq, error } => {
                self.pending.retain(|(s, _)| *s != seq);
                self.notes.push(Note::Refused(error));
            }
            Msg::Hash { n, hash } => {
                // a different run, or actions that never arrived: ask for the whole of it
                let off = n == self.n && self.state.as_ref().is_some_and(|s| s.hash64() != hash);
                if off || n > self.n {
                    self.out.push((To::One(self.lobby.host), Msg::Resync));
                }
            }
            Msg::Snapshot { n, lobby, state } => {
                self.lobby = lobby;
                self.state = Some(*state);
                self.n = n;
                self.pending.clear();
                self.notes.push(Note::Resynced);
            }
            _ => {}
        }
    }
}

/// A cheap fingerprint of "where the run is": it changes on a new round, stage or floor.
fn mark(run: &Run) -> u64 {
    let stage = match &run.stage {
        Stage::Map { .. } => 1,
        Stage::Combat => 2,
        Stage::Reward { .. } => 3,
        Stage::Rest { .. } => 4,
        Stage::Shop { .. } => 7,
        Stage::Omen { .. } => 8,
        Stage::Trophy { .. } => 9,
        Stage::Chest(_) => 10,
        Stage::Hollow { .. } => 11,
        Stage::Victory => 5,
        Stage::Defeat => 6,
    };
    let round = run.combat.as_ref().map(|c| c.round as u64).unwrap_or(0);
    let at = run.at.map(|(f, n)| f as u64 * 16 + n as u64 + 1).unwrap_or(0);
    stage | round << 8 | at << 40 | (run.act as u64) << 56
}

/// In-memory transport: several nodes in one process, for tests and for the
/// dev build's "four clients in one window" mode.
#[derive(Default)]
pub struct Hub {
    pub nodes: Vec<Node>,
    /// Peers whose messages are currently dropped in both directions.
    pub offline: Vec<Peer>,
}

impl Hub {
    pub fn node(&mut self, peer: Peer) -> &mut Node {
        self.nodes.iter_mut().find(|n| n.me == peer).expect("known peer")
    }

    /// Delivers messages until every outbox is empty.
    pub fn pump(&mut self, c: &Content) {
        for _ in 0..10_000 {
            let mut mail: Vec<(Peer, Peer, Msg)> = Vec::new();
            let peers: Vec<Peer> = self.nodes.iter().map(|n| n.me).collect();
            for node in &mut self.nodes {
                let from = node.me;
                for (to, msg) in node.take_out() {
                    match to {
                        To::One(p) => mail.push((from, p, msg)),
                        To::All => mail.extend(peers.iter().filter(|p| **p != from).map(|p| (from, *p, msg.clone()))),
                    }
                }
            }
            if mail.is_empty() {
                return;
            }
            for (from, to, msg) in mail {
                if self.offline.contains(&from) || self.offline.contains(&to) {
                    continue;
                }
                // through bytes, exactly as the real transport does it
                let Some(msg) = Msg::decode(&msg.encode()) else { panic!("message does not round-trip") };
                if let Some(node) = self.nodes.iter_mut().find(|n| n.me == to) {
                    node.handle(c, from, msg);
                }
            }
        }
        panic!("message storm: nodes never went quiet");
    }
}
