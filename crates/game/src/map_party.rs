//! The party over the path map, as in Slay the Spire 2: every other player's cursor drifts over
//! it in their hero's colour; a vote rings its node in the voter's colour; one who has chosen
//! and clicks their node again hurries the rest (it throbs for them, with a knock); and a split
//! vote is settled by a lot played out before everyone's eyes, the ring running over the voted
//! nodes, a node once per vote, slowing until it stops on the one chosen.

use crate::audio::{Mixer, Sfx};
use crate::net::{Net, NetStatus};
use crate::theme::{self, Fonts};
use crate::{AppState, Stage as Canvas};
use bevy::prelude::*;
use nav_core::RunEvent;

pub fn plugin(app: &mut App) {
    app.init_resource::<MapSpots>()
        .init_resource::<MapScroll>()
        .add_systems(OnEnter(AppState::Map), |mut scroll: ResMut<MapScroll>| scroll.fresh = true)
        .add_systems(Update, (roll_sheet, send_pointer, show_pointers, show_votes, play_lot, pulse).chain().run_if(in_state(AppState::Map)))
        // the map looked at in a fight rolls under the wheel as well
        .add_systems(Update, roll_sheet.run_if(in_state(AppState::Battle)))
        .add_systems(OnExit(AppState::Map), |mut commands: Commands, marks: Query<Entity, With<MapOverlay>>| {
            for e in &marks {
                commands.entity(e).despawn();
            }
        });
}

/// Where each node the party may step to sits on the map sheet, by choice number; the map screen
/// writes it as it draws.
#[derive(Resource, Default)]
pub struct MapSpots(pub Vec<Vec2>);

/// Height of the map sheet, far taller than the screen: the map is rolled through by the wheel.
pub const SHEET_H: f32 = 2000.0;

/// The tall sheet the map is drawn on; its own coordinates are what the players tell each other.
#[derive(Component)]
pub struct MapSheet;

/// How far down the sheet the top of the screen (below the bar) is, and where it is rolling to.
/// `fresh` asks the map to roll to the party when it is next drawn.
#[derive(Resource, Default)]
pub struct MapScroll {
    pub y: f32,
    pub target: f32,
    pub fresh: bool,
}

impl MapScroll {
    /// From a point on the sheet to the same point on the stage.
    pub fn to_stage(&self, at: Vec2) -> Vec2 {
        at + Vec2::new(0.0, crate::between::BAR_H - self.y)
    }
}

/// The wheel rolls the sheet; it glides to where it was sent.
fn roll_sheet(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut scroll: ResMut<MapScroll>,
    time: Res<Time>,
    mut sheets: Query<&mut Node, With<MapSheet>>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    let most = SHEET_H - (crate::STAGE_H - crate::between::BAR_H);
    for w in wheel.read() {
        let step = match w.unit {
            MouseScrollUnit::Line => w.y * 110.0,
            MouseScrollUnit::Pixel => w.y,
        };
        scroll.target = (scroll.target - step).clamp(0.0, most);
    }
    let k = (time.delta_secs() * 12.0).min(1.0);
    let y = scroll.y + (scroll.target - scroll.y) * k;
    if (y - scroll.y).abs() > 0.01 {
        scroll.y = y;
    }
    for mut node in &mut sheets {
        let top = px(-scroll.y);
        if node.top != top {
            node.top = top;
        }
    }
}

/// A lot being drawn: the votes as tickets (each the choice it was for), the choice it fell to,
/// and how far it has run. While it runs the map stays up.
#[derive(Resource)]
pub struct MapLot {
    tickets: Vec<u8>,
    /// The ring's stops, as indices into `tickets`, the last on the chosen one.
    stops: Vec<usize>,
    step: usize,
    wait: f32,
    /// Seconds left to linger on the chosen node once the ring has stopped.
    after: f32,
    /// Where the ring stands on the sheet.
    at: Option<Vec2>,
}

/// Everything this module puts over the map, gone when the map is left.
#[derive(Component)]
struct MapOverlay;

#[derive(Component)]
struct Cursor {
    seat: u8,
    to: Vec2,
    at: Vec2,
    quiet: f32,
}

#[derive(Component)]
struct LotRing;

#[derive(Component)]
struct Pulse {
    /// On the map sheet.
    at: Vec2,
    t: f32,
    life: f32,
    from: f32,
    to: f32,
    color: Color,
}

fn ring(commands: &mut Commands, canvas: Entity, at: Vec2, color: Color, from: f32, to: f32, life: f32) {
    commands.spawn((
        MapOverlay,
        Pulse { at, t: 0.0, life, from, to, color },
        Node { position_type: PositionType::Absolute, border: UiRect::all(px(5)), border_radius: BorderRadius::MAX, ..default() },
        BorderColor::all(color),
        GlobalZIndex(150),
        Pickable::IGNORE,
        ChildOf(canvas),
    ));
}

fn seat_color(net: &Net, seat: u8) -> Color {
    net.node.state.as_ref().and_then(|run| run.players.get(seat as usize)).map(|p| theme::hero_color(&p.hero)).unwrap_or(theme::BONE)
}

/// Tells the others where this player's cursor is, a dozen times a second while it moves.
/// Read from window events, as picking does, so the dev robot's mouse is seen too.
#[allow(clippy::type_complexity)]
fn send_pointer(
    mut net: Option<ResMut<Net>>,
    mut moves: MessageReader<bevy::window::WindowEvent>,
    window: Single<&Window>,
    scale: Res<UiScale>,
    scroll: Res<MapScroll>,
    time: Res<Time>,
    mut last: Local<(f32, Vec2, Option<Vec2>)>,
) {
    for ev in moves.read() {
        if let bevy::window::WindowEvent::CursorMoved(m) = ev {
            last.2 = Some(m.position);
        }
    }
    let Some(net) = net.as_mut() else { return };
    if net.node.state.as_ref().is_none_or(|r| r.players.len() < 2) {
        return;
    }
    last.0 += time.delta_secs();
    let Some(pos) = last.2 else { return };
    let at = crate::stage_point(&window, scale.0, pos) - scroll.to_stage(Vec2::ZERO);
    if last.0 >= 1.0 / 12.0 && at.distance(last.1) > 2.0 {
        last.0 = 0.0;
        last.1 = at;
        net.node.map_pointer(at.to_array());
    }
}

fn show_pointers(
    mut commands: Commands,
    net: Option<Res<Net>>,
    mut status: ResMut<NetStatus>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<crate::l10n::L10n>,
    time: Res<Time>,
    scroll: Res<MapScroll>,
    art: Res<crate::art::Art>,
    assets: Res<AssetServer>,
    mut cursors: Query<(Entity, &mut Cursor, &mut Node, &mut Visibility)>,
) {
    let Some(net) = net else { return };
    let dt = time.delta_secs();
    for (seat, at) in std::mem::take(&mut status.pointers) {
        let to = Vec2::from_array(at);
        match cursors.iter_mut().find(|(_, c, ..)| c.seat == seat) {
            Some((_, mut c, ..)) => {
                c.to = to;
                c.quiet = 0.0;
            }
            None => {
                let color = seat_color(&net, seat);
                let name = net.node.state.as_ref().and_then(|r| r.players.get(seat as usize)).map(|p| l10n.t(&format!("hero-{}", p.hero))).unwrap_or_default();
                commands
                    .spawn((
                        MapOverlay,
                        Cursor { seat, to, at: to, quiet: 0.0 },
                        Node { position_type: PositionType::Absolute, align_items: AlignItems::Center, column_gap: px(6), ..default() },
                        GlobalZIndex(160),
                        Pickable::IGNORE,
                        ChildOf(*canvas),
                    ))
                    .with_children(|c| {
                        // the same little face as their token on the map
                        let hero = net.node.state.as_ref().and_then(|r| r.players.get(seat as usize)).map(|p| p.hero.clone()).unwrap_or_default();
                        let mut dot = c.spawn((crate::run_ui::token_node(30.0), BoxShadow::new(color.with_alpha(0.7), px(0), px(0), px(2), px(10))));
                        crate::run_ui::hero_token(&mut dot, &hero, seat, &art, &assets);
                        c.spawn((theme::text(name, &fonts.body, 18.0, color.lighter(0.2)), TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK }));
                    });
            }
        }
    }
    for (_, mut c, mut node, mut vis) in &mut cursors {
        c.quiet += dt;
        c.at = c.at.lerp(c.to, (dt * 14.0).min(1.0));
        let at = scroll.to_stage(c.at);
        node.left = px(at.x - 15.0);
        node.top = px(at.y - 15.0);
        // a cursor not heard of for a while has left the map, or its player the game
        vis.set_if_neq(if c.quiet < 3.0 { Visibility::Inherited } else { Visibility::Hidden });
    }
}

#[allow(clippy::too_many_arguments)]
fn show_votes(
    mut commands: Commands,
    net: Option<Res<Net>>,
    mut status: ResMut<NetStatus>,
    spots: Res<MapSpots>,
    canvas: Single<Entity, With<Canvas>>,
    sfx: Res<Sfx>,
    mixer: Res<Mixer>,
) {
    let Some(net) = net else { return };
    for ev in std::mem::take(&mut status.map) {
        match ev {
            RunEvent::Voted { player, choice } => {
                if let Some(at) = spots.0.get(choice as usize) {
                    ring(&mut commands, *canvas, *at, seat_color(&net, player), 40.0, 86.0, 0.6);
                }
                sfx.play(&mut commands, "map_vote", &mixer, 1.0);
            }
            RunEvent::Lot { tied, chosen } if !tied.is_empty() => {
                // three rounds of the tickets and a little more, ending on a ticket of the chosen
                let n = tied.len();
                let last = tied.iter().rposition(|t| *t == chosen).unwrap_or(0);
                let stops: Vec<usize> = (0..n * 3 + last + 1).map(|k| k % n).collect();
                commands.insert_resource(MapLot { tickets: tied, stops, step: 0, wait: 0.0, after: 1.0, at: None });
                commands.spawn((
                    MapOverlay,
                    LotRing,
                    Node { position_type: PositionType::Absolute, width: px(78), height: px(78), border: UiRect::all(px(7)), border_radius: BorderRadius::MAX, ..default() },
                    BorderColor::all(theme::CANDLE),
                    BoxShadow::new(theme::CANDLE.with_alpha(0.8), px(0), px(0), px(4), px(18)),
                    Visibility::Hidden,
                    GlobalZIndex(170),
                    Pickable::IGNORE,
                    ChildOf(*canvas),
                ));
            }
            _ => {}
        }
    }
    for (from, node) in std::mem::take(&mut status.nudges) {
        if let Some(at) = spots.0.get(node as usize) {
            let color = seat_color(&net, from);
            ring(&mut commands, *canvas, *at, color, 44.0, 120.0, 0.5);
            ring(&mut commands, *canvas, *at, color, 44.0, 90.0, 0.35);
        }
        sfx.play(&mut commands, "map_nudge", &mixer, 1.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn play_lot(
    mut commands: Commands,
    lot: Option<ResMut<MapLot>>,
    spots: Res<MapSpots>,
    time: Res<Time>,
    canvas: Single<Entity, With<Canvas>>,
    scroll: Res<MapScroll>,
    mut rings: Query<(&mut Node, &mut Visibility), With<LotRing>>,
    sfx: Res<Sfx>,
    mixer: Res<Mixer>,
) {
    let Some(mut lot) = lot else { return };
    let dt = time.delta_secs();
    if let Some(at) = lot.at {
        let at = scroll.to_stage(at);
        for (mut node, _) in &mut rings {
            node.left = px(at.x - 39.0);
            node.top = px(at.y - 39.0);
        }
    }
    if lot.step >= lot.stops.len() {
        lot.after -= dt;
        if lot.after <= 0.0 {
            commands.remove_resource::<MapLot>();
        }
        return;
    }
    lot.wait -= dt;
    if lot.wait > 0.0 {
        return;
    }
    let ticket = lot.stops[lot.step];
    let choice = lot.tickets[ticket];
    let Some(at) = spots.0.get(choice as usize).copied() else {
        commands.remove_resource::<MapLot>();
        return;
    };
    lot.at = Some(at);
    let shown = scroll.to_stage(at);
    for (mut node, mut vis) in &mut rings {
        node.left = px(shown.x - 39.0);
        node.top = px(shown.y - 39.0);
        vis.set_if_neq(Visibility::Inherited);
    }
    lot.step += 1;
    let left = lot.stops.len() - lot.step;
    if left == 0 {
        sfx.play(&mut commands, "lot_land", &mixer, 1.0);
        ring(&mut commands, *canvas, at, theme::CANDLE, 60.0, 160.0, 0.7);
    } else {
        sfx.play(&mut commands, "lot_tick", &mixer, 0.9);
    }
    // quick at first, slowing to a crawl over the last few
    lot.wait = 0.07 + 0.5 / (left as f32 + 1.0).powf(1.2);
}

fn pulse(mut commands: Commands, time: Res<Time>, scroll: Res<MapScroll>, mut q: Query<(Entity, &mut Pulse, &mut Node, &mut BorderColor)>) {
    for (e, mut p, mut node, mut edge) in &mut q {
        p.t += time.delta_secs() / p.life;
        if p.t >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let ease = 1.0 - (1.0 - p.t).powi(2);
        let size = p.from + (p.to - p.from) * ease;
        let at = scroll.to_stage(p.at);
        node.left = px(at.x - size / 2.0);
        node.top = px(at.y - size / 2.0);
        node.width = px(size);
        node.height = px(size);
        *edge = BorderColor::all(p.color.with_alpha(1.0 - p.t));
    }
}
