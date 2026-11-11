//! «Тропа в Навь» — Bevy client. Rules live in `nav_core`; this crate only draws and sends actions.
//! `run` is the game: the desktop binary (`main.rs`) and the Android library (`crates/android`)
//! both start it.

// where Steam does not run, its names come from a stand-in that never starts
#[cfg(any(target_os = "android", target_os = "ios"))]
extern crate steam_stub as steamworks;

mod analytics;
mod art;
mod audio;
mod battle;
mod between;
mod book;
mod card_flip;
mod deck_fx;
#[cfg(feature = "dev")]
mod dev;
mod lan;
mod logfile;
mod l10n;
mod lobby;
mod map_ink;
mod map_party;
mod menu;
mod net;
mod preload;
mod pause;
mod platform;
mod profile;
mod reconnect;
mod run_ui;
mod session;
mod steam;
mod story;
mod theme;
mod touch;
mod updater;

use bevy::prelude::*;
use bevy::window::{WindowPosition, WindowResolution};
use l10n::{L10n, Lang};

/// The whole UI is laid out on a fixed canvas and scaled to the window.
pub const STAGE_W: f32 = 1920.0;
pub const STAGE_H: f32 = 1080.0;

#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum AppState {
    #[default]
    Boot,
    Menu,
    Settings,
    /// The last stockade: spend village currency between runs.
    Village,
    /// The graveyard's chronicle: enemies met and runs past.
    Chronicle,
    /// Create or join a party.
    Coop,
    /// Seats, heroes, ready.
    Lobby,
    /// The path map: vote for the next node.
    Map,
    Battle,
    Reward,
    /// The fire.
    Rest,
    /// The pedlar.
    Shop,
    Omen,
    RunEnd,
    /// Balance and card analytics dashboard.
    Analytics,
}

/// Parent of every screen: a 1920x1080 node centred in the window.
#[derive(Component)]
pub struct Stage;

#[derive(Resource, serde::Deserialize, Clone)]
pub struct Config {
    /// 480 is Valve's Spacewar test app; swap for the real id once the game has one.
    pub steam_app_id: u32,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub telemetry_url: Option<String>,
}

fn load_config(art: &art::Art) -> Config {
    art.read_text("config.ron")
        .and_then(|s| ron::from_str(&s).ok())
        .unwrap_or(Config { steam_app_id: 480, lang: None, telemetry_url: None })
}

/// Starts the game and runs it until it is closed.
pub fn run() {
    let assets = art::assets_dir();
    let art = art::Art::new(assets.clone());
    let config = load_config(&art);
    let steam = steam::init(config.steam_app_id);
    // playing without Steam, everyone on this machine shares one local profile
    let account = steam.as_ref().ok().map(|c| c.user().steam_id().raw()).unwrap_or(0);
    let profile = profile::Profile::load(account);
    // what the player last chose in the game, else what the config says, else the language the game
    // was installed in, else Steam's
    let installed = art.read_text("install_lang.txt").and_then(|word| Lang::from_word(&word));
    let lang = Lang::from_saved(profile.settings.text_lang)
        .or_else(|| config.lang.as_deref().map(|word| Lang::from_word(word).unwrap_or(Lang::Old)))
        .or(installed)
        .or_else(|| steam.as_ref().ok().map(steam::ui_lang))
        .unwrap_or(Lang::Old);
    // the build's fingerprint: its version and every rule file it read, so a party is never
    // played by two builds that would deal one run out two ways
    let fingerprint = std::cell::Cell::new(fnv(0xcbf2_9ce4_8422_2325, env!("GAME_VERSION").as_bytes()));
    let content = match nav_core::Content::load_with(&|sub| {
        let dir = format!("data/{sub}");
        let files: Vec<(String, String)> = art.list(&dir, "ron").into_iter().filter_map(|name| art.read_text(&format!("{dir}/{name}")).map(|src| (format!("{dir}/{name}"), src))).collect();
        for (name, src) in &files {
            fingerprint.set(fnv(fnv(fingerprint.get(), name.as_bytes()), src.replace('\r', "").as_bytes()));
        }
        Ok(files)
    }) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("content failed to load:\n{e}");
            std::process::exit(1);
        }
    };

    nav_net::set_build(fingerprint.get());
    let config_lang = lang;
    logfile::catch_panics();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(bevy::log::LogPlugin { custom_layer: logfile::layer, ..default() })
            .set(WindowPlugin {
                primary_window: Some(test_window(Window {
                    title: "Тропа в Навь".into(),
                    resolution: WindowResolution::new(1600, 900),
                    ..default()
                })),
                ..default()
            })
            .set(AssetPlugin { file_path: assets.to_string_lossy().into_owned(), ..default() }),
    )
    .insert_resource(ClearColor(theme::SOOT))
    .insert_resource(L10n::load(&art, lang))
    .insert_resource(art)
    .insert_resource(session::Db(content))
    .insert_resource(config)
    .insert_resource(profile)
    .init_state::<AppState>()
    .add_systems(Startup, (theme::load_fonts, setup_stage))
    .add_systems(PostStartup, menu::apply_settings)
    .add_systems(Update, fit_stage)
    .add_plugins((menu::plugin, battle::plugin, net::plugin, lobby::plugin, run_ui::plugin, audio::plugin, story::plugin, book::plugin, deck_fx::plugin, card_flip::plugin, map_party::plugin, map_ink::plugin, between::plugin, preload::plugin))
    .add_plugins((reconnect::plugin, pause::plugin, touch::plugin, updater::plugin, analytics::plugin));

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    app.add_systems(Update, window_icon);
    app.add_systems(Update, hide_system_bars);

    #[cfg(feature = "dev")]
    app.add_plugins(dev::plugin);
    app.add_plugins(lan::plugin).add_systems(Update, (logfile::note_screens, name_window));
    info!("Tropa v Nav {} ({}), language {:?}, Steam {}", env!("GAME_VERSION"), std::env::consts::OS, config_lang, steam.is_ok());

    // without Steam the game still runs: alone, or with friends over a local network
    match steam {
        Ok(client) => {
            steam::install(&mut app, client);
            app.add_systems(PostStartup, |mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu));
        }
        Err(e) => {
            warn!("Steam unavailable ({e}); running without it");
            app.add_systems(PostStartup, |mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu));
        }
    }
    app.run();
}

/// The window goes by the game's name in the language chosen.
fn name_window(l10n: Res<L10n>, mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>) {
    if l10n.is_changed() {
        for mut window in &mut windows {
            window.title = l10n.t("game-title");
        }
    }
}

fn setup_stage(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            ..default()
        },
        Pickable::IGNORE,
        children![(
            Stage,
            Node { width: px(STAGE_W), height: px(STAGE_H), flex_shrink: 0.0, ..default() },
            Pickable::IGNORE,
        )],
    ));
}

fn fit_stage(window: Single<&Window>, mut scale: ResMut<UiScale>) {
    let s = (window.width() / STAGE_W).min(window.height() / STAGE_H).max(0.1);
    if (scale.0 - s).abs() > 1e-4 {
        scale.0 = s;
    }
}

/// Cursor position in stage coordinates.
pub fn stage_cursor(window: &Window, scale: f32) -> Option<Vec2> {
    Some(stage_point(window, scale, window.cursor_position().or_else(touch::finger)?))
}

/// A point in the window (logical pixels) in stage coordinates.
pub fn stage_point(window: &Window, scale: f32, at: Vec2) -> Vec2 {
    let origin = Vec2::new(window.width() - STAGE_W * scale, window.height() - STAGE_H * scale) / 2.0;
    (at - origin) / scale
}

/// A dev screenshot run (`TROPA_SHOT`) opens its window out of the way: unfocused, beneath every
/// other window and off the screen, so a game being played meanwhile is never covered or
/// minimised. `TROPA_SHOW=1` shows it as usual.
fn test_window(window: Window) -> Window {
    let hidden = cfg!(feature = "dev") && std::env::var("TROPA_SHOT").is_ok() && std::env::var("TROPA_SHOW").is_err();
    if !hidden {
        return window;
    }
    // TROPA_WINDOW=<w>x<h> shoots at another window size, to see the stage fit it
    let resolution = std::env::var("TROPA_WINDOW")
        .ok()
        .and_then(|v| v.split_once('x').and_then(|(w, h)| Some(WindowResolution::new(w.parse().ok()?, h.parse().ok()?))))
        .unwrap_or(window.resolution.clone());
    Window {
        resolution,
        focused: false,
        window_level: bevy::window::WindowLevel::AlwaysOnBottom,
        position: WindowPosition::At(IVec2::new(-6000, -6000)),
        skip_taskbar: true,
        ..window
    }
}

/// FNV-1a over `bytes`, carried on from `h`.
fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        h = (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Puts the game's icon (`assets/icon.png`) on its window, once the window is there.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn window_icon(art: Res<art::Art>, windows: Query<Entity, With<bevy::window::PrimaryWindow>>, mut done: Local<bool>, _main_thread: bevy::ecs::system::NonSendMarker) {
    if *done {
        return;
    }
    let Ok(entity) = windows.single() else { return };
    let icon = art.file("icon.png").and_then(|rel| {
        let ext = rel.rsplit_once('.')?.1.to_string();
        let bytes = art.read(&rel)?;
        let image = Image::from_buffer(&bytes, bevy::image::ImageType::Extension(&ext), default(), true, default(), bevy::asset::RenderAssetUsages::all()).ok()?;
        let rgba = image.try_into_dynamic().ok()?.into_rgba8();
        let (w, h) = rgba.dimensions();
        winit::window::Icon::from_rgba(rgba.into_raw(), w, h).ok()
    });
    let Some(icon) = icon else {
        *done = true;
        return;
    };
    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        if let Some(window) = windows.get_window(entity) {
            window.set_window_icon(Some(icon));
            *done = true;
        }
    });
}

/// On a phone the game fills the screen: the system bars are hidden when it starts and each time it
/// comes back to the front.
fn hide_system_bars(mut focus: MessageReader<bevy::window::WindowFocused>, mut started: Local<bool>) {
    let back = focus.read().fold(false, |any, f| any || f.focused);
    if back || !*started {
        *started = true;
        platform::immersive();
    }
}
