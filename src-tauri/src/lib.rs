//! ClipCache — a clean, elegant Windows clipboard manager.

mod clipboard;
mod commands;
mod dib;
mod settings;
mod storage;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use tauri::Manager;

pub struct AppState {
    pub store: storage::Store,
    pub settings: RwLock<settings::Settings>,
    pub last_written_hash: Mutex<Option<String>>,
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
                last_written_hash: Mutex::new(None),
                data_dir: data_dir.clone(),
            });
            app.manage(state.clone());

            // Startup cleanup + background threads.
            let _ = state.store.cleanup_expired();
            spawn_listener(state.clone());
            spawn_cleanup(state.clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_clips,
            commands::get_clip,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Resolve the data directory: env override > app data dir > local fallback.
fn resolve_data_dir(app: &tauri::App) -> PathBuf {
    if let Some(p) = std::env::var_os("CLIPCACHE_DATA_DIR") {
        return PathBuf::from(p);
    }
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join("clipcache-data"))
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

            let Some(cap) = clipboard::capture() else {
                continue;
            };
            let hash = storage::content_hash(
                &cap.kind,
                cap.text.as_deref(),
                cap.html.as_deref(),
                cap.rtf.as_deref(),
                cap.png_bytes.as_deref(),
            );

            // Skip our own write-back; always consume the marker.
            let is_own = {
                let mut lw = state.last_written_hash.lock().unwrap();
                let m = lw.as_deref() == Some(hash.as_str());
                *lw = None;
                m
            };
            if is_own {
                continue;
            }

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
