//! Moves `nav_net` messages over Steam: lobbies for finding each other,
//! Networking Messages (relayed P2P) for the traffic. In the dev build the
//! same node can instead talk to in-process bot clients with no Steam at all.

use crate::session::{Db, Session};
use nav_core::{RunEvent, Stage};
use crate::steam::Steam;
use crate::AppState;
use bevy::prelude::*;
use nav_core::run_bot;
use nav_net::{Msg, Node, Note, Peer, To};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use steamworks::networking_types::{NetworkingIdentity, SendFlags};
use steamworks::{ChatMemberStateChange, GameLobbyJoinRequested, LobbyChatUpdate, LobbyId, LobbyType, SteamId};

/// Lobby metadata that tells our lobbies apart from everyone else's on a shared app id.
const GAME_KEY: &str = "game";
const GAME_VALUE: &str = "tropa_v_nav";
const CODE_KEY: &str = "code";
const CHANNEL: u32 = 0;

pub fn plugin(app: &mut App) {
    app.init_resource::<NetStatus>()
        .add_systems(Startup, hook_callbacks.run_if(resource_exists::<Steam>))
        .add_systems(PreUpdate, (steam_events, receive).chain().run_if(resource_exists::<Steam>))
        .add_systems(Update, (crate::session::submit, local_peers, autopilot, notes, follow_stage).chain().run_if(resource_exists::<Net>))
        .add_systems(PostUpdate, send.run_if(resource_exists::<Net>));
}

/// The co-op session this machine is part of.
#[derive(Resource)]
pub struct Net {
    pub node: Node,
    /// Steam lobby, or `None` for an offline dev session.
    pub lobby: Option<LobbyId>,
    pub code: String,
    /// Extra clients living in this process (dev build), played by the autopilot.
    pub locals: Vec<Node>,
    pub(crate) timer: f32,
    /// Dev: the local seat is played by the autopilot too.
    pub bot_me: bool,
    /// The finished run has been written to the profile.
    pub booked: bool,
    /// A dev shortcut started this run: it is neither saved nor booked.
    pub ephemeral: bool,
}

impl Net {
    pub fn me(&self) -> Peer {
        self.node.me
    }
}

/// What the lobby screens show while Steam is working on a request.
#[derive(Resource, Default)]
pub struct NetStatus {
    /// The host of a party over the local network has gone: the party screen says so.
    pub host_gone: bool,
    pub busy: bool,
    /// A message key or ready text to show the player.
    pub error: Option<String>,
    /// The lobby list changed; the lobby screen should redraw.
    pub dirty: bool,
    /// The run changed; map, reward and rest screens should redraw.
    pub run_dirty: bool,
    /// Pings not yet shown: `(player slot, kind, unit)`.
    pub pings: Vec<(u8, u8, nav_core::UnitRef)>,
    /// Story votes not yet counted: `(player slot, page text key, skip)`.
    pub story_votes: Vec<(u8, String, bool)>,
    /// Other players' hands on the book, tears and turns, not yet shown (`Note::Book*`).
    pub book: Vec<Note>,
    /// Cards other players are holding.
    pub held: Vec<Note>,
    /// What happened to this player's deck, not yet shown (see `deck_fx`).
    pub deck: Vec<RunEvent>,
    /// Votes and lots on the path map, not yet shown (see `run_ui::map_party`).
    pub map: Vec<RunEvent>,
    /// The other players' cursors over the map and their nudges, as last heard.
    pub pointers: Vec<(u8, [f32; 2])>,
    pub nudges: Vec<(u8, u8)>,
    pub ink: Vec<Note>,
    pub ink_resync: bool,
    /// Seconds the host has not been heard from, once that is long enough to call it lost.
    pub lost: Option<f32>,
    /// The connection is lost: the transport is to try again (Steam joins the lobby anew, the
    /// local network dials the host again).
    pub redial: bool,
    /// A saved party to pick up in the next lobby this machine opens as host.
    pub resume: Option<crate::profile::PartySave>,
}

enum SteamEv {
    Created(Result<LobbyId, String>),
    Found(Vec<LobbyId>),
    Joined(Result<LobbyId, String>),
    Member { lobby: LobbyId, user: SteamId, left: bool },
    Invited(LobbyId),
}

/// Results of Steam requests, filled from Steam's callbacks.
#[derive(Resource, Clone, Default)]
pub struct Inbox(Arc<Mutex<Vec<SteamEv>>>);

/// Steam ids allowed to open a message session with us: the people in our lobby.
#[derive(Resource, Clone, Default)]
struct Allowed(Arc<Mutex<HashSet<u64>>>);

impl Inbox {
    fn push(&self, e: SteamEv) {
        if let Ok(mut q) = self.0.lock() {
            q.push(e);
        }
    }
}

/// Six characters without look-alikes (no 0/O, 1/I).
pub fn new_code(seed: u64) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = nav_core::Rng::new(seed);
    (0..6).map(|_| ALPHABET[rng.below(ALPHABET.len() as u32) as usize] as char).collect()
}

fn hook_callbacks(mut commands: Commands, steam: Res<Steam>) {
    let inbox = Inbox::default();
    let allowed = Allowed::default();
    let c = &steam.client;

    let q = inbox.clone();
    std::mem::forget(c.register_callback(move |u: LobbyChatUpdate| {
        let left = !matches!(u.member_state_change, ChatMemberStateChange::Entered);
        q.push(SteamEv::Member { lobby: u.lobby, user: u.user_changed, left });
    }));
    // accepting an invite, or "Join game" on a friend, from the overlay
    let q = inbox.clone();
    std::mem::forget(c.register_callback(move |r: GameLobbyJoinRequested| q.push(SteamEv::Invited(r.lobby_steam_id))));

    let ok = allowed.clone();
    c.networking_messages().session_request_callback(move |request| {
        let known = request.remote().steam_id().is_some_and(|id| ok.0.lock().is_ok_and(|set| set.contains(&id.raw())));
        if known {
            request.accept();
        } else {
            request.reject();
        }
    });

    // launched from an invite while the game was closed: `+connect_lobby <id>`
    let args: Vec<String> = std::env::args().collect();
    if let Some(id) = args.iter().position(|a| a == "+connect_lobby").and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u64>().ok()) {
        inbox.push(SteamEv::Invited(LobbyId::from_raw(id)));
    }
    commands.insert_resource(inbox);
    commands.insert_resource(allowed);
}

// ---- requests from the UI ----

pub fn create_lobby(steam: &Steam, status: &mut NetStatus, inbox: &Inbox) {
    status.busy = true;
    status.error = None;
    let q = inbox.clone();
    steam.client.matchmaking().create_lobby(LobbyType::Public, nav_net::MAX_PLAYERS as u32, move |r| {
        q.push(SteamEv::Created(r.map_err(|e| e.to_string())));
    });
}

pub fn join_by_code(steam: &Steam, status: &mut NetStatus, inbox: &Inbox, code: &str) {
    use steamworks::{LobbyKey, LobbyListFilter, StringFilter, StringFilterKind};
    status.busy = true;
    status.error = None;
    let code = code.trim().to_uppercase();
    let mm = steam.client.matchmaking();
    mm.set_lobby_list_filter(LobbyListFilter {
        string: Some(vec![
            StringFilter(LobbyKey::new(GAME_KEY), GAME_VALUE, StringFilterKind::Equal),
            StringFilter(LobbyKey::new(CODE_KEY), &code, StringFilterKind::Equal),
        ]),
        ..Default::default()
    });
    let q = inbox.clone();
    mm.request_lobby_list(move |r| q.push(SteamEv::Found(r.unwrap_or_default())));
}

pub fn leave(commands: &mut Commands, steam: Option<&Steam>, net: &Net) {
    if let (Some(steam), Some(lobby)) = (steam, net.lobby) {
        steam.client.matchmaking().leave_lobby(lobby);
        steam.client.friends().set_rich_presence("status", None);
    }
    commands.remove_resource::<Net>();
}

/// Opens the Steam overlay's friend picker for this lobby.
pub fn invite(steam: &Steam, net: &Net) {
    if let Some(lobby) = net.lobby {
        steam.client.friends().activate_invite_dialog(lobby);
    }
}

/// A session that never touches Steam: this machine hosts, and `bots` extra
/// clients live in the same process. With no bots it is simply a solo run.
pub fn start_offline(commands: &mut Commands, db: &nav_core::Content, name: &str, bots: usize) {
    let mut node = Node::host(1, name);
    let mut locals: Vec<Node> = (0..bots).map(|i| Node::join(2 + i as u64, &format!("bot {}", i + 1), 1)).collect();
    route(db, &mut node, &mut locals);
    // each bot takes a hero and readies up; the host's own seat is left for the player
    let heroes: Vec<String> = db.heroes.keys().cloned().collect();
    for (i, bot) in locals.iter_mut().enumerate() {
        bot.pick_hero(heroes.get(i + 1).cloned(), 10);
    }
    route(db, &mut node, &mut locals);
    for bot in &mut locals {
        bot.set_ready(true);
    }
    route(db, &mut node, &mut locals);
    // dev shortcut: TROPA_HERO seats the player on a free hero, to look at the lobby with one chosen
    #[cfg(feature = "dev")]
    if let Ok(hero) = std::env::var("TROPA_HERO") {
        node.pick_hero(Some(hero), 1);
        route(db, &mut node, &mut locals);
    }
    commands.insert_resource(Net { node, lobby: None, code: String::new(), locals, timer: 0.0, bot_me: false, booked: false, ephemeral: false });
}

/// Picks up a saved solo run where it was left.
pub fn resume_solo(commands: &mut Commands, name: &str, run: nav_core::Run) {
    let hero = run.players.first().map(|p| p.hero.clone()).unwrap_or_default();
    let node = Node::resume(1, name, hero, run);
    commands.insert_resource(Net { node, lobby: None, code: String::new(), locals: vec![], timer: 0.0, bot_me: false, booked: false, ephemeral: false });
}

/// The dev shortcut `TROPA_FIGHT` and what goes with it: the run opens on that fight, set up as the
/// environment says. Every machine in a party must do the same, with the same environment.
#[cfg(feature = "dev")]
pub fn dev_fight(db: &nav_core::Content, run: &mut nav_core::Run, enemies: &[String]) {
    run.force_fight(db, &enemies);
    // TROPA_FOE_HP=5 leaves every foe that weak, to watch them die
    if let (Some(hp), Some(c)) = (std::env::var("TROPA_FOE_HP").ok().and_then(|v| v.parse::<i32>().ok()), run.combat.as_mut()) {
        for e in &mut c.enemies {
            e.hp = e.hp.min(hp);
        }
    }
    // TROPA_HERO_HP=2 leaves the heroes that weak, to watch them go down; 2@0 only the first
    if let (Ok(v), Some(c)) = (std::env::var("TROPA_HERO_HP"), run.combat.as_mut()) {
        let (hp, only) = v.split_once('@').map_or((v.as_str(), None), |(a, b)| (a, b.parse::<usize>().ok()));
        if let Ok(hp) = hp.parse::<i32>() {
            for (i, h) in c.heroes.iter_mut().enumerate() {
                if only.is_none_or(|o| o == i) {
                    h.hp = h.hp.min(hp);
                }
            }
        }
    }
    // TROPA_FOE_STR=40 makes the foes hit that much harder, to watch limbs and heads go
    if let (Some(n), Some(c)) = (std::env::var("TROPA_FOE_STR").ok().and_then(|v| v.parse::<i32>().ok()), run.combat.as_mut()) {
        for e in &mut c.enemies {
            e.statuses.insert(nav_core::StatusKind::Strength, n);
        }
    }
    // TROPA_HAND=a,b deals the heroes these cards, `all` every card there is
    if let (Ok(hand), Some(c)) = (std::env::var("TROPA_HAND"), run.combat.as_mut()) {
        let ids: Vec<String> = if hand == "all" { db.cards.keys().cloned().collect() } else { hand.split(',').map(|s| s.trim().to_string()).filter(|s| db.cards.contains_key(s)).collect() };
        for h in &mut c.heroes {
            h.hand = ids.iter().enumerate().map(|(i, def)| nav_core::Card { uid: 90_000 + i as u32, def: def.clone(), upgraded: false, free: false }).collect();
        }
    }
    // TROPA_ACT=2 puts that fight on another act's backdrop
    if let Some(act) = std::env::var("TROPA_ACT").ok().and_then(|a| a.parse().ok()) {
        run.act = act;
    }
}

/// Like [`start_offline`], but picks a hero for the host too and sets out at once.
#[cfg(feature = "dev")]
pub fn start_offline_run(commands: &mut Commands, db: &nav_core::Content, bots: usize, seed: u64, hero: Option<String>, bot_me: bool, force: Option<nav_core::NodeKind>) {
    let mut node = Node::host(1, "dev");
    let mut locals: Vec<Node> = (0..bots).map(|i| Node::join(2 + i as u64, &format!("bot {}", i + 1), 1)).collect();
    route(db, &mut node, &mut locals);
    let mine = hero.unwrap_or_else(|| db.heroes.keys().next().cloned().unwrap_or_default());
    node.pick_hero(Some(mine.clone()), 10);
    let rest: Vec<String> = db.heroes.keys().filter(|h| **h != mine).cloned().collect();
    for (i, bot) in locals.iter_mut().enumerate() {
        bot.pick_hero(rest.get(i).cloned(), 10);
    }
    route(db, &mut node, &mut locals);
    node.set_ready(true);
    for bot in &mut locals {
        bot.set_ready(true);
    }
    route(db, &mut node, &mut locals);
    // dev shortcut: everything unlocked
    node.start(db, seed, (2, 2, 2));
    route(db, &mut node, &mut locals);
    // every in-process node gets the same forced stage, so they stay in step
    if let Some(kind) = force {
        for n in std::iter::once(&mut node).chain(locals.iter_mut()) {
            if let Some(run) = n.state.as_mut() {
                run.force_stage(db, kind);
            }
        }
    }
    // dev shortcut: TROPA_FORCE=trophy opens on a boss's trophy vote
    if std::env::var("TROPA_FORCE").as_deref() == Ok("trophy") {
        for n in std::iter::once(&mut node).chain(locals.iter_mut()) {
            if let Some(run) = n.state.as_mut() {
                let offers = run.roll_trophies(db);
                run.stage = Stage::Trophy { offers, picks: vec![None; run.players.len()] };
            }
        }
    }
    // dev shortcut: TROPA_RELICS=<n> hands everyone that many relics, to see how they lie
    if let Some(n) = std::env::var("TROPA_RELICS").ok().and_then(|n| n.parse::<usize>().ok()) {
        let all: Vec<String> = db.relics.keys().take(n).cloned().collect();
        for nd in std::iter::once(&mut node).chain(locals.iter_mut()) {
            if let Some(run) = nd.state.as_mut() {
                for pl in &mut run.players {
                    pl.relics.extend(all.iter().cloned());
                }
            }
        }
    }
    // dev shortcut: TROPA_HEADLESS=1 has the player in that seat lying without a head
    if let Some(seat) = std::env::var("TROPA_HEADLESS").ok().and_then(|s| s.parse::<usize>().ok()) {
        for n in std::iter::once(&mut node).chain(locals.iter_mut()) {
            if let Some(pl) = n.state.as_mut().and_then(|run| run.players.get_mut(seat)) {
                pl.beheaded = true;
                pl.hp = 0;
            }
        }
    }
    // dev shortcut: TROPA_WALKS=1 has the local seat up with no head; TROPA_ARMS=3 short of those arms (1 weapon, 2 shield)
    for n in std::iter::once(&mut node).chain(locals.iter_mut()) {
        if let Some(pl) = n.state.as_mut().and_then(|run| run.players.get_mut(0)) {
            if std::env::var("TROPA_WALKS").is_ok_and(|v| !v.is_empty()) {
                pl.walks_headless = true;
            }
            if let Some(arms) = std::env::var("TROPA_ARMS").ok().and_then(|v| v.parse::<u8>().ok()) {
                pl.lost_arms = arms;
                pl.lost_limbs = arms.count_ones() as u8;
            }
        }
    }
    // dev shortcut: TROPA_OMEN=well opens on that omen
    if let Ok(id) = std::env::var("TROPA_OMEN") {
        for n in std::iter::once(&mut node).chain(locals.iter_mut()) {
            if let Some(run) = n.state.as_mut() {
                run.stage = Stage::Omen { id: id.clone(), picks: vec![None; run.players.len()] };
            }
        }
    }
    // dev shortcut: TROPA_POTIONS=a,b fills every belt with those draughts
    if let Ok(list) = std::env::var("TROPA_POTIONS") {
        let potions: Vec<String> = list.split(',').map(|s| s.trim().to_string()).filter(|s| db.potions.contains_key(s)).collect();
        for n in std::iter::once(&mut node).chain(locals.iter_mut()) {
            if let Some(run) = n.state.as_mut() {
                for p in &mut run.players {
                    p.potions = potions.clone();
                }
            }
        }
    }
    // dev shortcut: TROPA_FIGHT=leshy,tree_guard opens on that fight
    if let Ok(list) = std::env::var("TROPA_FIGHT") {
        let enemies: Vec<String> = list.split(',').map(|s| s.trim().to_string()).filter(|s| db.enemies.contains_key(s)).collect();
        if !enemies.is_empty() {
            for n in std::iter::once(&mut node).chain(locals.iter_mut()) {
                if let Some(run) = n.state.as_mut() {
                    dev_fight(db, run, &enemies);
                }
            }
            // no events announced this fight, so the battle screen is handed its state directly
            if let (Some(combat), Some(me)) = (node.state.as_ref().and_then(|r| r.combat.clone()), node.my_slot()) {
                commands.insert_resource(Session::from_state(combat, me, []));
            }
        }
    }
    commands.insert_resource(Net { node, lobby: None, code: String::new(), locals, timer: 0.0, bot_me, booked: false, ephemeral: true });
}

// ---- systems ----

fn steam_events(
    mut commands: Commands,
    db: Res<Db>,
    steam: Res<Steam>,
    inbox: Res<Inbox>,
    allowed: Res<Allowed>,
    mut net: Option<ResMut<Net>>,
    mut status: ResMut<NetStatus>,
    mut next: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    let events: Vec<SteamEv> = inbox.0.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
    let mm = steam.client.matchmaking();
    // the connection is lost: knock at the lobby's door again
    if status.redial {
        if let Some(lobby) = net.as_ref().and_then(|n| n.lobby) {
            status.redial = false;
            info!("trying to get back into the lobby");
            let q = inbox.clone();
            mm.join_lobby(lobby, move |r| q.push(SteamEv::Joined(r.map_err(|_| "join failed".to_string()))));
        }
    }
    let me = steam.client.user().steam_id();
    let name = steam.client.friends().name();
    for e in events {
        match e {
            SteamEv::Created(Ok(lobby)) => {
                let code = new_code(lobby.raw() ^ time.elapsed().as_nanos() as u64);
                mm.set_lobby_data(lobby, GAME_KEY, GAME_VALUE);
                mm.set_lobby_data(lobby, CODE_KEY, &code);
                // a saved party is picked up in the new lobby; its players come back to their seats
                let node = match status.resume.take() {
                    Some(save) => Node::resume_party(me.raw(), &name, save.lobby, save.n, save.run),
                    None => Node::host(me.raw(), &name),
                };
                commands.insert_resource(Net { node, lobby: Some(lobby), code, locals: vec![], timer: 0.0, bot_me: false, booked: false, ephemeral: false });
                status.busy = false;
                status.dirty = true;
                next.set(AppState::Lobby);
            }
            SteamEv::Found(list) => match list.first() {
                Some(lobby) => {
                    let q = inbox.clone();
                    mm.join_lobby(*lobby, move |r| q.push(SteamEv::Joined(r.map_err(|_| "join failed".to_string()))));
                }
                None => {
                    status.busy = false;
                    status.error = Some("coop-error-not-found".into());
                }
            },
            SteamEv::Invited(lobby) => {
                if net.is_none() {
                    status.busy = true;
                    let q = inbox.clone();
                    mm.join_lobby(lobby, move |r| q.push(SteamEv::Joined(r.map_err(|_| "join failed".to_string()))));
                }
            }
            // back in the lobby after the connection dropped: hello again, and the run goes on
            SteamEv::Joined(Ok(lobby)) if net.as_ref().is_some_and(|n| n.lobby == Some(lobby)) => {
                if let Ok(mut set) = allowed.0.lock() {
                    set.extend(mm.lobby_members(lobby).iter().map(|m| m.raw()));
                }
                if let Some(net) = net.as_mut() {
                    info!("rejoined the lobby; asking the host for the run");
                    net.node.rejoin();
                }
            }
            SteamEv::Joined(Ok(lobby)) => {
                // someone else's Spacewar lobby would not carry our tag
                if mm.lobby_data(lobby, GAME_KEY).as_deref() != Some(GAME_VALUE) {
                    mm.leave_lobby(lobby);
                    status.busy = false;
                    status.error = Some("coop-error-not-found".into());
                    continue;
                }
                let host = mm.lobby_owner(lobby);
                let code = mm.lobby_data(lobby, CODE_KEY).unwrap_or_default();
                // the host must be allowed before our hello reaches them and they answer
                if let Ok(mut set) = allowed.0.lock() {
                    set.extend(mm.lobby_members(lobby).iter().map(|m| m.raw()));
                }
                commands.insert_resource(Net { node: Node::join(me.raw(), &name, host.raw()), lobby: Some(lobby), code, locals: vec![], timer: 0.0, bot_me: false, booked: false, ephemeral: false });
                status.busy = false;
                status.dirty = true;
                next.set(AppState::Lobby);
            }
            SteamEv::Created(Err(e)) | SteamEv::Joined(Err(e)) => {
                warn!("lobby request failed: {e}");
                status.busy = false;
                status.error = Some("coop-error-steam".into());
            }
            SteamEv::Member { lobby, user, left } => {
                let Some(net) = net.as_mut().filter(|n| n.lobby == Some(lobby)) else { continue };
                if left {
                    info!("player {} left the lobby", user.raw());
                    net.node.peer_left(user.raw());
                }
                // a client cut off sees everyone leave and itself made owner: that is its own
                // connection gone, not the host's, and it waits to get back in
                let owner = mm.lobby_owner(lobby).raw();
                let alone = mm.lobby_members(lobby).len() <= 1;
                if owner == net.me() && !net.node.is_host() && alone && net.node.state.is_some() {
                    warn!("cut off from the party: everyone seems gone, waiting to get back in");
                    status.lost.get_or_insert(0.0);
                } else {
                    // Steam picks the new owner itself when the host goes
                    net.node.host_changed(&db.0, owner);
                }
                status.dirty = true;
            }
        }
    }
    if let Some(net) = &net {
        if let Some(lobby) = net.lobby {
            if let Ok(mut set) = allowed.0.lock() {
                *set = mm.lobby_members(lobby).iter().map(|m| m.raw()).collect();
            }
        }
    }
}

fn receive(steam: Res<Steam>, db: Res<Db>, mut net: Option<ResMut<Net>>) {
    let messages = steam.client.networking_messages().receive_messages_on_channel(CHANNEL, 64);
    let Some(net) = net.as_mut() else { return };
    for m in messages {
        let Some(from) = m.identity_peer().steam_id() else { continue };
        if let Some(msg) = Msg::decode(m.data()) {
            net.node.handle(&db.0, from.raw(), msg);
        }
    }
}

pub(crate) fn send(steam: Option<Res<Steam>>, db: Res<Db>, mut net: ResMut<Net>) {
    let net = &mut *net;
    let Some(lobby) = net.lobby else {
        route(&db.0, &mut net.node, &mut net.locals);
        return;
    };
    let Some(steam) = steam else { return };
    let out = net.node.take_out();
    if out.is_empty() {
        return;
    }
    let me = net.node.me;
    let members: Vec<SteamId> = steam.client.matchmaking().lobby_members(lobby).into_iter().filter(|m| m.raw() != me).collect();
    let nm = steam.client.networking_messages();
    for (to, msg) in out {
        let bytes = msg.encode();
        let targets: Vec<SteamId> = match to {
            To::One(p) => vec![SteamId::from_raw(p)],
            To::All => members.clone(),
        };
        for t in targets {
            // reliable and unbatched: a turn-based game cares about order, not throughput
            if let Err(e) = nm.send_message_to_user(NetworkingIdentity::new_steam_id(t), SendFlags::RELIABLE_NO_NAGLE | SendFlags::AUTO_RESTART_BROKEN_SESSION, &bytes, CHANNEL) {
                warn!("send to {} failed: {e:?}", t.raw());
            }
        }
    }
}

/// Delivers messages between the node and its in-process peers until all are quiet.
fn route(db: &nav_core::Content, node: &mut Node, locals: &mut [Node]) {
    for _ in 0..64 {
        let mut mail: Vec<(Peer, To, Msg)> = node.take_out().into_iter().map(|(to, m)| (node.me, to, m)).collect();
        for l in locals.iter_mut() {
            let from = l.me;
            mail.extend(l.take_out().into_iter().map(|(to, m)| (from, to, m)));
        }
        if mail.is_empty() {
            return;
        }
        for (from, to, msg) in mail {
            let deliver = |n: &mut Node| {
                let wanted = match to {
                    To::One(p) => p == n.me,
                    To::All => from != n.me,
                };
                if wanted {
                    if let Some(m) = Msg::decode(&msg.encode()) {
                        n.handle(db, from, m);
                    }
                }
            };
            deliver(node);
            for l in locals.iter_mut() {
                deliver(l);
            }
        }
    }
}

/// In-process clients take their turns with the autopilot, at a watchable pace.
fn local_peers(db: Res<Db>, time: Res<Time>, mut net: ResMut<Net>, session: Option<Res<Session>>) {
    if (net.locals.is_empty() && !net.bot_me) || session.is_some_and(|s| !s.idle()) {
        return;
    }
    net.timer -= time.delta_secs();
    if net.timer > 0.0 {
        return;
    }
    net.timer = 0.45;
    let net = &mut *net;
    let mut seats: Vec<&mut Node> = net.locals.iter_mut().collect();
    if net.bot_me {
        seats.insert(0, &mut net.node);
    }
    for seat in seats {
        let (Some(slot), Some(state)) = (seat.my_slot(), seat.state.as_ref()) else { continue };
        if let Some(action) = run_bot(&db.0, state, slot) {
            seat.submit(&db.0, action);
            return;
        }
    }
}

fn autopilot(db: Res<Db>, time: Res<Time>, mut net: ResMut<Net>, mut wait: Local<f32>) {
    let dt = time.delta_secs();
    net.node.tick(dt);
    *wait -= dt;
    if *wait <= 0.0 {
        *wait = 0.6;
        net.node.autopilot(&db.0);
    }
}

/// Turns protocol notes into what the screens need.
fn notes(
    mut commands: Commands,
    mut net: ResMut<Net>,
    mut session: Option<ResMut<Session>>,
    mut status: ResMut<NetStatus>,
    l10n: Res<crate::l10n::L10n>,
    profile: Res<crate::profile::Profile>,
) {
    let notes = net.node.take_notes();
    // a lone player's run is kept on disk as it goes; the autopilot's dev runs are not
    let solo = net.lobby.is_none() && net.locals.is_empty() && !net.bot_me && !net.ephemeral && net.code.is_empty();
    if solo && notes.iter().any(|n| matches!(n, Note::Applied { .. } | Note::Started)) {
        match net.node.state.as_ref() {
            Some(run) if matches!(run.stage, Stage::Victory | Stage::Defeat) => profile.clear_run(),
            Some(run) => profile.save_run(run),
            None => {}
        }
    }
    // the host of a party keeps its run too, after each fight and each step between them (not in
    // the middle of a fight, which is picked up from its start)
    let party = !solo && net.node.is_host() && !net.ephemeral && net.locals.is_empty() && net.node.lobby.players.len() > 1;
    if party && notes.iter().any(|n| matches!(n, Note::Applied { .. } | Note::Started)) {
        match net.node.state.as_ref() {
            Some(run) if matches!(run.stage, Stage::Victory | Stage::Defeat) => profile.clear_party(),
            Some(run) if run.stage != Stage::Combat => profile.save_party(&crate::profile::PartySave { lobby: net.node.lobby.clone(), n: net.node.n, run: run.clone() }),
            _ => {}
        }
    }
    for note in notes {
        match note {
            Note::LobbyChanged | Note::HostChanged => status.dirty = true,
            Note::OtherBuild => {
                status.error = Some("coop-error-build".into());
                status.dirty = true;
                commands.remove_resource::<Net>();
                return;
            }
            Note::Started => status.run_dirty = true,
            Note::Applied { events, .. } => {
                status.run_dirty = true;
                let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { continue };
                for e in events.iter() {
                    debug!("run: {e:?}");
                }
                status.map.extend(events.iter().filter(|e| matches!(e, RunEvent::Voted { .. } | RunEvent::Lot { .. })).cloned());
                status.deck.extend(events.iter().filter(|e| match e {
                    RunEvent::CardGained { player, .. } | RunEvent::Upgraded { player, .. } | RunEvent::CardRemoved { player, .. } => *player == me,
                    RunEvent::RelicGained { player, .. } | RunEvent::PotionGained { player, .. } => *player == me,
                    // the hands at the chest are shaken out before anything is handed over
                    RunEvent::Thrown { .. } => true,
                    RunEvent::CardTaken { player, card } => *player == me && card.is_some(),
                    _ => false,
                }).cloned());
                let combat_events: Vec<nav_core::Event> = events.iter().filter_map(|e| if let RunEvent::Combat(c) = e { Some(c.clone()) } else { None }).collect();
                let started = events.iter().any(|e| matches!(e, RunEvent::CombatStarted));
                match (session.as_mut(), run.combat.as_ref()) {
                    // a new fight: its opening already sits in the state
                    (_, Some(combat)) if started => commands.insert_resource(Session::from_state(combat.clone(), me, combat_events)),
                    (Some(s), Some(combat)) => {
                        s.truth = combat.clone();
                        s.queue.extend(combat_events);
                    }
                    _ => {}
                }
            }
            Note::Resynced => {
                status.run_dirty = true;
                status.ink_resync = true;
                if let (Some(combat), Some(me)) = (net.node.state.as_ref().and_then(|r| r.combat.clone()), net.node.my_slot()) {
                    // no animation to catch up on: show the fight as it now stands
                    commands.insert_resource(Session::from_state(combat, me, []));
                }
            }
            Note::Refused(e) => {
                let text = match e {
                    nav_core::RunError::Combat(c) => l10n.t(&format!("error-{c:?}")),
                    other => l10n.t(&format!("error-{other:?}")),
                };
                match session.as_mut() {
                    Some(s) => s.error = Some((text, 1.6)),
                    None => status.error = Some(text),
                }
            }
            Note::Ping { from, kind, at } => status.pings.push((from, kind, at)),
            Note::StoryVote { from, page, skip } => status.story_votes.push((from, page, skip)),
            note @ Note::CardHeld { .. } => status.held.push(note),
            note @ (Note::BookHand { .. } | Note::BookTear { .. } | Note::BookTurn { .. } | Note::BookCarry { .. }) => status.book.push(note),
            Note::MapPointer { from, at } => status.pointers.push((from, at)),
            note @ (Note::MapInk { .. } | Note::MapInkRequest { .. }) => status.ink.push(note),
            Note::Nudge { from, node } => status.nudges.push((from, node)),
        }
    }
}

/// Opens the screen that matches where the run is. A fight is left only
/// once its last blow has been shown.
fn follow_stage(
    net: Res<Net>,
    session: Option<Res<Session>>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    time: Res<Time>,
    lot: Option<Res<crate::map_party::MapLot>>,
    omen_lot: Option<Res<crate::run_ui::OmenLot>>,
    status: Res<NetStatus>,
    mut linger: Local<f32>,
) {
    let Some(run) = net.node.state.as_ref() else { return };
    let want = match run.stage {
        Stage::Map { .. } => AppState::Map,
        Stage::Combat => AppState::Battle,
        Stage::Reward { .. } => AppState::Reward,
        Stage::Rest { .. } => AppState::Rest,
        Stage::Shop { .. } => AppState::Shop,
        Stage::Omen { .. } | Stage::Trophy { .. } | Stage::Chest(_) => AppState::Omen,
        Stage::Victory | Stage::Defeat => AppState::RunEnd,
    };
    let now = *state.get();
    if now == want || !matches!(now, AppState::Lobby | AppState::Map | AppState::Battle | AppState::Reward | AppState::Rest | AppState::Shop | AppState::Omen | AppState::RunEnd | AppState::Menu) {
        *linger = 0.0;
        return;
    }
    // a lot drawn on the map is watched to its end before the party sets off
    // (or about to be: the map screen has not yet taken up the lot heard of this frame)
    let drawn = status.map.iter().any(|e| matches!(e, RunEvent::Lot { tied, .. } if !tied.is_empty()));
    if now == AppState::Map && (lot.is_some() || drawn) {
        return;
    }
    // so is one drawn over an omen's tied options, on the omen's own window
    if now == AppState::Omen && (omen_lot.is_some() || drawn) {
        return;
    }
    if now == AppState::Battle {
        if session.as_ref().is_some_and(|s| !s.idle()) {
            return;
        }
        // let the killing blow land before the screen changes; a death is given time to be seen
        *linger += time.delta_secs();
        let hold = if matches!(run.stage, Stage::Defeat) { 2.8 } else { 0.9 };
        if *linger < hold {
            return;
        }
    }
    if want == AppState::Battle && session.is_none() {
        return;
    }
    *linger = 0.0;
    next.set(want);
}
