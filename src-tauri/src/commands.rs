//! Tauri IPC command handlers.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::State;

use crate::settings::Settings;
use crate::storage::{ClipDetail, ClipSummary, Stats};
use crate::AppState;

/// Largest page the UI may request. Kept in sync with `MAX_CLIPS_CEILING` so the
/// list and the `max_clips` setting cannot disagree.
pub const MAX_PAGE: usize = 1000;

#[tauri::command]
pub fn list_clips(
    state: State<'_, Arc<AppState>>,
    limit: Option<usize>,
    offset: Option<usize>,
    kind: Option<String>,
    search: Option<String>,
) -> Vec<ClipSummary> {
    state.store.list(
        limit.unwrap_or(200).min(MAX_PAGE),
        offset.unwrap_or(0),
        kind.as_deref(),
        search.as_deref(),
    )
}

#[tauri::command]
pub fn get_clip(state: State<'_, Arc<AppState>>, id: i64) -> Option<ClipDetail> {
    state.store.get(id)
}

/// Base64 PNG thumbnail for one record, fetched per visible row instead of
/// being pushed with every list refresh.
#[tauri::command]
pub fn get_thumb(state: State<'_, Arc<AppState>>, id: i64) -> Option<String> {
    state.store.thumb(id)
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

    // Suppress the listener for this exact clipboard change. The sequence
    // number must be read *after* the write: `SetClipboardData` bumps it.
    state
        .own_write_seq
        .store(crate::clipboard::clipboard_sequence(), Ordering::SeqCst);

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

/// Retention as sent by the UI. The old signature used `Option<u64>`, where
/// `None` meant *both* "leave it alone" and "keep forever" — so any caller that
/// omitted the field silently turned retention permanent. A string moves the
/// intent into the value: `null`/absent = unchanged, `"forever"` = never expire,
/// `"168"` = 168 hours.
fn parse_retention(raw: Option<&str>) -> Result<Option<Option<u64>>, String> {
    let Some(raw) = raw else {
        return Ok(None); // unchanged
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.eq_ignore_ascii_case("forever") {
        return Ok(Some(None));
    }
    let hours: u64 = raw.parse().map_err(|_| format!("无效的保留时间: {raw}"))?;
    if hours == 0 {
        return Err("保留时间必须大于 0".to_string());
    }
    Ok(Some(Some(hours)))
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, Arc<AppState>>,
    retention: Option<String>,
    max_clips: Option<usize>,
    theme: Option<String>,
    apply_existing: Option<bool>,
) -> Result<Settings, String> {
    let retention = parse_retention(retention.as_deref())?;

    let mut s = state.settings.write().unwrap();
    if let Some(r) = retention {
        s.retention_hours = r;
    }
    if let Some(m) = max_clips {
        // Keep the setting inside the range the list can actually page through.
        s.max_clips = m.clamp(10, MAX_PAGE);
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

    Ok(new)
}

#[tauri::command]
pub fn get_stats(state: State<'_, Arc<AppState>>) -> Stats {
    state.store.stats()
}

/// Clipboard format dump for diagnosing paste problems. Debug builds only:
/// in release the command is not registered at all.
#[cfg(debug_assertions)]
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

#[cfg(test)]
mod tests {
    use super::parse_retention;

    #[test]
    fn retention_distinguishes_unchanged_from_forever() {
        // Absent or blank: leave the current setting alone.
        assert_eq!(parse_retention(None), Ok(None));
        assert_eq!(parse_retention(Some("")), Ok(None));
        assert_eq!(parse_retention(Some("   ")), Ok(None));

        // Explicit "forever" is the only way to request never-expire.
        assert_eq!(parse_retention(Some("forever")), Ok(Some(None)));
        assert_eq!(parse_retention(Some("FOREVER")), Ok(Some(None)));

        // Numbers carry the window.
        assert_eq!(parse_retention(Some("168")), Ok(Some(Some(168))));
        assert_eq!(parse_retention(Some(" 24 ")), Ok(Some(Some(24))));

        // Zero and garbage are rejected rather than silently stored.
        assert!(parse_retention(Some("0")).is_err());
        assert!(parse_retention(Some("abc")).is_err());
        assert!(parse_retention(Some("-5")).is_err());
    }
}
