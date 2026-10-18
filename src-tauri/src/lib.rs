//! ClipCache — a clean, elegant Windows clipboard manager.

mod clipboard;
mod commands;
mod dib;
mod settings;
mod storage;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use tauri::Manager;

pub struct AppState {
    pub store: storage::Store,
    pub settings: RwLock<settings::Settings>,
    /// Sequence number produced by our own clipboard write-back. The listener
    /// skips exactly that clipboard change; `0` means "nothing pending".
    ///
    /// Matching on the sequence number rather than on a content hash is what
    /// makes this correct: `write_to_clipboard` rewrites HTML (it inlines the
    /// PNG as a data URI), so the bytes on the clipboard do not hash to the
    /// stored fields and a content match would re-capture the clip.
    pub own_write_seq: AtomicU32,
    pub data_dir: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = resolve_data_dir(app);
            std::fs::create_dir_all(&data_dir)?;
            let store = storage::Store::open(&data_dir).map_err(std::io::Error::other)?;
            let settings = settings::Settings::load(&data_dir);

            let state = Arc::new(AppState {
                store,
                settings: RwLock::new(settings),
                own_write_seq: AtomicU32::new(0),
                data_dir: data_dir.clone(),
            });
            app.manage(state.clone());

            // Startup cleanup + background threads.
            let _ = state.store.cleanup_expired();
            spawn_listener(state.clone());
            spawn_cleanup(state.clone());

            Ok(())
        })
        .invoke_handler(build_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// The debug-only `get_clipboard_formats` command is registered in debug builds
/// and absent from release, so the shipped app exposes no diagnostics surface.
#[cfg(debug_assertions)]
fn build_handler<R: tauri::Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::list_clips,
        commands::get_clip,
        commands::get_thumb,
        commands::copy_to_clipboard,
        commands::delete_clip,
        commands::clear_all,
        commands::pin_clip,
        commands::get_settings,
        commands::update_settings,
        commands::get_stats,
        commands::get_clipboard_formats,
        commands::get_data_dir,
        commands::open_data_dir,
    ]
}

#[cfg(not(debug_assertions))]
fn build_handler<R: tauri::Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::list_clips,
        commands::get_clip,
        commands::get_thumb,
        commands::copy_to_clipboard,
        commands::delete_clip,
        commands::clear_all,
        commands::pin_clip,
        commands::get_settings,
        commands::update_settings,
        commands::get_stats,
        commands::get_data_dir,
        commands::open_data_dir,
    ]
}

/// Resolve the data directory: env override > app data dir > local fallback.
fn resolve_data_dir(app: &tauri::App) -> PathBuf {
    if let Some(p) = std::env::var_os("CLIPCACHE_DATA_DIR") {
        return PathBuf::from(p);
    }
    app.path().app_data_dir().unwrap_or_else(|_| {
        std::env::current_dir()
            .unwrap_or_default()
            .join("clipcache-data")
    })
}

/// Poll the clipboard sequence number and capture on change.
fn spawn_listener(state: Arc<AppState>) {
    std::thread::spawn(move || {
        let mut last_seq = clipboard::clipboard_sequence();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let seq = clipboard::clipboard_sequence();
            if seq == last_seq {
                continue;
            }
            last_seq = seq;

            // Skip exactly the clipboard change our own write-back produced.
            // Consume the marker either way so a stale value cannot suppress a
            // later user copy.
            let expected = state.own_write_seq.swap(0, Ordering::SeqCst);
            if expected != 0 && expected == seq {
                continue;
            }

            let Some(cap) = clipboard::capture() else {
                continue;
            };

            let (retention, max_clips) = {
                let s = state.settings.read().unwrap();
                (s.retention_hours, s.max_clips)
            };
            state.store.insert(cap, retention);
            state.store.trim_to_max(max_clips);
        }
    });
}

/// Periodically remove expired records.
fn spawn_cleanup(state: Arc<AppState>) {
    std::thread::spawn(move || {
        let _ = state.store.cleanup_expired();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(300));
            let _ = state.store.cleanup_expired();
        }
    });
}
