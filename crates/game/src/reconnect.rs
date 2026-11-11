//! A party over the network outlives a dropped connection. The host says where the run stands
//! every two seconds; a client that hears nothing from it for a while calls the connection lost,
//! shows so, and keeps trying to get back in (each transport knows how: see `net` and `lan`),
//! for two minutes. Got back, the host hands it its seat and the run as it now stands. The host,
//! for its part, shows who has dropped out while the autopilot plays for them.

use crate::l10n::L10n;
use crate::net::{Net, NetStatus};
use crate::theme::{self, Fonts};
use crate::AppState;
use bevy::prelude::*;

/// Seconds between the host's word on where the run stands.
const BEAT: f32 = 2.0;
/// Seconds of silence from the host before a client calls the connection lost.
const QUIET: f32 = 7.0;
/// Seconds between tries to get back in.
const RETRY: f32 = 4.0;
/// Seconds of trying before the party is given up; the host keeps the run saved.
const GIVE_UP: f32 = 120.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<Watch>().add_systems(Update, (watch.run_if(resource_exists::<Net>), banner).chain());
}

#[derive(Resource, Default)]
struct Watch {
    heard: u64,
    quiet: f32,
    beat: f32,
    retry: f32,
}

/// The line across the top that says the connection is being got back, or who is away.
#[derive(Component)]
struct Banner;

fn watch(mut commands: Commands, time: Res<Time>, mut net: ResMut<Net>, mut status: ResMut<NetStatus>, mut w: ResMut<Watch>, steam: Option<Res<crate::steam::Steam>>, mut next: ResMut<NextState<AppState>>) {
    let dt = time.delta_secs();
    // an offline session (alone, or with bots in this process) has no line to lose
    let networked = net.lobby.is_some() || net.code == "LAN";
    if !networked {
        status.lost = None;
        return;
    }
    if net.node.is_host() {
        status.lost = None;
        w.beat -= dt;
        if w.beat <= 0.0 {
            w.beat = BEAT;
            net.node.heartbeat();
        }
        return;
    }
    // before the run starts there is nothing to keep hearing about
    if net.node.state.is_none() {
        w.heard = net.node.heard;
        w.quiet = 0.0;
        return;
    }
    if net.node.heard != w.heard {
        w.heard = net.node.heard;
        w.quiet = 0.0;
        if let Some(gone) = status.lost.take() {
            info!("connection to the host back after {gone:.0} s");
            net.node.rejoin();
        }
        return;
    }
    w.quiet += dt;
    if status.lost.is_none() && w.quiet >= QUIET {
        warn!("nothing heard from the host for {QUIET} s: the connection is lost, trying to get back in");
        status.lost = Some(0.0);
        w.retry = 0.0;
    }
    let Some(lost) = status.lost.as_mut() else { return };
    *lost += dt;
    if *lost >= GIVE_UP {
        warn!("could not get back to the party in {GIVE_UP} s; leaving it (the host keeps the run saved)");
        status.lost = None;
        status.error = Some("coop-error-lost".into());
        crate::net::leave(&mut commands, steam.as_deref(), &net);
        next.set(AppState::Coop);
        return;
    }
    w.retry -= dt;
    if w.retry <= 0.0 {
        w.retry = RETRY;
        status.redial = true;
        // the line may still be up and only quiet: a hello costs nothing
        net.node.rejoin();
    }
}

fn banner(mut commands: Commands, net: Option<Res<Net>>, status: Res<NetStatus>, l10n: Res<L10n>, fonts: Res<Fonts>, stage: Single<Entity, With<crate::Stage>>, mut line: Query<(&mut Visibility, &mut Text), With<Banner>>) {
    let text = match (&net, status.lost) {
        (Some(_), Some(lost)) => Some(l10n.tf("coop-reconnecting", &[("s", ((GIVE_UP - lost).max(0.0) as i32).into())])),
        (Some(net), None) if net.node.is_host() && net.node.state.is_some() && net.locals.is_empty() => {
            let away: Vec<(String, f32)> = net.node.lobby.players.iter().enumerate().filter(|(_, p)| !p.connected).map(|(i, p)| (p.name.clone(), (nav_net::RECONNECT_GRACE_PERIOD - net.node.disconnect_timer[i]).max(0.0))).collect();
            (!away.is_empty()).then(|| {
                let who: Vec<String> = away.iter().map(|(w, _)| w.clone()).collect();
                let min_s = away.iter().map(|(_, s)| *s as i32).min().unwrap_or(0);
                l10n.tf("coop-player-away", &[("who", who.join(", ").into()), ("s", min_s.into())])
            })
        }
        _ => None,
    };
    let Ok((mut vis, mut shown)) = line.single_mut() else {
        commands.spawn((
            Banner,
            Node { position_type: PositionType::Absolute, top: px(72), width: px(crate::STAGE_W), justify_content: JustifyContent::Center, ..default() },
            theme::text("", &fonts.body, 26.0, theme::CANDLE),
            TextLayout::justify(Justify::Center),
            TextShadow { offset: Vec2::new(2.0, 2.0), color: Color::BLACK },
            Visibility::Hidden,
            GlobalZIndex(400),
            ChildOf(*stage),
        ));
        return;
    };
    let want = if text.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    vis.set_if_neq(want);
    if let Some(t) = text {
        if shown.0 != t {
            shown.0 = t;
        }
    }
}
