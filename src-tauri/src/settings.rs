//! User settings persisted to a JSON file.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    /// `None` means "keep forever".
    pub retention_hours: Option<u64>,
    pub max_clips: usize,
    pub theme: String, // "light" | "dark" | "system"
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            retention_hours: Some(168), // 7 days
            max_clips: 500,
            theme: "system".to_string(),
        }
    }
}

impl Settings {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("settings.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) {
        let path = data_dir.join("settings.json");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }
}
