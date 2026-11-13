//! Development-only helpers, compiled in with `--features dev`.
//!
//!   TROPA_SCREEN=coop|village|settings|lobby|run|host  open the party menu, the village, settings, an offline lobby, a run in progress, or a real Steam lobby
//!   TROPA_BOTS=<0..3>           bot companions for `lobby` and `run`
//!   TROPA_HERO=<id>             the local hero for `run`
//!   TROPA_BOT_ME=1              the local seat is played by the autopilot too
//!   TROPA_FORCE=shop|omen|rest|trophy|chest  open `run` straight at that stage
//!   TROPA_SHRINE=<0..2>         pretend the shrine stands at this level (in memory only; never saved by a screenshot run)
//!   TROPA_FIGHT=<id>,<id>       open `run` on a fight with exactly these enemies
//!   TROPA_HERO_HP=<n>[@slot]    the heroes of that fight, or the one in that seat, are down to that HP
//!   TROPA_FOE_STR=<n>           the foes of that fight hit that much harder
//!   TROPA_HEADLESS=<seat>       the player in that seat lies without a head
//!   TROPA_OMEN=<id>             open `run` on that omen
//!   TROPA_HAND=<id>,<id>|all    the heroes of that fight hold these cards, or every card (a text that cannot fit is logged)
//!   TROPA_SEED=<n>              fixed seed for that run
//!   TROPA_POTIONS=<id>,<id>     every player's belt holds these draughts
//!   TROPA_DRIVE=<file>          steps written to this file are played as they come (also `p <png>` a shot, `q` quit)
//!   TROPA_ROBOT=<script>        feed the game mouse input, as the window itself would: steps separated by `;`,
//!                               `w <seconds>`, `m <x> <y>` (glide there, stage pixels), `d` (press), `u` (release),
//!                               `s <lines>` (turn the wheel, positive up the page), `r` (right click),
//!                               `rd` / `ru` (the right button down / up)
//!   TROPA_WINDOW=<w>x<h>        the hidden screenshot window takes this size
//!   TROPA_RESIZE=<s>:<w>x<h>    that many seconds in, the window is made that size, as a player maximising it
//!   TROPA_SHOT=<file.png>       save a screenshot after TROPA_SHOT_AT seconds (default 3) and quit
//!   TROPA_SHOT_COUNT=<n>        instead a series of n frames, TROPA_SHOT_EVERY seconds apart (default 0.1), as <file>_<i>.png
//!   TROPA_STORY=<scene>         open a story scene in the book at once (see story.rs), TROPA_STORY_PAGE=<n> on its n-th page
//!   TROPA_GHOST=1               a second reader's hand turns the book's leaf, as their machine would tell it

use crate::session::Db;
use crate::AppState;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Menu), open_screen).add_systems(Update, (shot, resize_later)).add_systems(First, robot);
}

fn env<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn open_screen(
    mut commands: Commands,
    db: Res<Db>,
    steam: Option<Res<crate::steam::Steam>>,
    inbox: Option<Res<crate::net::Inbox>>,
    mut status: ResMut<crate::net::NetStatus>,
    mut next: ResMut<NextState<AppState>>,
    mut done: Local<bool>,
    mut profile: ResMut<crate::profile::Profile>,
) {
    let Ok(screen) = std::env::var("TROPA_SCREEN") else { return };
    if *done {
        return;
    }
    *done = true;
    if let Some(level) = env::<u8>("TROPA_SHRINE") {
        profile.bypass_change_detection().buildings.insert("shrine".into(), level.min(2));
    }
    let bots = env::<usize>("TROPA_BOTS").unwrap_or(0).min(3);
    match screen.as_str() {
        "coop" => next.set(AppState::Coop),
        "village" => next.set(AppState::Village),
        "chronicle" => next.set(AppState::Chronicle),
        "settings" => next.set(AppState::Settings),
        "lobby" => {
            crate::net::start_offline(&mut commands, &db.0, "dev", bots);
            next.set(AppState::Lobby);
        }
        // straight into a run over the real protocol; the stage router opens the right screen
        "run" => {
            let seed = env("TROPA_SEED").unwrap_or(7);
            let bot_me = std::env::var("TROPA_BOT_ME").is_ok();
            let force = match std::env::var("TROPA_FORCE").as_deref() {
                Ok("shop") => Some(nav_core::NodeKind::Shop),
                Ok("omen") => Some(nav_core::NodeKind::Event),
                Ok("rest") => Some(nav_core::NodeKind::Rest),
                Ok("chest") => Some(nav_core::NodeKind::Chest),
                _ => None,
            };
            crate::net::start_offline_run(&mut commands, &db.0, bots, seed, std::env::var("TROPA_HERO").ok(), bot_me, force);
        }
        "host" => {
            if let (Some(steam), Some(inbox)) = (steam, inbox) {
                crate::net::create_lobby(&steam, &mut status, &inbox);
            }
        }
        _ => {}
    }
}

fn shot(mut commands: Commands, time: Res<Time>, mut taken: Local<Option<f32>>, mut series: Local<u32>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("TROPA_SHOT") else { return };
    let at = env::<f32>("TROPA_SHOT_AT").unwrap_or(3.0);
    let now = time.elapsed_secs();
    // TROPA_SHOT_COUNT=n frames TROPA_SHOT_EVERY seconds apart, saved as <file>_<i>.png, to
    // look at an animation
    let count = env::<u32>("TROPA_SHOT_COUNT").unwrap_or(1).max(1);
    let every = env::<f32>("TROPA_SHOT_EVERY").unwrap_or(0.1);
    if count > 1 && *series < count {
        if now >= at + every * *series as f32 {
            let file = path.replace(".png", &format!("_{:02}.png", *series));
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(file));
            *series += 1;
            if *series == count {
                *taken = Some(now);
            }
        }
        return;
    }
    match *taken {
        None if now >= at => {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            *taken = Some(now);
        }
        // give the capture a moment to reach the disk
        Some(t) if now - t > 1.0 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

/// One step of a mouse script.
enum Step {
    Wait(f32),
    Move(Vec2),
    Down,
    Up,
    /// The right button held down, and let go.
    RightDown,
    RightUp,
    RightClick,
    /// Wheel lines, positive away from the player (up the page).
    Wheel(f32),
    /// A screenshot, to this file.
    Shot(String),
    /// Leave the game.
    Quit,
    /// A key pressed and let go, by its name: space, enter, left, right, esc, m, back.
    Key(String),
    /// Types these characters, one key at a time.
    Type(String),
}

fn parse_steps(text: &str) -> Vec<Step> {
    text.split(';')
        .filter_map(|part| {
            let words: Vec<&str> = part.split_whitespace().collect();
            match words.as_slice() {
                ["w", t] => t.parse().ok().map(Step::Wait),
                ["m", x, y] => Some(Step::Move(Vec2::new(x.parse().ok()?, y.parse().ok()?))),
                ["d"] => Some(Step::Down),
                ["u"] => Some(Step::Up),
                ["r"] => Some(Step::RightClick),
                ["rd"] => Some(Step::RightDown),
                ["ru"] => Some(Step::RightUp),
                ["s", n] => n.parse().ok().map(Step::Wheel),
                ["p", file] => Some(Step::Shot(file.to_string())),
                ["q"] => Some(Step::Quit),
                ["k", name] => Some(Step::Key(name.to_string())),
                ["t", text] => Some(Step::Type(text.to_string())),
                _ => None,
            }
        })
        .collect()
}

/// Plays `TROPA_ROBOT` into the window's own event stream, so hovering, dragging and clicking
/// run through exactly the code a real mouse reaches. The player's cursor is never touched.
fn robot(
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut buttons: MessageWriter<bevy::input::mouse::MouseButtonInput>,
    mut keys: MessageWriter<bevy::input::keyboard::KeyboardInput>,
    time: Res<Time>,
    window: Single<(Entity, &Window), With<bevy::window::PrimaryWindow>>,
    mut events: MessageWriter<bevy::window::WindowEvent>,
    mut wheel: MessageWriter<bevy::input::mouse::MouseWheel>,
    mut script: Local<Option<Vec<Step>>>,
    mut at: Local<(usize, f32, Vec2, Vec2)>,
) {
    use bevy::input::mouse::MouseButtonInput;
    use bevy::input::ButtonState;
    use bevy::window::{CursorMoved, WindowEvent};
    let steps = script.get_or_insert_with(|| parse_steps(&std::env::var("TROPA_ROBOT").unwrap_or_default()));
    // `TROPA_DRIVE=<file>`: steps written to that file are taken up and played as they come, so
    // the game can be played a look at a time
    if let Ok(file) = std::env::var("TROPA_DRIVE") {
        if let Ok(text) = std::fs::read_to_string(&file) {
            if std::fs::remove_file(&file).is_ok() {
                steps.extend(parse_steps(&text));
            }
        }
    }
    let (entity, win) = *window;
    // stage pixels to window pixels: the stage is scaled to fit the window
    let k = (win.width() / crate::STAGE_W).min(win.height() / crate::STAGE_H);
    let (index, elapsed, pos, from) = &mut *at;
    let Some(step) = steps.get(*index) else { return };
    *elapsed += time.delta_secs();
    let next = match step {
        Step::Wait(t) => *elapsed >= *t,
        Step::Move(to) => {
            // a glide of a third of a second: a drag needs to be seen moving
            let p = (*elapsed / 0.35).min(1.0);
            let now = from.lerp(*to, p);
            events.write(WindowEvent::CursorMoved(CursorMoved { window: entity, position: now * k, delta: Some((now - *pos) * k) }));
            *pos = now;
            p >= 1.0
        }
        Step::Wheel(n) => {
            use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
            // the wheel is read as its own message, not from the window's event stream
            wheel.write(MouseWheel { unit: MouseScrollUnit::Line, x: 0.0, y: *n, window: entity, phase: bevy::input::touch::TouchPhase::Moved });
            true
        }
        Step::Shot(file) => {
            commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(file.clone()));
            true
        }
        Step::Quit => {
            exit.write(AppExit::Success);
            true
        }
        Step::RightClick => {
            // down one frame, up a little later, as a finger does
            let held = *elapsed < 0.1;
            let state = if held { ButtonState::Pressed } else { ButtonState::Released };
            events.write(WindowEvent::MouseButtonInput(MouseButtonInput { button: MouseButton::Right, state, window: entity }));
            buttons.write(MouseButtonInput { button: MouseButton::Right, state, window: entity });
            !held
        }
        Step::RightDown | Step::RightUp => {
            let state = if matches!(step, Step::RightDown) { ButtonState::Pressed } else { ButtonState::Released };
            events.write(WindowEvent::MouseButtonInput(MouseButtonInput { button: MouseButton::Right, state, window: entity }));
            buttons.write(MouseButtonInput { button: MouseButton::Right, state, window: entity });
            true
        }
        Step::Down | Step::Up => {
            let state = if matches!(step, Step::Down) { ButtonState::Pressed } else { ButtonState::Released };
            // a real click reaches both: picking by the window's stream, and whatever reads the
            // button itself
            events.write(WindowEvent::MouseButtonInput(MouseButtonInput { button: MouseButton::Left, state, window: entity }));
            buttons.write(MouseButtonInput { button: MouseButton::Left, state, window: entity });
            true
        }
        Step::Type(text) => {
            use bevy::input::keyboard::{Key, KeyboardInput};
            for ch in text.chars() {
                let logical_key = Key::Character(ch.to_string().into());
                for state in [ButtonState::Pressed, ButtonState::Released] {
                    keys.write(KeyboardInput { key_code: KeyCode::KeyA, logical_key: logical_key.clone(), state, text: None, repeat: false, window: entity });
                }
            }
            true
        }
        Step::Key(name) => {
            use bevy::input::keyboard::{Key, KeyboardInput};
            let (key_code, logical_key) = match name.as_str() {
                "space" => (KeyCode::Space, Key::Space),
                "enter" => (KeyCode::Enter, Key::Enter),
                "left" => (KeyCode::ArrowLeft, Key::ArrowLeft),
                "right" => (KeyCode::ArrowRight, Key::ArrowRight),
                "esc" => (KeyCode::Escape, Key::Escape),
                "m" => (KeyCode::KeyM, Key::Character("m".into())),
                _ => (KeyCode::Backspace, Key::Backspace),
            };
            for state in [ButtonState::Pressed, ButtonState::Released] {
                keys.write(KeyboardInput { key_code, logical_key: logical_key.clone(), state, text: None, repeat: false, window: entity });
            }
            true
        }
    };
    if next {
        *index += 1;
        *elapsed = 0.0;
        *from = *pos;
    }
}

/// `TROPA_RESIZE=<seconds>:<w>x<h>`: the window changes size once, as when a player maximises it.
fn resize_later(time: Res<Time>, mut windows: Query<&mut Window>, mut done: Local<bool>) {
    if *done {
        return;
    }
    let Ok(spec) = std::env::var("TROPA_RESIZE") else { return };
    let Some((at, size)) = spec.split_once(':') else { return };
    let (Ok(at), Some((w, h))) = (at.parse::<f32>(), size.split_once('x')) else { return };
    if time.elapsed_secs() < at {
        return;
    }
    *done = true;
    if let (Ok(w), Ok(h)) = (w.parse::<f32>(), h.parse::<f32>()) {
        for mut window in &mut windows {
            window.resolution.set(w, h);
        }
    }
}
