use nav_core::*;
use nav_net::*;
use std::path::PathBuf;

const HEROES: [&str; 4] = ["bogatyr", "volhv", "volkolak", "znaharka"];

#[test]
fn map_ink_is_shared_in_both_directions_without_changing_the_run() {
    let c = content();
    let mut hub = party(&c, 3, 42);
    let before = hashes(&hub);
    for node in &mut hub.nodes { node.take_notes(); }
    let changes = vec![
        InkChange::Begin([10.0, 20.0]),
        InkChange::Point([100.0, 20.0]),
        InkChange::Rub { from: [50.0, 0.0], to: [50.0, 40.0] },
        InkChange::Clear,
    ];
    for sender in 1..=3 {
        for change in &changes { hub.node(sender).map_ink(To::All, change.clone()); }
        hub.pump(&c);
        for node in &mut hub.nodes {
            let received: Vec<_> = node.take_notes().into_iter().filter_map(|n| match n {
                Note::MapInk { from, change, .. } => { assert_eq!(from, (sender - 1) as u8); Some(change) }
                _ => None,
            }).collect();
            assert_eq!(received, if node.me == sender { vec![] } else { changes.clone() });
        }
    }
    assert_eq!(hashes(&hub), before);
}

#[test]
fn map_ink_snapshot_can_be_requested_after_rejoining() {
    let c = content();
    let mut hub = party(&c, 2, 42);
    hub.offline.push(2);
    hub.node(1).peer_left(2);
    hub.offline.clear();
    hub.node(2).rejoin();
    hub.pump(&c);
    for node in &mut hub.nodes { node.take_notes(); }
    hub.node(2).request_map_ink();
    hub.pump(&c);
    assert!(hub.node(1).take_notes().iter().any(|n| matches!(n, Note::MapInkRequest { from: 2, .. })));
    let all = InkChange::Snapshot(vec![vec![[10.0, 20.0], [30.0, 40.0]]]);
    hub.node(1).map_ink(To::One(2), all.clone());
    hub.pump(&c);
    assert!(hub.node(2).take_notes().iter().any(|n| matches!(n, Note::MapInk { from: 0, change, .. } if *change == all)));
}

#[test]
fn map_ink_rejects_other_runs_acts_unknown_senders_and_invalid_points() {
    let c = content();
    let mut hub = party(&c, 2, 42);
    let node = hub.node(1);
    node.take_notes();
    let run = node.state.as_ref().unwrap();
    let (seed, act) = (run.seed, run.act);
    for (from, seed, act, change) in [
        (2, seed + 1, act, InkChange::Clear),
        (2, seed, act + 1, InkChange::Clear),
        (99, seed, act, InkChange::Clear),
        (2, seed, act, InkChange::Point([f32::NAN, 0.0])),
    ] {
        node.handle(&c, from, Msg::MapInk { seed, act, change });
    }
    assert!(node.take_notes().is_empty());
}

fn content() -> Content {
    Content::load_dir(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")).unwrap()
}

/// A started run with `n` players; peer ids are 1..=n and peer 1 hosts.
fn party(c: &Content, n: usize, seed: u64) -> Hub {
    let mut hub = Hub::default();
    hub.nodes.push(Node::host(1, "host"));
    for p in 2..=n as u64 {
        hub.nodes.push(Node::join(p, &format!("p{p}"), 1));
    }
    hub.pump(c);
    for (i, node) in hub.nodes.iter_mut().enumerate() {
        node.pick_hero(Some(HEROES[i].into()), 10);
    }
    hub.pump(c);
    for node in &mut hub.nodes {
        node.set_ready(true);
    }
    hub.pump(c);
    assert!(hub.node(1).start(c, seed, (2, 2, 2)), "lobby should be startable");
    hub.pump(c);
    hub
}

fn online(hub: &Hub) -> impl Iterator<Item = &Node> {
    hub.nodes.iter().filter(|n| !hub.offline.contains(&n.me))
}

fn hashes(hub: &Hub) -> Vec<u64> {
    online(hub).map(|n| n.state.as_ref().unwrap().hash64()).collect()
}

fn assert_in_sync(hub: &Hub) {
    let h = hashes(hub);
    assert!(h.windows(2).all(|w| w[0] == w[1]), "nodes diverged: {h:x?}");
}

fn stage(hub: &Hub) -> Stage {
    online(hub).next().unwrap().state.as_ref().unwrap().stage.clone()
}

fn over(hub: &Hub) -> bool {
    matches!(stage(hub), Stage::Victory | Stage::Defeat)
}

/// One pass: every connected player's own machine picks a move and submits it,
/// then the host steers the seats nobody holds. Returns whether anything happened.
fn step(c: &Content, hub: &mut Hub) -> bool {
    let mut moved = false;
    for i in 0..hub.nodes.len() {
        let node = &mut hub.nodes[i];
        if hub.offline.contains(&node.me) {
            continue;
        }
        let (Some(slot), Some(state)) = (node.my_slot(), node.state.as_ref()) else { continue };
        if let Some(action) = run_bot(c, state, slot) {
            node.submit(c, action);
            hub.pump(c);
            assert_in_sync(hub);
            moved = true;
        }
    }
    while hub.nodes.iter_mut().find(|n| n.is_host() && !hub.offline.contains(&n.me)).is_some_and(|h| h.autopilot(c)) {
        hub.pump(c);
        assert_in_sync(hub);
        moved = true;
    }
    moved
}

fn play_out(c: &Content, hub: &mut Hub, limit: usize) -> Stage {
    for _ in 0..limit {
        if over(hub) {
            return stage(hub);
        }
        assert!(step(c, hub), "nobody can move in {:?}", stage(hub));
    }
    panic!("the run did not end in {limit} passes");
}

/// Votes and plays until the party is in a fight.
fn into_combat(c: &Content, hub: &mut Hub) {
    for _ in 0..50 {
        if stage(hub) == Stage::Combat {
            return;
        }
        assert!(step(c, hub));
    }
    panic!("never reached a fight");
}

#[test]
fn lobby_fills_and_heroes_are_unique() {
    let c = content();
    let mut hub = Hub::default();
    hub.nodes.push(Node::host(1, "host"));
    for p in 2..=5 {
        hub.nodes.push(Node::join(p, "x", 1));
    }
    hub.pump(&c);
    assert_eq!(hub.node(1).lobby.players.len(), 4, "the fifth player doesn't fit");
    let host_view = hub.node(1).lobby.clone();
    assert_eq!(hub.node(3).lobby, host_view);
    // the hub broadcasts to every node, so peer 5 hears the lobby; what matters is it has no seat
    assert_eq!(hub.node(1).lobby.slot_of(5), None);
    assert_eq!(hub.node(5).my_slot(), None);

    hub.node(1).pick_hero(Some("bogatyr".into()), 10);
    hub.node(2).pick_hero(Some("bogatyr".into()), 10);
    hub.pump(&c);
    let lobby = hub.node(4).lobby.clone();
    assert_eq!(lobby.players[0].hero.as_deref(), Some("bogatyr"));
    assert_eq!(lobby.players[1].hero, None, "a taken hero can't be picked twice");

    hub.node(2).set_ready(true);
    hub.pump(&c);
    assert!(!hub.node(1).lobby.players[1].ready, "no ready without a hero");
    assert!(!hub.node(1).start(&c, 1, (2, 2, 2)), "can't start until everyone is ready");
    assert!(!hub.node(2).start(&c, 1, (2, 2, 2)), "only the host starts");
}

#[test]
fn four_players_stay_in_sync_through_a_whole_run() {
    let c = content();
    for seed in [3, 4] {
        let mut hub = party(&c, 4, seed);
        assert_in_sync(&hub);
        let run = hub.node(2).state.clone().unwrap();
        assert_eq!(run.players.len(), 4);
        assert_eq!(run.players[2].hero, "volkolak");
        assert_eq!(hub.node(3).my_slot(), Some(2));
        let end = play_out(&c, &mut hub, 20_000);
        assert!(matches!(end, Stage::Victory | Stage::Defeat));
        assert_in_sync(&hub);
        assert!(hub.nodes.iter().all(|n| n.n == hub.nodes[0].n));
    }
}

#[test]
fn illegal_action_is_refused_and_changes_nothing() {
    let c = content();
    let mut hub = party(&c, 2, 9);
    into_combat(&c, &mut hub);
    let before = hashes(&hub);
    hub.node(2).take_notes();
    hub.node(2).submit(&c, RunAction::Combat(CombatAction::Play { card: 999_999, target: None }));
    hub.pump(&c);
    assert_eq!(hashes(&hub), before);
    assert!(hub.node(2).take_notes().contains(&Note::Refused(RunError::Combat(ActionError::NoSuchCard))));
    // and an action for the wrong stage
    hub.node(2).submit(&c, RunAction::Sleep);
    hub.pump(&c);
    assert_eq!(hashes(&hub), before);
    assert!(hub.node(2).take_notes().contains(&Note::Refused(RunError::WrongStage)));
}

#[test]
fn a_hand_on_the_book_reaches_the_others() {
    let c = content();
    let mut hub = party(&c, 3, 4);
    for n in 1..=3 {
        hub.node(n).take_notes();
    }
    hub.node(2).book_hand("story-pro_1", [0.5, 0.3, 0.1], Some(31));
    hub.node(2).book_tear("story-pro_1", 77);
    hub.node(2).book_turn("story-pro_1");
    hub.node(2).book_carry("story-pro_1", 3, Some(12), [0.2, 0.4, -0.1]);
    hub.pump(&c);
    for n in [1, 3] {
        let notes = hub.node(n).take_notes();
        assert!(notes.contains(&Note::BookHand { from: 1, page: "story-pro_1".into(), at: [0.5, 0.3, 0.1], hold: Some(31) }), "{notes:?}");
        assert!(notes.contains(&Note::BookTear { from: 1, page: "story-pro_1".into(), edge: 77 }));
        assert!(notes.contains(&Note::BookTurn { from: 1, page: "story-pro_1".into() }));
        assert!(notes.contains(&Note::BookCarry { from: 1, page: "story-pro_1".into(), piece: 3, point: Some(12), at: [0.2, 0.4, -0.1] }));
    }
    // the reader's own hand is not shown back to them
    assert!(!hub.node(2).take_notes().iter().any(|n| matches!(n, Note::BookHand { .. })));
}

#[test]
fn a_cheating_client_cannot_act_for_someone_else() {
    let c = content();
    let mut hub = party(&c, 2, 9);
    // peer 2 forges a "host says" message; the real host must ignore it
    let before = hashes(&hub);
    hub.node(1).handle(&c, 2, Msg::Did { n: 1, player: 0, action: RunAction::Vote(0) });
    hub.pump(&c);
    assert_eq!(hashes(&hub), before);
}

#[test]
fn drifted_client_is_repaired_by_snapshot() {
    let c = content();
    let mut hub = party(&c, 2, 9);
    hub.node(2).state.as_mut().unwrap().players[0].gold += 500;
    assert_ne!(hashes(&hub)[0], hashes(&hub)[1]);
    hub.node(2).take_notes();
    // moving on (here: into the first fight) makes the host publish a hash
    hub.node(1).submit(&c, RunAction::Vote(0));
    hub.node(2).submit(&c, RunAction::Vote(0));
    hub.pump(&c);
    assert_in_sync(&hub);
    assert!(hub.node(2).take_notes().contains(&Note::Resynced));
}

#[test]
fn dropped_player_is_steered_by_autopilot_and_can_return() {
    let c = content();
    let mut hub = party(&c, 3, 6);
    into_combat(&c, &mut hub);
    // peer 3 loses connection
    hub.offline.push(3);
    hub.node(1).peer_left(3);
    hub.node(2).peer_left(3);
    hub.node(1).reconnect_grace = 0.0;
    hub.node(2).reconnect_grace = 0.0;
    hub.pump(&c);
    assert!(!hub.node(2).lobby.players[2].connected);
    let floors = hub.node(1).state.as_ref().unwrap().floors_cleared;
    for _ in 0..4000 {
        if over(&hub) || hub.node(1).state.as_ref().unwrap().floors_cleared >= floors + 2 {
            break;
        }
        assert!(step(&c, &mut hub));
    }
    assert!(over(&hub) || hub.node(1).state.as_ref().unwrap().floors_cleared >= floors + 2, "the run goes on without the dropped player");
    if over(&hub) {
        return;
    }

    // peer 3 comes back with a stale run and says hello again
    hub.offline.clear();
    let stale = hub.node(3).state.as_ref().unwrap().hash64();
    assert_ne!(stale, hub.node(1).state.as_ref().unwrap().hash64());
    *hub.node(3) = Node::join(3, "p3", 1);
    hub.pump(&c);
    assert_in_sync(&hub);
    assert!(hub.node(1).lobby.players[2].connected);
    assert_eq!(hub.node(3).my_slot(), Some(2));
    assert!(!hub.node(1).autopilot(&c), "the autopilot lets go of a returned player");
    let end = play_out(&c, &mut hub, 20_000);
    assert!(matches!(end, Stage::Victory | Stage::Defeat));
    assert_in_sync(&hub);
}

#[test]
fn dropped_player_waits_for_reconnect_grace_period() {
    let c = content();
    let mut hub = party(&c, 3, 6);
    into_combat(&c, &mut hub);
    hub.offline.push(3);
    hub.node(1).peer_left(3);
    hub.node(2).peer_left(3);
    hub.pump(&c);
    assert!(!hub.node(1).lobby.players[2].connected);
    // Disconnect timer starts at 0.0, so autopilot must NOT play for player 3:
    assert!(!hub.node(1).autopilot(&c), "Autopilot must not play while reconnect timer < 120s");
    // Advance timer to 119s:
    hub.node(1).disconnect_timer[2] = 119.0;
    assert!(!hub.node(1).autopilot(&c), "Autopilot must still not play at 119s");
    // Once timer reaches 120s:
    hub.node(1).disconnect_timer[2] = 120.0;
    assert!(hub.node(1).autopilot(&c), "Autopilot only plays once 120s reconnect grace expires");
}

#[test]
fn the_run_survives_the_host_leaving() {
    let c = content();
    let mut hub = party(&c, 3, 8);
    into_combat(&c, &mut hub);
    // peer 2 asks for something the host never gets to answer
    hub.offline.push(1);
    let node = hub.node(2);
    let action = run_bot(&c, node.state.as_ref().unwrap(), node.my_slot().unwrap()).unwrap();
    let before = node.n;
    node.submit(&c, action);
    hub.pump(&c);
    assert_eq!(hub.node(2).n, before, "nothing confirms it while the host is gone");

    // Steam hands the lobby to peer 2
    for p in [2, 3] {
        hub.node(p).peer_left(1);
        hub.node(p).reconnect_grace = 0.0;
        hub.node(p).host_changed(&c, 2);
    }
    hub.pump(&c);
    assert!(hub.node(2).is_host());
    assert!(hub.node(2).n > before, "the unanswered request went to the new host");
    assert_in_sync(&hub);
    let end = play_out(&c, &mut hub, 20_000);
    assert!(matches!(end, Stage::Victory | Stage::Defeat));
    assert_in_sync(&hub);
}

#[test]
fn messages_round_trip_through_bytes() {
    let c = content();
    let mut hub = party(&c, 2, 9);
    into_combat(&c, &mut hub);
    let state = hub.nodes[0].state.clone().unwrap();
    let msg = Msg::Snapshot { n: 5, lobby: hub.nodes[0].lobby.clone(), state: Box::new(state.clone()) };
    let Some(Msg::Snapshot { n, state: back, .. }) = Msg::decode(&msg.encode()) else { panic!("lost in transit") };
    assert_eq!(n, 5);
    assert_eq!(back.hash64(), state.hash64());
}

#[test]
fn a_saved_solo_run_resumes_where_it_stopped() {
    let c = content();
    let mut hub = party(&c, 1, 5);
    into_combat(&c, &mut hub);
    assert!(step(&c, &mut hub));
    let saved = hub.nodes[0].state.clone().unwrap();
    // through text, as the save file does it
    let text = ron::to_string(&saved).unwrap();
    let loaded: Run = ron::from_str(&text).unwrap();
    assert_eq!(loaded.hash64(), saved.hash64());

    let mut node = Node::resume(1, "host", "bogatyr".into(), loaded);
    assert!(node.is_host());
    assert_eq!(node.my_slot(), Some(0));
    assert!(node.take_notes().contains(&Note::Resynced));
    for _ in 0..20_000 {
        let state = node.state.as_ref().unwrap();
        if matches!(state.stage, Stage::Victory | Stage::Defeat) {
            return;
        }
        let action = run_bot(&c, state, 0).expect("a resumed run can go on");
        node.submit(&c, action);
        assert!(!node.take_notes().iter().any(|n| matches!(n, Note::Refused(_))));
    }
    panic!("the resumed run never ended");
}

#[test]
fn a_chosen_starting_relic_reaches_the_run() {
    let c = content();
    let mut hub = Hub::default();
    hub.nodes.push(Node::host(1, "host"));
    hub.nodes.push(Node::join(2, "p2", 1));
    hub.pump(&c);
    hub.node(1).pick_hero(Some("bogatyr".into()), 10);
    hub.node(2).pick_hero(Some("volhv".into()), 10);
    hub.node(2).pick_starter(1);
    hub.node(1).pick_starter(7);
    hub.pump(&c);
    assert_eq!(hub.node(2).lobby.players[0].starter, 2, "clamped to what a shrine can offer");
    hub.node(1).set_ready(true);
    hub.node(2).set_ready(true);
    hub.pump(&c);
    assert!(hub.node(1).start(&c, 5, (2, 2, 2)));
    hub.pump(&c);
    let run = hub.node(2).state.clone().unwrap();
    assert_eq!(run.players[0].relics, vec!["bog_alt_brother_bracelet".to_string()]);
    assert_eq!(run.players[1].relics, vec!["vol_alt_carved_bone".to_string()]);
}


#[test]
fn a_client_cut_off_comes_back_with_rejoin_and_is_caught_up() {
    let c = content();
    let mut hub = party(&c, 2, 9);
    into_combat(&c, &mut hub);
    // the client's internet goes: it hears nothing while the host plays on for it
    hub.offline.push(2);
    hub.node(1).peer_left(2);
    for _ in 0..40 {
        step(&c, &mut hub);
    }
    let heard = hub.node(2).heard;
    hub.offline.clear();
    // back again: the same machine says hello, and is handed the run as it stands
    hub.node(2).rejoin();
    hub.pump(&c);
    assert!(hub.node(2).heard > heard);
    assert!(hub.node(1).lobby.players[1].connected);
    assert_in_sync(&hub);
}

#[test]
fn a_heartbeat_catches_up_a_client_that_missed_actions() {
    let c = content();
    let mut hub = party(&c, 2, 4);
    into_combat(&c, &mut hub);
    hub.offline.push(2);
    for _ in 0..10 {
        step(&c, &mut hub);
    }
    hub.offline.clear();
    hub.node(1).heartbeat();
    hub.pump(&c);
    assert_in_sync(&hub);
}

#[test]
fn a_saved_party_is_picked_up_and_its_players_take_their_seats_again() {
    let c = content();
    let mut hub = party(&c, 2, 5);
    into_combat(&c, &mut hub);
    let (lobby, n, run) = { let h = hub.node(1); (h.lobby.clone(), h.n, h.state.clone().unwrap()) };
    // a new evening: the host opens the save on another address, the friend comes on yet another
    let mut again = Hub::default();
    again.nodes.push(Node::resume_party(10, "host", lobby, n, run.clone()));
    assert!(!again.node(10).lobby.players[1].connected, "the friend's seat waits");
    again.nodes.push(Node::join(20, "friend", 10));
    again.pump(&c);
    assert_eq!(again.node(20).my_slot(), Some(1));
    assert_eq!(again.node(20).state.as_ref().unwrap().hash64(), run.hash64());
}

#[test]
fn a_player_on_another_build_is_not_let_in() {
    let c = content();
    let mut host = Node::host(1, "host");
    host.handle(&c, 2, Msg::Hello { name: "guest".into(), build: u64::MAX });
    assert_eq!(host.lobby.players.len(), 1, "another build took a seat");
    let mut guest = Node::join(2, "guest", 1);
    guest.handle(&c, 1, Msg::OtherBuild);
    assert!(guest.take_notes().contains(&Note::OtherBuild));
}
