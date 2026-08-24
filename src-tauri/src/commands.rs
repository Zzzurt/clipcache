//! Tauri IPC command handlers.

use std::sync::Arc;

use tauri::State;

use crate::settings::Settings;
use crate::storage::{self, ClipDetail, ClipSummary, Stats};
use crate::AppState;

#[tauri::command]
pub fn list_clips(
    state: State<'_, Arc<AppState>>,
    limit: Option<usize>,
    offset: Option<usize>,
    kind: Option<String>,
    search: Option<String>,
) -> Vec<ClipSummary> {
    state
        .store
        .list(
            limit.unwrap_or(200).min(1000),
            offset.unwrap_or(0),
            kind.as_deref(),
            search.as_deref(),
        )
}

#[tauri::command]
pub fn get_clip(state: State<'_, Arc<AppState>>, id: i64) -> Option<ClipDetail> {
    state.store.get(id)
}

#[tauri::command]
pub fn copy_to_clipboard(state: State<'_, Arc<AppState>>, id: i64) -> Result<(), String> {
    let detail = state.store.get(id).ok_or("记录不存在")?;

    use base64::Engine;
    let png_bytes = detail
        .image_base64
        .as_ref()
        .map(|b| base64::engine::general_purpose::STANDARD.decode(b))
        .transpose()
        .map_err(|e| e.to_string())?;

    let ok = crate::clipboard::write_to_clipboard(
        detail.text.as_deref(),
        detail.html.as_deref(),
        detail.rtf.as_deref(),
        png_bytes.as_deref(),
    );
    if !ok {
        return Err("写入剪贴板失败".to_string());
    }

    // Mark our own write so the listener does not re-capture it.
    let hash = storage::content_hash(
        &detail.kind,
        detail.text.as_deref(),
        detail.html.as_deref(),
        detail.rtf.as_deref(),
        png_bytes.as_deref(),
    );
    *state.last_written_hash.lock().unwrap() = Some(hash);

    // Move the record to the top, like Windows clipboard history.
    let retention = state.settings.read().unwrap().retention_hours;
    state.store.touch(id, retention);

    Ok(())
}

#[tauri::command]
pub fn delete_clip(state: State<'_, Arc<AppState>>, id: i64) -> bool {
    state.store.delete(id)
}

#[tauri::command]
pub fn clear_all(state: State<'_, Arc<AppState>>) -> usize {
    state.store.clear()
}

#[tauri::command]
pub fn pin_clip(state: State<'_, Arc<AppState>>, id: i64, pinned: bool) -> bool {
    let retention = state.settings.read().unwrap().retention_hours;
    state.store.set_pinned(id, pinned, retention)
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Settings {
    state.settings.read().unwrap().clone()
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, Arc<AppState>>,
    retention_hours: Option<u64>,
    max_clips: Option<usize>,
    theme: Option<String>,
    apply_existing: Option<bool>,
) -> Settings {
    let mut s = state.settings.write().unwrap();
    s.retention_hours = retention_hours; // None => keep forever
    if let Some(m) = max_clips {
        s.max_clips = m;
    }
    if let Some(t) = theme {
        s.theme = t;
    }
    let new = s.clone();
    drop(s);

    new.save(&state.data_dir);

    if apply_existing.unwrap_or(false) {
        state.store.apply_retention(new.retention_hours);
    }
    state.store.trim_to_max(new.max_clips);

    new
}

#[tauri::command]
pub fn get_stats(state: State<'_, Arc<AppState>>) -> Stats {
    state.store.stats()
}

#[tauri::command]
pub fn get_clipboard_formats() -> Vec<serde_json::Value> {
    crate::clipboard::clipboard_formats()
        .into_iter()
        .map(|(id, name, size)| serde_json::json!({ "id": id, "name": name, "size": size }))
        .collect()
}

#[tauri::command]
pub fn get_data_dir(state: State<'_, Arc<AppState>>) -> String {
    state.data_dir.to_string_lossy().into_owned()
}

#[tauri::command]
pub fn open_data_dir(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(&state.data_dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
