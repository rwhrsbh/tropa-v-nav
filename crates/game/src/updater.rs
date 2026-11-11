//! In-game auto-updater for all platforms.
//! - Windows: downloads `TropaVNav-Setup-*.exe`, runs it, exits game cleanly.
//! - Linux: downloads `TropaVNav-linux.tar.gz`, unpacks directly to the game directory, cleans up, restarts.
//! - Android: downloads APK, invokes Android package installer intent, cleans up.
//! - macOS: shows notification modal and button to open download page in browser.
//! All temporary files (.exe, .tar.gz, .apk, .old) are cleaned up automatically.

use crate::l10n::L10n;
use crate::menu::menu_button;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage, STAGE_H, STAGE_W};
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub fn plugin(app: &mut App) {
    app.init_resource::<UpdaterState>()
        .add_systems(Startup, (start_background_check, cleanup_leftovers))
        .add_systems(Update, (poll_background_tasks, sync_update_ui.run_if(in_state(AppState::Menu))));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    pub fn parse(s: &str) -> Option<Self> {
        let clean = s.trim().strip_prefix('v').unwrap_or(s.trim());
        let mut parts = clean.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch_str = parts.next().unwrap_or("0").split('-').next()?;
        let patch = patch_str.parse().ok()?;
        Some(Self { major, minor, patch })
    }

    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Self { major: 0, minor: 1, patch: 0 })
    }

    pub fn to_string(self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub version: SemVer,
    pub html_url: String,
    pub asset_name: Option<String>,
    pub download_url: Option<String>,
    pub body: String,
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    Available(ReleaseInfo),
    Downloading {
        percent: f32,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Installing,
    Error(String),
    UpToDate,
}

#[derive(Resource, Default)]
pub struct UpdaterState {
    pub status: UpdateStatus,
    pub dismissed: bool,
    pub progress_tracker: Option<DownloadTracker>,
    pub check_rx: Option<Arc<Mutex<Option<Result<Option<ReleaseInfo>, String>>>>>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct DownloadTracker {
    pub downloaded: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
    pub finished: Arc<AtomicBool>,
    pub error: Arc<Mutex<Option<String>>>,
    pub target_path: PathBuf,
    pub release: ReleaseInfo,
}

/// Matches platform-specific release assets from GitHub Releases API response.
pub fn match_platform_asset<'a>(assets: &'a [serde_json::Value]) -> Option<(&'a str, &'a str)> {
    for asset in assets {
        let Some(name) = asset.get("name").and_then(|v| v.as_str()) else { continue };
        let Some(url) = asset.get("browser_download_url").and_then(|v| v.as_str()) else { continue };

        #[cfg(windows)]
        if name.ends_with(".exe") && (name.contains("Setup") || name.contains("TropaVNav")) {
            return Some((name, url));
        }

        #[cfg(target_os = "linux")]
        if name.contains("linux") && (name.ends_with(".tar.gz") || name.ends_with(".tgz")) {
            return Some((name, url));
        }

        #[cfg(target_os = "macos")]
        if name.contains("macos") && (name.ends_with(".tar.gz") || name.ends_with(".dmg") || name.ends_with(".zip")) {
            return Some((name, url));
        }

        #[cfg(target_os = "android")]
        if name.ends_with(".apk") {
            return Some((name, url));
        }
    }
    None
}

/// Matches assets for a specific OS name (for testing across platforms).
#[allow(dead_code)]
pub fn match_asset_for_os<'a>(os: &str, assets: &'a [serde_json::Value]) -> Option<(&'a str, &'a str)> {
    for asset in assets {
        let Some(name) = asset.get("name").and_then(|v| v.as_str()) else { continue };
        let Some(url) = asset.get("browser_download_url").and_then(|v| v.as_str()) else { continue };

        match os {
            "windows" if name.ends_with(".exe") && (name.contains("Setup") || name.contains("TropaVNav")) => {
                return Some((name, url));
            }
            "linux" if name.contains("linux") && (name.ends_with(".tar.gz") || name.ends_with(".tgz")) => {
                return Some((name, url));
            }
            "macos" if name.contains("macos") && (name.ends_with(".tar.gz") || name.ends_with(".dmg") || name.ends_with(".zip")) => {
                return Some((name, url));
            }
            "android" if name.ends_with(".apk") => {
                return Some((name, url));
            }
            _ => {}
        }
    }
    None
}

/// Queries GitHub releases synchronously (meant to be run in a background thread).
pub fn check_for_updates_sync() -> Result<Option<ReleaseInfo>, String> {
    let current = SemVer::current();
    let url = "https://api.github.com/repos/rwhrsbh/tropa-v-nav/releases";

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("tropa-v-nav")
        .build();

    let res = agent.get(url).call().map_err(|e| format!("{e}"))?;
    let releases: Vec<serde_json::Value> = res.into_json().map_err(|e| format!("{e}"))?;

    for rel in releases {
        let draft = rel.get("draft").and_then(|v| v.as_bool()).unwrap_or(false);
        let prerelease = rel.get("prerelease").and_then(|v| v.as_bool()).unwrap_or(false);
        if draft || prerelease {
            continue;
        }

        let Some(tag_name) = rel.get("tag_name").and_then(|v| v.as_str()) else { continue };
        let Some(version) = SemVer::parse(tag_name) else { continue };

        if version > current {
            let html_url = rel.get("html_url").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let body = rel.get("body").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let empty_vec = vec![];
            let assets = rel.get("assets").and_then(|v| v.as_array()).unwrap_or(&empty_vec);
            let (asset_name, download_url) = match_platform_asset(assets)
                .map(|(n, u)| (Some(n.to_string()), Some(u.to_string())))
                .unwrap_or((None, None));

            return Ok(Some(ReleaseInfo {
                tag_name: tag_name.to_string(),
                version,
                html_url,
                asset_name,
                download_url,
                body,
            }));
        }
    }
    Ok(None)
}

/// Spawns a background thread to check GitHub for updates.
pub fn trigger_check(state: &mut UpdaterState) {
    state.status = UpdateStatus::Checking;
    let slot = Arc::new(Mutex::new(None));
    state.check_rx = Some(slot.clone());

    std::thread::Builder::new()
        .name("updater-check".into())
        .spawn(move || {
            let result = check_for_updates_sync();
            if let Ok(mut lock) = slot.lock() {
                *lock = Some(result);
            }
        })
        .ok();
}

fn start_background_check(mut state: ResMut<UpdaterState>) {
    trigger_check(&mut state);
}

/// Removes any junk/temporary files left behind by past updates.
pub fn cleanup_leftovers() {
    let temp = std::env::temp_dir();

    // Windows setup artifact
    let win_tmp = temp.join("tropa-update-setup.exe");
    if win_tmp.exists() {
        let _ = std::fs::remove_file(win_tmp);
    }

    // Linux tar artifact & backup binary
    let lin_tmp = temp.join("tropa_update.tar.gz");
    if lin_tmp.exists() {
        let _ = std::fs::remove_file(lin_tmp);
    }
    if let Ok(exe) = std::env::current_exe() {
        let old = exe.with_file_name("tropa.old");
        if old.exists() {
            let _ = std::fs::remove_file(old);
        }
    }

    // Android APK in cache
    let apk_tmp = temp.join("tropa-update.apk");
    if apk_tmp.exists() {
        let _ = std::fs::remove_file(apk_tmp);
    }
}

/// Starts downloading the release asset for Windows, Linux or Android.
pub fn start_download(state: &mut UpdaterState, info: ReleaseInfo) {
    let Some(download_url) = info.download_url.clone().or_else(|| Some(info.html_url.clone())) else { return };

    let filename = if cfg!(windows) {
        "tropa-update-setup.exe"
    } else if cfg!(target_os = "android") {
        "tropa-update.apk"
    } else {
        "tropa_update.tar.gz"
    };

    let target_path = std::env::temp_dir().join(filename);
    let tracker = DownloadTracker {
        downloaded: Arc::new(AtomicU64::new(0)),
        total: Arc::new(AtomicU64::new(0)),
        finished: Arc::new(AtomicBool::new(false)),
        error: Arc::new(Mutex::new(None)),
        target_path: target_path.clone(),
        release: info.clone(),
    };

    state.progress_tracker = Some(tracker.clone());
    state.status = UpdateStatus::Downloading {
        percent: 0.0,
        downloaded_bytes: 0,
        total_bytes: None,
    };

    std::thread::Builder::new()
        .name("updater-download".into())
        .spawn(move || {
            use std::io::{Read, Write};
            let agent = ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(300))
                .user_agent("tropa-v-nav")
                .build();

            let response = match agent.get(&download_url).call() {
                Ok(r) => r,
                Err(e) => {
                    if let Ok(mut lock) = tracker.error.lock() {
                        *lock = Some(format!("download request failed: {e}"));
                    }
                    return;
                }
            };

            let total_len = response
                .header("Content-Length")
                .and_then(|l| l.parse::<u64>().ok())
                .unwrap_or(0);
            tracker.total.store(total_len, Ordering::Relaxed);

            let mut reader = response.into_reader();
            let mut file = match std::fs::File::create(&target_path) {
                Ok(f) => f,
                Err(e) => {
                    if let Ok(mut lock) = tracker.error.lock() {
                        *lock = Some(format!("cannot create file {}: {e}", target_path.display()));
                    }
                    return;
                }
            };

            let mut buf = [0u8; 65536];
            let mut bytes_so_far = 0u64;

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Err(e) = file.write_all(&buf[..n]) {
                            if let Ok(mut lock) = tracker.error.lock() {
                                *lock = Some(format!("write error: {e}"));
                            }
                            return;
                        }
                        bytes_so_far += n as u64;
                        tracker.downloaded.store(bytes_so_far, Ordering::Relaxed);
                    }
                    Err(e) => {
                        if let Ok(mut lock) = tracker.error.lock() {
                            *lock = Some(format!("read error: {e}"));
                        }
                        return;
                    }
                }
            }

            let _ = file.flush();
            tracker.finished.store(true, Ordering::Relaxed);
        })
        .ok();
}

/// Applies the update on the local platform.
pub fn apply_update(target_path: &PathBuf, mut exit_writer: MessageWriter<AppExit>) {
    #[cfg(windows)]
    {
        bevy::log::info!("launching Windows installer: {}", target_path.display());
        if let Err(e) = std::process::Command::new(target_path).spawn() {
            bevy::log::error!("failed to launch installer: {e}");
        } else {
            exit_writer.write(AppExit::Success);
        }
    }

    #[cfg(target_os = "linux")]
    {
        bevy::log::info!("extracting Linux update: {}", target_path.display());
        let res: Result<(), String> = (|| {
            let exe = std::env::current_exe().map_err(|e| format!("{e}"))?;
            let game_dir = exe.parent().ok_or_else(|| "no parent dir".to_string())?;
            let old_exe = game_dir.join("tropa.old");
            let _ = std::fs::rename(&exe, &old_exe);

            let status = std::process::Command::new("tar")
                .args(["-xzf", target_path.to_str().unwrap(), "-C", game_dir.to_str().unwrap(), "--strip-components=1"])
                .status()
                .map_err(|e| format!("{e}"))?;

            if !status.success() {
                let _ = std::fs::rename(&old_exe, &exe);
                return Err("tar extraction failed".into());
            }

            let _ = std::fs::remove_file(target_path);
            let _ = std::fs::remove_file(&old_exe);

            // Restart updated game
            std::process::Command::new(&exe)
                .args(std::env::args().skip(1))
                .spawn()
                .map_err(|e| format!("{e}"))?;
            Ok(())
        })();

        match res {
            Ok(()) => {
                exit_writer.write(AppExit::Success);
            }
            Err(e) => {
                bevy::log::error!("Linux update failed: {e}");
            }
        }
    }

    #[cfg(target_os = "android")]
    {
        bevy::log::info!("prompting Android APK install: {}", target_path.display());
        crate::platform::open_url(&format!("file://{}", target_path.display()));
    }

    #[cfg(target_os = "macos")]
    {
        let _ = target_path;
        let _ = exit_writer;
    }
}

/// Polls background download or update check threads.
fn poll_background_tasks(mut state: ResMut<UpdaterState>, exit_writer: MessageWriter<AppExit>) {
    // Check update check slot
    let check_res = state.check_rx.as_ref().and_then(|rx| {
        rx.try_lock().ok().and_then(|mut lock| lock.take())
    });

    if let Some(res) = check_res {
        state.check_rx = None;
        match res {
            Ok(Some(release)) => {
                bevy::log::info!("new release available: {}", release.tag_name);
                state.status = UpdateStatus::Available(release);
            }
            Ok(None) => {
                state.status = UpdateStatus::UpToDate;
            }
            Err(e) => {
                bevy::log::warn!("updater check failed: {e}");
                state.status = UpdateStatus::Error(e);
            }
        }
    }

    // Check download tracker
    let tracker_opt = state.progress_tracker.clone();
    if let Some(tracker) = tracker_opt {
        if let Ok(lock) = tracker.error.try_lock() {
            if let Some(err) = lock.as_ref() {
                state.status = UpdateStatus::Error(err.clone());
                state.progress_tracker = None;
                return;
            }
        }

        let downloaded = tracker.downloaded.load(Ordering::Relaxed);
        let total = tracker.total.load(Ordering::Relaxed);
        let finished = tracker.finished.load(Ordering::Relaxed);

        if finished {
            state.status = UpdateStatus::Installing;
            state.progress_tracker = None;
            apply_update(&tracker.target_path, exit_writer);
        } else {
            let total_opt = if total > 0 { Some(total) } else { None };
            let percent = if total > 0 {
                ((downloaded as f64 / total as f64) * 100.0) as f32
            } else {
                0.0
            };
            state.status = UpdateStatus::Downloading {
                percent,
                downloaded_bytes: downloaded,
                total_bytes: total_opt,
            };
        }
    }
}

// ---------------------------------------------------------------------------
// UI Systems
// ---------------------------------------------------------------------------

#[derive(Component)]
struct UpdateOverlay;

#[derive(Component)]
struct ProgressBarFill;

fn sync_update_ui(
    mut commands: Commands,
    state: Res<UpdaterState>,
    existing: Query<Entity, With<UpdateOverlay>>,
    mut fill_query: Query<&mut Node, With<ProgressBarFill>>,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
) {
    let should_show = !state.dismissed
        && matches!(
            state.status,
            UpdateStatus::Available(_) | UpdateStatus::Downloading { .. } | UpdateStatus::Installing | UpdateStatus::Error(_)
        );

    if !should_show {
        for e in &existing {
            commands.entity(e).despawn();
        }
        return;
    }

    // If downloading, update progress bar fill width without respawning
    if let UpdateStatus::Downloading { percent, .. } = state.status {
        for mut node in &mut fill_query {
            node.width = percent_val(percent.clamp(2.0, 100.0));
        }
    }

    if !existing.is_empty() {
        return;
    }

    // Spawn the modal dialog
    let overlay_entity = commands
        .spawn((
            UpdateOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: px(STAGE_W),
                height: px(STAGE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.90)),
            GlobalZIndex(1200),
            ChildOf(*stage),
        ))
        .id();

    match &state.status {
        UpdateStatus::Available(release) => {
            let rel = release.clone();
            commands.entity(overlay_entity).with_children(|p| {
                p.spawn(theme::text(l10n.t("updater-title"), &fonts.title, 56.0, theme::BONE));
                p.spawn(theme::text(
                    l10n.tf(
                        "updater-available",
                        &[
                            ("version", rel.version.to_string().into()),
                            ("current", SemVer::current().to_string().into()),
                        ],
                    ),
                    &fonts.body,
                    26.0,
                    theme::CANDLE,
                ));

                #[cfg(target_os = "macos")]
                {
                    p.spawn(theme::text(
                        l10n.t("updater-mac-hint"),
                        &fonts.body,
                        22.0,
                        theme::DIM,
                    ));
                    let download_url = rel.download_url.clone().unwrap_or_else(|| rel.html_url.clone());
                    p.spawn((
                        menu_button(l10n.t("updater-btn-download"), &fonts),
                        observe(move |_: On<Activate>, mut state: ResMut<UpdaterState>| {
                            crate::platform::open_url(&download_url);
                            state.dismissed = true;
                        }),
                    ));
                }

                #[cfg(not(target_os = "macos"))]
                {
                    let r_for_update = rel.clone();
                    p.spawn((
                        menu_button(l10n.t("updater-btn-update"), &fonts),
                        observe(move |_: On<Activate>, mut state: ResMut<UpdaterState>| {
                            start_download(&mut state, r_for_update.clone());
                        }),
                    ));

                    let browser_url = rel.html_url.clone();
                    p.spawn((
                        menu_button(l10n.t("updater-btn-browser"), &fonts),
                        observe(move |_: On<Activate>| {
                            crate::platform::open_url(&browser_url);
                        }),
                    ));

                    #[cfg(target_os = "android")]
                    p.spawn(theme::text(
                        l10n.t("updater-android-sig-hint"),
                        &fonts.body,
                        20.0,
                        theme::DIM,
                    ));
                }

                p.spawn((
                    menu_button(l10n.t("updater-btn-later"), &fonts),
                    observe(|_: On<Activate>, mut state: ResMut<UpdaterState>| {
                        state.dismissed = true;
                    }),
                ));
            });
        }
        UpdateStatus::Downloading { percent, downloaded_bytes, .. } => {
            let mb = (*downloaded_bytes as f32) / (1024.0 * 1024.0);
            commands.entity(overlay_entity).with_children(|p| {
                p.spawn(theme::text(l10n.t("updater-title"), &fonts.title, 56.0, theme::BONE));
                p.spawn(theme::text(
                    l10n.tf(
                        "updater-downloading",
                        &[
                            ("percent", (*percent as i32).into()),
                            ("mb", format!("{mb:.1}").into()),
                        ],
                    ),
                    &fonts.body,
                    26.0,
                    theme::CANDLE,
                ));

                // Progress bar container
                p.spawn((
                    Node {
                        width: px(520),
                        height: px(28),
                        border: UiRect::all(px(2)),
                        padding: UiRect::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(theme::SOOT),
                    BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        ProgressBarFill,
                        Node {
                            width: percent_val(percent.clamp(2.0, 100.0)),
                            height: percent_val(100.0),
                            ..default()
                        },
                        BackgroundColor(theme::CANDLE),
                    ));
                });
            });
        }
        UpdateStatus::Installing => {
            commands.entity(overlay_entity).with_children(|p| {
                p.spawn(theme::text(l10n.t("updater-title"), &fonts.title, 56.0, theme::BONE));
                p.spawn(theme::text(l10n.t("updater-installing"), &fonts.body, 28.0, theme::CANDLE));
            });
        }
        UpdateStatus::Error(err) => {
            let err_msg = err.clone();
            commands.entity(overlay_entity).with_children(|p| {
                p.spawn(theme::text(l10n.t("updater-title"), &fonts.title, 56.0, theme::BONE));
                p.spawn(theme::text(
                    l10n.tf("updater-error", &[("err", err_msg.as_str().into())]),
                    &fonts.body,
                    22.0,
                    theme::CANDLE,
                ));
                p.spawn((
                    menu_button(l10n.t("updater-btn-later"), &fonts),
                    observe(|_: On<Activate>, mut state: ResMut<UpdaterState>| {
                        state.dismissed = true;
                    }),
                ));
            });
        }
        _ => {}
    }
}

fn percent_val(v: f32) -> Val {
    Val::Percent(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_parse() {
        assert_eq!(SemVer::parse("0.1.6"), Some(SemVer { major: 0, minor: 1, patch: 6 }));
        assert_eq!(SemVer::parse("v0.1.4"), Some(SemVer { major: 0, minor: 1, patch: 4 }));
        assert_eq!(SemVer::parse("v1.2.3-rc1"), Some(SemVer { major: 1, minor: 2, patch: 3 }));
        assert_eq!(SemVer::parse("invalid"), None);
    }

    #[test]
    fn test_semver_order() {
        let v010 = SemVer { major: 0, minor: 1, patch: 0 };
        let v014 = SemVer { major: 0, minor: 1, patch: 4 };
        let v016 = SemVer { major: 0, minor: 1, patch: 6 };
        let v020 = SemVer { major: 0, minor: 2, patch: 0 };

        assert!(v014 > v010);
        assert!(v016 > v014);
        assert!(v020 > v016);
        assert!(v010 < v016);
    }

    #[test]
    fn test_cleanup_leftovers_no_panic() {
        cleanup_leftovers();
    }

    #[test]
    fn test_match_asset_for_os() {
        let assets = serde_json::json!([
            { "name": "TropaVNav-android.apk", "browser_download_url": "https://example.com/apk" },
            { "name": "TropaVNav-linux.tar.gz", "browser_download_url": "https://example.com/linux" },
            { "name": "TropaVNav-macos.tar.gz", "browser_download_url": "https://example.com/macos" },
            { "name": "TropaVNav-Setup-0.1.0.exe", "browser_download_url": "https://example.com/win" }
        ]);
        let assets_arr = assets.as_array().unwrap();

        assert_eq!(
            match_asset_for_os("windows", assets_arr),
            Some(("TropaVNav-Setup-0.1.0.exe", "https://example.com/win"))
        );
        assert_eq!(
            match_asset_for_os("linux", assets_arr),
            Some(("TropaVNav-linux.tar.gz", "https://example.com/linux"))
        );
        assert_eq!(
            match_asset_for_os("macos", assets_arr),
            Some(("TropaVNav-macos.tar.gz", "https://example.com/macos"))
        );
        assert_eq!(
            match_asset_for_os("android", assets_arr),
            Some(("TropaVNav-android.apk", "https://example.com/apk"))
        );
    }
}
