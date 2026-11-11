//! Puts `steam_api64.dll` next to the game's exe. The Steamworks crate builds
//! it into its own output folder, where a double-clicked exe would not find it.

use std::path::{Path, PathBuf};

fn detect_version() -> String {
    if let Ok(v) = std::env::var("GAME_VERSION") {
        let clean = v.trim().trim_start_matches('v');
        if !clean.is_empty() {
            return clean.to_string();
        }
    }
    if let Ok(t) = std::env::var("TAG") {
        let clean = t.trim().trim_start_matches('v');
        if !clean.is_empty() {
            return clean.to_string();
        }
    }
    // Attempt git describe
    if let Ok(output) = std::process::Command::new("git")
        .args(["describe", "--tags", "--always"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let clean = s.trim_start_matches('v');
            // If it's a version tag like 0.1.8 or 0.1.8-4-g123456
            if let Some(base) = clean.split('-').next() {
                if base.split('.').count() >= 2 {
                    return base.to_string();
                }
            }
        }
    }
    std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.8".to_string())
}

fn main() {
    let version = detect_version();
    println!("cargo:rustc-env=GAME_VERSION={version}");
    println!("cargo:rerun-if-env-changed=GAME_VERSION");
    println!("cargo:rerun-if-env-changed=TAG");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/tags");

    // OUT_DIR is target/<profile>/build/tropa-<hash>/out
    let Some(out) = std::env::var_os("OUT_DIR").map(PathBuf::from) else { return };
    let Some(profile_dir) = out.ancestors().nth(3).map(Path::to_path_buf) else { return };
    let build_dir = profile_dir.join("build");
    let Ok(entries) = std::fs::read_dir(&build_dir) else { return };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("steamworks-sys-") {
            continue;
        }
        for name in ["steam_api64.dll", "steam_api.dll", "libsteam_api.so", "libsteam_api.dylib"] {
            let from = entry.path().join("out").join(name);
            if from.is_file() {
                let to = profile_dir.join(name);
                // only when missing or stale, so an unchanged build stays quiet
                let stale = std::fs::metadata(&to).map(|m| m.len()).ok() != std::fs::metadata(&from).map(|m| m.len()).ok();
                if stale {
                    if let Err(e) = std::fs::copy(&from, &to) {
                        println!("cargo:warning=could not copy {name} next to the exe: {e}");
                    }
                }
            }
        }
    }
}
