//! Pencil marks on the path map, as in Slay the Spire 2: the right mouse button held down draws a
//! line on the sheet, or, when the eraser is the tool (or Shift is held), rubs out the marks where a
//! broad round eraser is drawn over them, cutting a stroke where it passes; the little tray at the left picks the tool and wipes this player's own marks,
//! as Delete does. Marks are shared with the party, kept until the act ends, and never press a
//! node: the nodes take the left button only.

use crate::l10n::L10n;
use crate::map_party::{MapScroll, MapSheet, SHEET_H};
use crate::net::{Net, NetStatus};
use nav_net::{InkChange, Note, To};
use std::collections::BTreeMap;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage};
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

pub fn plugin(app: &mut App) {
    app.init_resource::<MapInk>()
        .add_systems(Update, sync_ink)
        .add_systems(Update, (redraw_ink, tray, light_tray, draw_ink).chain().after(sync_ink).run_if(in_state(AppState::Map)))
        // the map looked at in a fight (see `run_ui::MapPeek`) is drawn on as well
        .add_systems(Update, (redraw_ink, tray, light_tray, draw_ink).chain().after(sync_ink).run_if(in_state(AppState::Battle)));
}

/// How far everything of the pencil is lifted: in a fight the map is laid over the whole fight,
/// and its marks and tray with it.
fn lift(state: &Option<Res<State<AppState>>>) -> i32 {
    if state.as_ref().is_some_and(|s| *s.get() == AppState::Battle) { crate::run_ui::PEEK_Z } else { 0 }
}

/// What the right button does on the map.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Tool {
    #[default]
    Pencil,
    Eraser,
}

/// Strokes in sheet coordinates, and the act they were drawn in.
#[derive(Resource, Default)]
pub struct MapInk {
    act: u8,
    seed: Option<u64>,
    strokes: Vec<Vec<Vec2>>,
    others: BTreeMap<u8, Vec<Vec<Vec2>>>,
    pending: Vec<InkChange>,
    remote_dirty: bool,
    /// A stroke is being drawn now.
    drawing: bool,
    /// Where the eraser was last frame, while it is rubbing: it rubs the whole way between.
    rubbed_from: Option<Vec2>,
    pub tool: Tool,
    /// This player's seat: their marks are drawn in its colour (see `theme::marker_color`).
    seat: Option<u8>,
}

impl MapInk {
    /// The colour this player's own marks are drawn in: their seat's, as everyone else sees them.
    fn own_color(&self) -> Color {
        self.seat.map_or(PENCIL, theme::marker_color)
    }
}

/// One piece of a pencil stroke, child of the sheet.
#[derive(Component)]
struct InkDot;

#[derive(Component)]
struct RemoteInk;

/// The eraser's round outline under the pointer while it rubs.
#[derive(Component)]
struct EraserRing;

/// The tray of pencil tools, and the buttons that pick one.
#[derive(Component)]
struct InkTray;
#[derive(Component)]
struct ToolButton(Tool);

/// Pencil grey, a little warm, so it reads on parchment and is not taken for the map's own ink.
/// Opaque: the pieces of a stroke overlap at their ends, and see-through they would show there
/// as a row of darker beads.
const PENCIL: Color = Color::srgb(0.33, 0.29, 0.37);
/// The line's thickness.
const DOT: f32 = 6.0;
/// How far apart the points of a stroke are kept; the line is drawn whole between them.
const STEP: f32 = 6.0;
/// The eraser's reach from the pointer: it is ten times the pencil's breadth across.
const RUB: f32 = DOT * 5.0;
const EDGE: Color = Color::srgb(0.42, 0.33, 0.22);
/// Where the marks lie among everything drawn: over the map screen, under its tray.
const INK_Z: i32 = 5;
/// How far below the top bar the tray begins: under the party's panel, which is as tall as four
/// players make it.
const TRAY_DROP: f32 = 210.0;

/// A piece of line from `a` to `b`: a bar with round ends turned along the way, so the pieces of a
/// stroke overlap into one unbroken line (a lone point is a round dot).
fn piece(commands: &mut Commands, sheet: Entity, a: Vec2, b: Vec2, z: i32, hue: Color) -> Entity {
    let d = b - a;
    let mid = (a + b) / 2.0;
    let w = d.length() + DOT;
    commands.spawn((
        InkDot,
        Node { position_type: PositionType::Absolute, left: px(mid.x - w / 2.0), top: px(mid.y - DOT / 2.0), width: px(w), height: px(DOT), border_radius: BorderRadius::MAX, ..default() },
        UiTransform { rotation: Rot2::radians(d.y.atan2(d.x)), ..UiTransform::IDENTITY },
        BackgroundColor(hue),
        GlobalZIndex(z),
        Pickable::IGNORE,
        ChildOf(sheet),
    )).id()
}

/// How far `p` is from the segment `a`-`b`.
fn off(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t).distance(p)
}

/// The eraser drawn from `from` to `to` rubs out what of the strokes lies under it: a stroke it
/// crosses is cut there and the pieces either side stay. Whether anything was rubbed out.
fn rub(strokes: &mut Vec<Vec<Vec2>>, from: Vec2, to: Vec2) -> bool {
    let mut any = false;
    let mut kept = Vec::with_capacity(strokes.len());
    for stroke in strokes.drain(..) {
        // the points close enough together that the eraser cannot slip between them
        let mut fine = Vec::with_capacity(stroke.len());
        for (k, p) in stroke.iter().enumerate() {
            if let Some(prev) = k.checked_sub(1).map(|k| stroke[k]) {
                let n = (prev.distance(*p) / (STEP / 2.0)).ceil().max(1.0) as usize;
                fine.extend((1..n).map(|i| prev.lerp(*p, i as f32 / n as f32)));
            }
            fine.push(*p);
        }
        if !fine.iter().any(|p| off(*p, from, to) <= RUB) {
            kept.push(stroke);
            continue;
        }
        any = true;
        let mut piece: Vec<Vec2> = Vec::new();
        for p in fine {
            if off(p, from, to) <= RUB {
                // a crumb left at the eraser's edge is rubbed out with the rest
                if piece.len() > 1 {
                    kept.push(std::mem::take(&mut piece));
                }
                piece.clear();
            } else {
                piece.push(p);
            }
        }
        if piece.len() > 1 {
            kept.push(piece);
        }
    }
    *strokes = kept;
    any
}

fn lay_stroke(commands: &mut Commands, sheet: Entity, stroke: &[Vec2], z: i32, hue: Color) {
    match stroke {
        [] => {}
        [p] => { piece(commands, sheet, *p, *p, z, hue); }
        _ => {
            for w in stroke.windows(2) {
                piece(commands, sheet, w[0], w[1], z, hue);
            }
        }
    }
}

/// The map is drawn anew whenever the run changes: the marks are put back on each new sheet, and
/// forgotten when a new act begins.
fn redraw_ink(mut commands: Commands, mut ink: ResMut<MapInk>, sheets: Query<(Entity, Ref<MapSheet>)>, dots: Query<Entity, With<RemoteInk>>, state: Option<Res<State<AppState>>>) {
    let z = lift(&state) + INK_Z;
    let own = ink.own_color();
    for (sheet, marker) in &sheets {
        if marker.is_added() {
            for stroke in &ink.strokes {
                lay_stroke(&mut commands, sheet, stroke, z, own);
            }
        }
        if marker.is_added() || ink.remote_dirty {
            for e in &dots {
                commands.entity(e).despawn();
            }
            for (seat, strokes) in &ink.others {
                for stroke in strokes {
                    for (a, b) in stroke.first().map(|p| (*p, *p)).into_iter().chain(stroke.windows(2).map(|w| (w[0], w[1]))) {
                        let e = piece(&mut commands, sheet, a, b, z, theme::marker_color(*seat));
                        commands.entity(e).insert(RemoteInk);
                    }
                }
            }
            ink.remote_dirty = false;
        }
    }
}

fn snapshot(ink: &MapInk) -> InkChange {
    InkChange::Snapshot(ink.strokes.iter().map(|s| s.iter().map(|p| p.to_array()).collect()).collect())
}

fn apply_change(strokes: &mut Vec<Vec<Vec2>>, change: InkChange) {
    match change {
        InkChange::Begin(p) => strokes.push(vec![Vec2::from_array(p)]),
        InkChange::Point(p) => {
            if let Some(stroke) = strokes.last_mut() {
                stroke.push(Vec2::from_array(p));
            }
        }
        InkChange::Rub { from, to } => { rub(strokes, Vec2::from_array(from), Vec2::from_array(to)); }
        InkChange::Clear => strokes.clear(),
        InkChange::Snapshot(all) => *strokes = all.into_iter().map(|s| s.into_iter().map(Vec2::from_array).collect()).collect(),
    }
}

/// Receive even with the map closed; sheet coordinates do not depend on either player's scroll.
fn sync_ink(mut ink: ResMut<MapInk>, net: Option<ResMut<Net>>, mut status: ResMut<NetStatus>) {
    let Some(mut net) = net else {
        *ink = MapInk::default();
        status.ink.clear();
        status.ink_resync = false;
        return;
    };
    let Some(run) = net.node.state.as_ref() else { return };
    let (seed, act) = (run.seed, run.act);
    let me = net.node.my_slot();
    let changed = ink.seed != Some(seed) || ink.act != act;
    if changed {
        *ink = MapInk { seed: Some(seed), act, ..default() };
    }
    if ink.seat != me {
        ink.seat = me;
    }
    // Flush earlier edits before answering a snapshot request so reliable ordering cannot
    // append those same points again after the snapshot.
    for change in std::mem::take(&mut ink.pending) {
        net.node.map_ink(To::All, change);
    }
    let resync = std::mem::take(&mut status.ink_resync);
    if changed || resync {
        net.node.request_map_ink();
        net.node.map_ink(To::All, snapshot(&ink));
    }
    for note in std::mem::take(&mut status.ink) {
        match note {
            Note::MapInk { from, seed: s, act: a, change } if s == seed && a == act => {
                apply_change(ink.others.entry(from).or_default(), change);
                ink.remote_dirty = true;
            }
            Note::MapInkRequest { from, seed: s, act: a } if s == seed && a == act => {
                net.node.map_ink(To::One(from), snapshot(&ink));
            }
            _ => {}
        }
    }
}

/// A tool's button: its picture (`assets/map/tool_<name>`) beside its name.
fn tray_button(commands: &mut ChildSpawnerCommands, label: String, icon: Option<Handle<Image>>, fonts: &Fonts) -> Entity {
    let mut button = commands.spawn((
        Button,
        Node { width: px(240), padding: UiRect::axes(px(10), px(6)), border: UiRect::all(px(3)), column_gap: px(10), align_items: AlignItems::Center, ..default() },
        BackgroundColor(theme::PEAT),
        BorderColor::all(EDGE),
    ));
    button.with_children(|b| {
        if let Some(icon) = icon {
            b.spawn((ImageNode::new(icon), Node { width: px(44), height: px(44), flex_shrink: 0.0, ..default() }, Pickable::IGNORE));
        }
        b.spawn(theme::text(label, &fonts.title, 24.0, theme::BONE));
    });
    button.id()
}

fn wipe(commands: &mut Commands, ink: &mut MapInk, dots: &Query<Entity, (With<InkDot>, Without<RemoteInk>)>) {
    ink.strokes.clear();
    ink.pending.push(InkChange::Clear);
    ink.drawing = false;
    ink.rubbed_from = None;
    for e in dots {
        commands.entity(e).despawn();
    }
}

/// The tray lies at the left of the map, beside the parchment, once per map screen; in a fight it
/// belongs to the map looked at there, and goes when that is rolled up.
#[allow(clippy::too_many_arguments)]
fn tray(
    mut commands: Commands,
    trays: Query<(), With<InkTray>>,
    stage: Single<Entity, With<Stage>>,
    peek: Query<Entity, With<crate::run_ui::MapPeek>>,
    state: Option<Res<State<AppState>>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<crate::art::Art>,
    assets: Res<AssetServer>,
) {
    if !trays.is_empty() {
        return;
    }
    let up = lift(&state);
    let parent = if up == 0 {
        *stage
    } else {
        let Ok(peek) = peek.single() else { return };
        peek
    };
    commands
        .spawn((
            InkTray,
            Node { position_type: PositionType::Absolute, left: px(28), top: px(crate::between::BAR_H + TRAY_DROP), flex_direction: FlexDirection::Column, row_gap: px(8), ..default() },
            GlobalZIndex(up + 40),
            DespawnOnExit(AppState::Map),
            ChildOf(parent),
        ))
        .with_children(|p| {
            p.spawn(theme::text(l10n.t("map-ink-title"), &fonts.body, 22.0, theme::BONE));
            let icon = |name: &str| art.image(&assets, &format!("map/tool_{name}"));
            let pencil = tray_button(p, l10n.t("map-ink-pencil"), icon("pencil"), &fonts);
            p.commands().entity(pencil).insert((ToolButton(Tool::Pencil), observe(|_: On<Activate>, mut ink: ResMut<MapInk>| ink.tool = Tool::Pencil)));
            let eraser = tray_button(p, l10n.t("map-ink-eraser"), icon("eraser"), &fonts);
            p.commands().entity(eraser).insert((ToolButton(Tool::Eraser), observe(|_: On<Activate>, mut ink: ResMut<MapInk>| ink.tool = Tool::Eraser)));
            let clear = tray_button(p, l10n.t("map-ink-clear"), icon("wipe"), &fonts);
            p.commands().entity(clear).insert(observe(|_: On<Activate>, mut commands: Commands, mut ink: ResMut<MapInk>, dots: Query<Entity, (With<InkDot>, Without<RemoteInk>)>| wipe(&mut commands, &mut ink, &dots)));
        });
}

/// The tool in hand shows lit.
fn light_tray(ink: Res<MapInk>, mut buttons: Query<(&ToolButton, &mut BorderColor, &mut BackgroundColor)>) {
    for (b, mut border, mut bg) in &mut buttons {
        let on = b.0 == ink.tool;
        *border = BorderColor::all(if on { theme::CANDLE } else { EDGE });
        bg.0 = if on { Color::srgb(0.30, 0.22, 0.14) } else { theme::PEAT };
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_ink(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window>,
    scale: Res<UiScale>,
    scroll: Res<MapScroll>,
    mut moves: MessageReader<bevy::window::WindowEvent>,
    mut cursor: Local<Option<Vec2>>,
    sheet: Query<Entity, With<MapSheet>>,
    dots: Query<Entity, (With<InkDot>, Without<RemoteInk>)>,
    rings: Query<Entity, With<EraserRing>>,
    mut ink: ResMut<MapInk>,
    state: Option<Res<State<AppState>>>,
) {
    let up = lift(&state);
    let ink_z = up + INK_Z;
    let own = ink.own_color();
    let ring = |commands: &mut Commands, at: Option<(Entity, Vec2)>| {
        for e in &rings {
            commands.entity(e).despawn();
        }
        if let Some((sheet, at)) = at {
            commands.spawn((
                EraserRing,
                Node { position_type: PositionType::Absolute, left: px(at.x - RUB), top: px(at.y - RUB), width: px(RUB * 2.0), height: px(RUB * 2.0), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, ..default() },
                BorderColor::all(PENCIL),
                BackgroundColor(theme::BONE.with_alpha(0.25)),
                GlobalZIndex(ink_z + 1),
                Pickable::IGNORE,
                ChildOf(sheet),
            ));
        }
    };
    // where the pointer is, as the window's own events tell it (as the party's pointers are read)
    for ev in moves.read() {
        if let bevy::window::WindowEvent::CursorMoved(m) = ev {
            *cursor = Some(m.position);
        }
    }
    let Ok(sheet) = sheet.single() else { return };
    if keys.just_pressed(KeyCode::Delete) && !ink.strokes.is_empty() {
        wipe(&mut commands, &mut ink, &dots);
        return;
    }
    if !mouse.pressed(MouseButton::Right) {
        ink.drawing = false;
        if ink.rubbed_from.take().is_some() {
            ring(&mut commands, None);
        }
        return;
    }
    let Some(pos) = window.cursor_position().or_else(crate::touch::finger).or(*cursor) else { return };
    let stage = crate::stage_point(&window, scale.0, pos);
    // below the top bar only, where the sheet shows
    if stage.y < crate::between::BAR_H {
        return;
    }
    let at = stage - scroll.to_stage(Vec2::ZERO);
    if at.y < 0.0 || at.y > SHEET_H {
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if shift || ink.tool == Tool::Eraser {
        let from = ink.rubbed_from.replace(at).unwrap_or(at);
        ring(&mut commands, Some((sheet, at)));
        if rub(&mut ink.strokes, from, at) {
            ink.pending.push(InkChange::Rub { from: from.to_array(), to: at.to_array() });
            for e in &dots {
                commands.entity(e).despawn();
            }
            for stroke in &ink.strokes {
                lay_stroke(&mut commands, sheet, stroke, ink_z, own);
            }
        }
        ink.drawing = false;
        return;
    }
    if ink.rubbed_from.take().is_some() {
        ring(&mut commands, None);
    }
    if !ink.drawing || mouse.just_pressed(MouseButton::Right) {
        ink.drawing = true;
        ink.strokes.push(vec![at]);
        ink.pending.push(InkChange::Begin(at.to_array()));
        piece(&mut commands, sheet, at, at, ink_z, own);
        return;
    }
    let Some(stroke) = ink.strokes.last_mut() else { return };
    let last = *stroke.last().unwrap_or(&at);
    if last.distance(at) < STEP {
        return;
    }
    // the piece from the last point reaches the pointer whole, however quick the stroke
    stroke.push(at);
    ink.pending.push(InkChange::Point(at.to_array()));
    piece(&mut commands, sheet, last, at, ink_z, own);
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec2;

    #[test]
    fn clearing_my_marks_preserves_the_other_players_marks_on_the_sheet() {
        use super::*;
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<MapInk>().add_systems(Update, redraw_ink);
        app.world_mut().spawn(MapSheet);
        {
            let mut ink = app.world_mut().resource_mut::<MapInk>();
            ink.strokes.push(vec![Vec2::new(10.0, 20.0)]);
            ink.others.insert(1, vec![vec![Vec2::new(30.0, 40.0)]]);
        }
        app.update();
        app.world_mut().run_system_once(|mut commands: Commands, mut ink: ResMut<MapInk>, dots: Query<Entity, (With<InkDot>, Without<RemoteInk>)>| {
            wipe(&mut commands, &mut ink, &dots);
        }).unwrap();
        assert_eq!(app.world_mut().query_filtered::<Entity, With<RemoteInk>>().iter(app.world()).count(), 1);
        assert_eq!(app.world_mut().query_filtered::<Entity, (With<InkDot>, Without<RemoteInk>)>().iter(app.world()).count(), 0);
        assert_eq!(app.world().resource::<MapInk>().pending, vec![InkChange::Clear]);
    }

    #[test]
    fn received_strokes_and_erasing_match_the_local_drawing() {
        use nav_net::InkChange;
        let mut local = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(400.0, 0.0)]];
        let mut remote = Vec::new();
        super::apply_change(&mut remote, InkChange::Begin([0.0, 0.0]));
        super::apply_change(&mut remote, InkChange::Point([400.0, 0.0]));
        assert_eq!(remote, local);
        super::rub(&mut local, Vec2::new(200.0, 0.0), Vec2::new(200.0, 20.0));
        super::apply_change(&mut remote, InkChange::Rub { from: [200.0, 0.0], to: [200.0, 20.0] });
        assert_eq!(remote, local);
        let ink = super::MapInk { strokes: local.clone(), ..Default::default() };
        // A replacement is idempotent, even if some of its points arrived before the snapshot.
        super::apply_change(&mut remote, super::snapshot(&ink));
        super::apply_change(&mut remote, super::snapshot(&ink));
        assert_eq!(remote, local);
        super::apply_change(&mut remote, InkChange::Clear);
        assert!(remote.is_empty());
        assert!(!local.is_empty());
    }

    #[test]
    fn the_eraser_rubs_out_only_what_it_passes_over() {
        let mut strokes = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(400.0, 0.0)], vec![Vec2::new(500.0, 500.0), Vec2::new(520.0, 500.0)]];
        assert!(super::rub(&mut strokes, Vec2::new(200.0, 10.0), Vec2::new(200.0, 10.0)));
        // the long line is cut in two where the eraser lay, the far stroke is left alone
        assert_eq!(strokes.len(), 3);
        assert!(strokes[0].iter().all(|p| p.x < 200.0 - super::RUB + 1.0));
        assert!(strokes[1].iter().all(|p| p.x > 200.0 + super::RUB - 1.0));
        assert_eq!(strokes[2], vec![Vec2::new(500.0, 500.0), Vec2::new(520.0, 500.0)]);
    }

    #[test]
    fn the_eraser_rubs_the_whole_way_it_is_drawn() {
        let mut strokes = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(400.0, 0.0)]];
        // drawn quickly along the line: nothing of it is left between
        assert!(super::rub(&mut strokes, Vec2::new(-10.0, 0.0), Vec2::new(410.0, 0.0)));
        assert!(strokes.is_empty());
    }

    #[test]
    fn the_eraser_far_off_leaves_everything() {
        let mut strokes = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(400.0, 0.0)]];
        assert!(!super::rub(&mut strokes, Vec2::new(200.0, 80.0), Vec2::new(200.0, 80.0)));
        assert_eq!(strokes, vec![vec![Vec2::new(0.0, 0.0), Vec2::new(400.0, 0.0)]]);
    }
}
