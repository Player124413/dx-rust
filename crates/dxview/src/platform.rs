//! Platform paths and Android lifecycle. Desktop keeps its portable "files beside the executable" layout;
//! Android uses the app-private files directory supplied by GameActivity (never a hard-coded /sdcard path).
use bevy::prelude::*;
use bevy::window::AppLifecycle;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init() {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(target_os = "android")]
    {
        // Bevy's Android entry point has already published AndroidApp before calling main.
        let _ = bevy::log::info!("DX RUST data directory: {}", dir.display());
    }
}

pub fn data_dir() -> PathBuf {
    DATA_DIR.get_or_init(|| {
        #[cfg(target_os = "android")]
        {
            if let Some(app) = bevy::android::ANDROID_APP.get() {
                if let Some(p) = app.internal_data_path() { return PathBuf::from(p); }
            }
            // A missing internal path is a broken host; do not silently write into the process CWD.
            panic!("Android did not provide an internal data directory");
        }
        #[cfg(not(target_os = "android"))]
        {
            std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| PathBuf::from("."))
        }
    }).clone()
}

pub fn read_kv(path: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(path).unwrap_or_default().lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
        .collect()
}

pub fn kv_value(path: &Path, key: &str) -> Option<String> {
    read_kv(path).into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Replace a file atomically. The temporary file is in the same directory, so rename is atomic on both
/// desktop filesystems and Android's ext4/f2fs app data partitions.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let tmp = path.with_extension(format!("{}.tmp", path.extension().and_then(|s| s.to_str()).unwrap_or("dx")));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Game directory chosen by the Android launcher. On desktop the original argument/launcher path still wins.
pub fn android_game_root() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        let p = kv_value(&data_dir().join("launcher.txt"), "GamePath")?;
        let p = PathBuf::from(p);
        unreal_pkg::gamedir::is_game_dir(&p).then_some(p)
    }
    #[cfg(not(target_os = "android"))]
    { None }
}

/// Physical-pixel insets written by the Android launcher for the current landscape surface.
pub fn safe_insets() -> (f32, f32, f32, f32) {
    let p = data_dir().join("surface.txt");
    let get = |k: &str| kv_value(&p, k).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(0.0);
    (get("InsetLeft"), get("InsetTop"), get("InsetRight"), get("InsetBottom"))
}

#[derive(Resource, Default)]
pub struct Lifecycle { pub suspended: bool, pub paused_before: bool }

pub fn lifecycle_system(
    mut events: MessageReader<AppLifecycle>,
    mut state: ResMut<Lifecycle>,
    mut time: ResMut<Time<Virtual>>,
    mut touch: ResMut<crate::touch::TouchState>,
) {
    for event in events.read() {
        match event {
            AppLifecycle::WillSuspend | AppLifecycle::Suspended | AppLifecycle::Idle => {
                if !state.suspended { state.paused_before = time.is_paused(); }
                state.suspended = true;
                time.pause();
                touch.reset();
                crate::sound::set_background_mute(true);
                crate::settings::save();
            }
            AppLifecycle::WillResume | AppLifecycle::Running => {
                if state.suspended && !state.paused_before && !crate::menus::open() && !crate::touch::editor_open() { time.unpause(); }
                state.suspended = false;
                crate::sound::set_background_mute(false);
                touch.reset();
            }
        }
    }
}
