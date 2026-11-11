//! What differs from one system to the next, kept in one place so no system is forgotten: where
//! the player's files live, the clipboard, this machine's addresses, whether the screen is touched
//! rather than pointed at, and how the game's own files are read (loose on a desktop, packed inside
//! the APK on Android).

use std::path::PathBuf;

/// Played with fingers on a touch screen: no right button, no hover, no keyboard at hand.
pub const TOUCH: bool = cfg!(any(target_os = "android", target_os = "ios"));

/// The folder the game keeps the player's own files in (profiles, saves, logs).
pub fn user_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        return std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("TropaVNav"));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/TropaVNav"));
    }
    #[cfg(target_os = "android")]
    {
        return bevy::android::ANDROID_APP.get().and_then(|app| app.internal_data_path()).map(|d| d.join("TropaVNav"));
    }
    #[cfg(target_os = "ios")]
    {
        return std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/TropaVNav"));
    }
    // Linux and the other unixes: the XDG data folder
    #[allow(unreachable_code)]
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .map(|d| d.join("tropa-v-nav"))
}

/// Runs a helper program with no console window flashing up on Windows.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn quiet(program: &str) -> std::process::Command {
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

/// The text on the system clipboard.
pub fn clipboard_get() -> Option<String> {
    #[cfg(windows)]
    let out = quiet("powershell").args(["-NoProfile", "-Command", "Get-Clipboard"]).output().ok();
    #[cfg(target_os = "macos")]
    let out = quiet("pbpaste").output().ok();
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android", target_os = "ios"))))]
    let out = quiet("wl-paste")
        .arg("--no-newline")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .or_else(|| quiet("xclip").args(["-selection", "clipboard", "-o"]).output().ok().filter(|o| o.status.success()))
        .or_else(|| quiet("xsel").args(["--clipboard", "--output"]).output().ok());
    // a phone's clipboard is reached through Java; the code is typed there instead
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let out: Option<std::process::Output> = None;
    out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

/// Puts `text` on the system clipboard.
pub fn clipboard_set(text: &str) {
    #[cfg(windows)]
    {
        let _ = quiet("powershell").args(["-NoProfile", "-Command", &format!("Set-Clipboard -Value '{}'", text.replace('\'', ""))]).spawn();
    }
    #[cfg(all(unix, not(any(target_os = "android", target_os = "ios"))))]
    {
        use std::io::Write;
        #[cfg(target_os = "macos")]
        let tools: &[(&str, &[&str])] = &[("pbcopy", &[])];
        #[cfg(not(target_os = "macos"))]
        let tools: &[(&str, &[&str])] = &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"]), ("xsel", &["--clipboard", "--input"])];
        for (tool, args) in tools {
            let Ok(mut child) = quiet(tool).args(*args).stdin(std::process::Stdio::piped()).spawn() else { continue };
            if let Some(mut input) = child.stdin.take() {
                let _ = input.write_all(text.as_bytes());
            }
            let _ = child.wait();
            return;
        }
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = text;
}

/// This machine's IPv4 addresses, as the system lists them.
/// Opens a URL or system path in the platform default application / browser.
pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        let _ = quiet("cmd").args(["/c", "start", "", url]).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = quiet("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android", target_os = "ios"))))]
    {
        let _ = quiet("xdg-open").arg(url).spawn();
    }
    #[cfg(target_os = "android")]
    {
        if let Err(e) = android_open_url(url) {
            bevy::log::warn!("failed to open url on android: {e}");
        }
    }
    #[cfg(target_os = "ios")]
    {
        let _ = url;
    }
}

pub fn local_ipv4s() -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut keep = |ip: &str| {
        let ip = ip.trim().split('/').next().unwrap_or_default();
        if ip.parse::<std::net::Ipv4Addr>().is_ok_and(|a| !a.is_loopback()) && !found.iter().any(|f| f == ip) {
            found.push(ip.to_string());
        }
    };
    #[cfg(windows)]
    if let Ok(out) = quiet("ipconfig").output() {
        for line in String::from_utf8_lossy(&out.stdout).lines().filter(|l| l.contains("IPv4")) {
            if let Some(ip) = line.rsplit(':').next() {
                keep(ip);
            }
        }
    }
    // macOS and the BSDs: `inet 10.0.0.5 netmask ...`; Linux: `inet 10.0.0.5/24 brd ...`
    #[cfg(all(unix, not(any(target_os = "android", target_os = "ios"))))]
    {
        let listed = quiet("ip").args(["-4", "-o", "addr", "show"]).output().ok().filter(|o| o.status.success()).or_else(|| quiet("ifconfig").output().ok());
        if let Some(out) = listed {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut words = text.split_whitespace();
            while let Some(w) = words.next() {
                if w == "inet" {
                    if let Some(ip) = words.next() {
                        keep(ip.trim_start_matches("addr:"));
                    }
                }
            }
        }
    }
    // everywhere, phones too: the address the system would send from (no packet leaves)
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("192.0.2.1:9").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                keep(&addr.ip().to_string());
            }
        }
    }
    found
}

/// Reads one of the game's own files, `rel` under the assets folder `root`. On Android they lie
/// packed inside the APK, where only the asset manager reaches them.
pub fn read_asset(root: &std::path::Path, rel: &str) -> Option<Vec<u8>> {
    #[cfg(target_os = "android")]
    {
        use std::io::Read;
        let _ = root;
        let app = bevy::android::ANDROID_APP.get()?;
        let name = std::ffi::CString::new(rel).ok()?;
        let mut asset = app.asset_manager().open(&name)?;
        let mut bytes = Vec::new();
        asset.read_to_end(&mut bytes).ok()?;
        return Some(bytes);
    }
    #[allow(unreachable_code)]
    std::fs::read(root.join(rel)).ok()
}

/// Hides the phone's status and navigation bars, sticky: a swipe from an edge shows them only for
/// a moment, and leaving the game takes a second swipe. Asked again whenever the game comes back to
/// the front, since the system shows them again then. Nothing on the desktops.
pub fn immersive() {
    #[cfg(target_os = "android")]
    if let Err(e) = android_immersive() {
        bevy::log::warn!("could not hide the system bars: {e}");
    }
}

/// The Java half (`crates/android/java/tropa/Immersive.java`, built into `immersive.dex`) is
/// loaded from memory and handed to the UI thread, which alone may change the bars.
#[cfg(target_os = "android")]
fn android_open_url(url: &str) -> Result<(), jni::errors::Error> {
    use jni::objects::{JObject, JValue};
    let Some(app) = bevy::android::ANDROID_APP.get() else { return Ok(()) };
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut _)? };
    let mut env = vm.attach_current_thread()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    let uri_str = env.new_string(url)?;
    let uri = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&uri_str)])?.l()?;
    let action = env.new_string("android.intent.action.VIEW")?;
    let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;Landroid/net/Uri;)V", &[JValue::Object(&action), JValue::Object(&uri)])?;
    env.call_method(&activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)])?;
    Ok(())
}

#[cfg(target_os = "android")]
fn android_immersive() -> Result<(), jni::errors::Error> {
    use jni::objects::{JObject, JValue};
    static DEX: &[u8] = include_bytes!("../../android/immersive.dex");
    let Some(app) = bevy::android::ANDROID_APP.get() else { return Ok(()) };
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut _)? };
    let mut env = vm.attach_current_thread()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    let bytes = env.byte_array_from_slice(DEX)?;
    let buffer = env.call_static_method("java/nio/ByteBuffer", "wrap", "([B)Ljava/nio/ByteBuffer;", &[JValue::Object(&bytes)])?.l()?;
    let parent = env.call_method(&activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
    let loader = env.new_object("dalvik/system/InMemoryDexClassLoader", "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V", &[JValue::Object(&buffer), JValue::Object(&parent)])?;
    let name = env.new_string("tropa.Immersive")?;
    let class = env.call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&name)])?.l()?;
    let class = jni::objects::JClass::from(class);
    let task = env.new_object(&class, "(Landroid/app/Activity;)V", &[JValue::Object(&activity)])?;
    env.call_method(&activity, "runOnUiThread", "(Ljava/lang/Runnable;)V", &[JValue::Object(&task)])?;
    Ok(())
}

/// Whether the game's files are packed where `std::fs` cannot see them, so the list of them
/// (`index.txt`, written by the packer) has to be asked instead.
pub const PACKED_ONLY: bool = cfg!(target_os = "android");

#[cfg(test)]
mod tests {
    #[test]
    fn some_address_is_found_or_none_at_all() {
        for ip in super::local_ipv4s() {
            assert!(ip.parse::<std::net::Ipv4Addr>().is_ok(), "{ip}");
        }
    }

    #[test]
    fn the_user_folder_is_named_for_the_game() {
        let dir = super::user_dir().expect("a home or appdata folder");
        assert!(dir.to_string_lossy().to_lowercase().contains("tropa"));
    }
}
