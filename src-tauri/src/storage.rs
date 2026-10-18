//! In-memory clip store persisted to a JSON file (pure Rust, no C deps).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, RwLock};

use crate::clipboard::CapturedClip;

#[derive(Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: i64,
    pub kind: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub image_path: Option<String>,
    pub source_app: Option<String>,
    pub preview: String,
    pub char_count: i64,
    pub byte_size: i64,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub content_hash: String,
    #[serde(default)]
    pub pinned: bool,
    /// Whether a `thumb_<id>.png` exists next to the image. The thumbnail bytes
    /// themselves are never stored here: a base64 blob per record made
    /// `clips.json` and every list refresh megabytes large.
    #[serde(default)]
    pub has_thumb: bool,
    /// Only read, never written: 0.2.0 stored thumbnails inline as base64 and
    /// `Store::open` migrates them to files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thumb_base64: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct ClipSummary {
    pub id: i64,
    pub kind: String,
    pub preview: String,
    pub source_app: Option<String>,
    pub char_count: i64,
    pub byte_size: i64,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub pinned: bool,
    /// The list only says *that* a thumbnail exists; the UI fetches the bytes
    /// through `get_thumb` for the rows it actually shows.
    pub has_thumb: bool,
}

#[derive(Clone, Serialize)]
pub struct ClipDetail {
    pub id: i64,
    pub kind: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub image_base64: Option<String>,
    pub source_app: Option<String>,
    pub preview: String,
    pub char_count: i64,
    pub byte_size: i64,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub pinned: bool,
}

#[derive(Clone, Serialize)]
pub struct Stats {
    pub count: usize,
    pub total_bytes: u64,
    pub image_count: usize,
}

pub struct Store {
    clips: RwLock<Vec<Clip>>,
    next_id: AtomicI64,
    clips_path: PathBuf,
    images_dir: PathBuf,
    write_lock: Mutex<()>,
}

/// Deterministic FNV-1a 64-bit hash over the primary clip content, for dedup.
pub fn content_hash(
    kind: &str,
    text: Option<&str>,
    html: Option<&str>,
    rtf: Option<&str>,
    png: Option<&[u8]>,
) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    eat(kind.as_bytes());
    eat(&[0]);
    if let Some(t) = text {
        eat(t.as_bytes());
    }
    eat(&[0]);
    if let Some(x) = html {
        eat(x.as_bytes());
    }
    eat(&[0]);
    if let Some(r) = rtf {
        eat(r.as_bytes());
    }
    eat(&[0]);
    if let Some(p) = png {
        eat(p);
    }
    format!("{:016x}", h)
}

impl Store {
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let images_dir = data_dir.join("images");
        std::fs::create_dir_all(&images_dir).map_err(|e| e.to_string())?;
        let clips_path = data_dir.join("clips.json");

        let mut clips: Vec<Clip> = Vec::new();
        if clips_path.exists() {
            if let Ok(text) = std::fs::read_to_string(&clips_path) {
                clips = serde_json::from_str(&text).unwrap_or_default();
            }
        }
        let next_id = clips.iter().map(|c| c.id).max().unwrap_or(0) + 1;

        let store = Self {
            clips: RwLock::new(clips),
            next_id: AtomicI64::new(next_id),
            clips_path,
            images_dir,
            write_lock: Mutex::new(()),
        };
        store.migrate_inline_thumbnails();
        Ok(store)
    }

    /// 0.2.0 kept thumbnails as base64 in `clips.json`. Move them into
    /// `thumb_<id>.png` so the file stops growing with the history, and rewrite
    /// the JSON once if anything moved.
    fn migrate_inline_thumbnails(&self) {
        use base64::Engine;
        let mut changed = false;
        {
            let mut clips = self.clips.write().unwrap();
            for clip in clips.iter_mut() {
                let Some(encoded) = clip.thumb_base64.take() else {
                    continue;
                };
                changed = true;
                // The 0.2.0 field held base64 text; the file on disk holds PNG
                // bytes, which is what `thumb()` re-encodes on the way out.
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&encoded) {
                    if std::fs::write(self.thumb_path(clip.id), &bytes).is_ok() {
                        clip.has_thumb = true;
                    }
                }
            }
        }
        if changed {
            self.persist();
        }
    }

    fn thumb_path(&self, id: i64) -> PathBuf {
        self.images_dir.join(format!("thumb_{}.png", id))
    }

    fn remove_thumb(&self, id: i64) {
        let _ = std::fs::remove_file(self.thumb_path(id));
    }

    /// Read the stored thumbnail for `id` as base64, for the list to fetch lazily.
    pub fn thumb(&self, id: i64) -> Option<String> {
        {
            let clips = self.clips.read().unwrap();
            let clip = clips.iter().find(|c| c.id == id)?;
            if !clip.has_thumb {
                return None;
            }
        }
        use base64::Engine;
        let bytes = std::fs::read(self.thumb_path(id)).ok()?;
        Some(base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    pub fn insert(&self, cap: CapturedClip, retention_hours: Option<u64>) -> Option<ClipSummary> {
        let hash = content_hash(
            &cap.kind,
            cap.text.as_deref(),
            cap.html.as_deref(),
            cap.rtf.as_deref(),
            cap.png_bytes.as_deref(),
        );

        // Dedup: only skip when the most recent clip has identical content
        // (guards against the same copy action being captured twice). Re-copied
        // content is allowed to re-enter after a different clip, so it is not
        // silently dropped from the history.
        {
            let clips = self.clips.read().unwrap();
            if clips
                .last()
                .map(|c| c.content_hash == hash)
                .unwrap_or(false)
            {
                return None;
            }
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let created_at = now_ms();
        let expires_at = retention_hours.map(|h| created_at + (h * 3_600_000) as i64);

        let image_path = match &cap.png_bytes {
            Some(png) => {
                let fname = format!("{}_{}.png", id, created_at);
                let full = self.images_dir.join(&fname);
                if std::fs::write(&full, png).is_ok() {
                    Some(fname)
                } else {
                    None
                }
            }
            None => None,
        };

        // The thumbnail is written next to the image; only a flag is stored.
        let has_thumb = cap
            .png_bytes
            .as_deref()
            .and_then(make_thumbnail)
            .map(|png| std::fs::write(self.thumb_path(id), png).is_ok())
            .unwrap_or(false);

        let clip = Clip {
            id,
            kind: cap.kind.clone(),
            text: cap.text,
            html: cap.html,
            rtf: cap.rtf,
            image_path,
            source_app: cap.source_app,
            preview: cap.preview.clone(),
            char_count: cap.char_count,
            byte_size: cap.byte_size,
            created_at,
            expires_at,
            content_hash: hash,
            pinned: false,
            has_thumb,
            thumb_base64: None,
        };
        let summary = summary_of(&clip);

        self.clips.write().unwrap().push(clip);
        self.persist();
        Some(summary)
    }

    pub fn list(
        &self,
        limit: usize,
        offset: usize,
        kind: Option<&str>,
        search: Option<&str>,
    ) -> Vec<ClipSummary> {
        let clips = self.clips.read().unwrap();
        let search_lc = search.map(|s| s.to_lowercase());
        let mut matched: Vec<&Clip> = clips
            .iter()
            .filter(|c| match kind {
                Some(k) if !k.is_empty() => c.kind == k,
                _ => true,
            })
            .filter(|c| match &search_lc {
                Some(q) if !q.is_empty() => {
                    c.preview.to_lowercase().contains(q)
                        || c.text
                            .as_ref()
                            .map(|t| t.to_lowercase().contains(q))
                            .unwrap_or(false)
                }
                _ => true,
            })
            .collect();
        matched.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.created_at.cmp(&a.created_at))
        });
        matched
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(summary_of)
            .collect()
    }

    pub fn get(&self, id: i64) -> Option<ClipDetail> {
        let clips = self.clips.read().unwrap();
        let clip = clips.iter().find(|c| c.id == id)?;
        let image_base64 = match &clip.image_path {
            Some(p) => {
                let full = self.images_dir.join(p);
                use base64::Engine;
                std::fs::read(full)
                    .ok()
                    .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            }
            None => None,
        };
        Some(ClipDetail {
            id: clip.id,
            kind: clip.kind.clone(),
            text: clip.text.clone(),
            html: clip.html.clone(),
            rtf: clip.rtf.clone(),
            image_base64,
            source_app: clip.source_app.clone(),
            preview: clip.preview.clone(),
            char_count: clip.char_count,
            byte_size: clip.byte_size,
            created_at: clip.created_at,
            expires_at: clip.expires_at,
            pinned: clip.pinned,
        })
    }

    pub fn delete(&self, id: i64) -> bool {
        let mut clips = self.clips.write().unwrap();
        if let Some(pos) = clips.iter().position(|c| c.id == id) {
            let clip = clips.remove(pos);
            if let Some(p) = &clip.image_path {
                let _ = std::fs::remove_file(self.images_dir.join(p));
            }
            self.remove_thumb(id);
            drop(clips);
            self.persist();
            true
        } else {
            false
        }
    }

    pub fn clear(&self) -> usize {
        let mut clips = self.clips.write().unwrap();
        let n = clips.len();
        for c in clips.iter() {
            if let Some(p) = &c.image_path {
                let _ = std::fs::remove_file(self.images_dir.join(p));
            }
            self.remove_thumb(c.id);
        }
        clips.clear();
        drop(clips);
        self.persist();
        n
    }

    /// Move a record to the top (refresh its timestamp), used on copy-back.
    pub fn touch(&self, id: i64, retention_hours: Option<u64>) -> bool {
        let mut clips = self.clips.write().unwrap();
        let Some(pos) = clips.iter().position(|c| c.id == id) else {
            return false;
        };
        let mut clip = clips.remove(pos);
        clip.created_at = now_ms();
        clip.expires_at = retention_hours.map(|h| clip.created_at + (h * 3_600_000) as i64);
        clips.push(clip);
        drop(clips);
        self.persist();
        true
    }

    /// Pin/unpin a record. Pinned records never expire.
    pub fn set_pinned(&self, id: i64, pinned: bool, retention_hours: Option<u64>) -> bool {
        let now = now_ms();
        let mut clips = self.clips.write().unwrap();
        let Some(c) = clips.iter_mut().find(|c| c.id == id) else {
            return false;
        };
        c.pinned = pinned;
        if pinned {
            c.expires_at = None;
        } else {
            c.expires_at = retention_hours.map(|h| now + (h * 3_600_000) as i64);
        }
        drop(clips);
        self.persist();
        true
    }

    /// Remove expired records; returns how many were removed.
    pub fn cleanup_expired(&self) -> usize {
        let now = now_ms();
        let mut clips = self.clips.write().unwrap();
        let before = clips.len();
        let mut dropped_ids: Vec<i64> = Vec::new();
        clips.retain(|c| {
            let expired = !c.pinned && c.expires_at.map(|e| e < now).unwrap_or(false);
            if expired {
                if let Some(p) = &c.image_path {
                    let _ = std::fs::remove_file(self.images_dir.join(p));
                }
                dropped_ids.push(c.id);
            }
            !expired
        });
        let removed = before - clips.len();
        for id in dropped_ids {
            self.remove_thumb(id);
        }
        if removed > 0 {
            drop(clips);
            self.persist();
        }
        removed
    }

    /// Trim oldest records beyond `max` (pinned records are never trimmed).
    pub fn trim_to_max(&self, max: usize) {
        let mut clips = self.clips.write().unwrap();
        let mut removed = 0usize;
        while clips.len() > max {
            let pos = clips.iter().position(|c| !c.pinned);
            match pos {
                Some(p) => {
                    let c = clips.remove(p);
                    if let Some(img) = &c.image_path {
                        let _ = std::fs::remove_file(self.images_dir.join(img));
                    }
                    self.remove_thumb(c.id);
                    removed += 1;
                }
                None => break, // everything remaining is pinned
            }
        }
        if removed > 0 {
            drop(clips);
            self.persist();
        }
    }

    /// Recompute `expires_at` for all existing records from now. Pinned records
    /// keep their `None` expiry so "never expires" stays true in the data.
    pub fn apply_retention(&self, retention_hours: Option<u64>) {
        let now = now_ms();
        let mut clips = self.clips.write().unwrap();
        for c in clips.iter_mut() {
            if c.pinned {
                c.expires_at = None;
            } else {
                c.expires_at = retention_hours.map(|h| now + (h * 3_600_000) as i64);
            }
        }
        drop(clips);
        self.persist();
    }

    pub fn stats(&self) -> Stats {
        let clips = self.clips.read().unwrap();
        let count = clips.len();
        let total_bytes = clips.iter().map(|c| c.byte_size.max(0) as u64).sum();
        let image_count = clips.iter().filter(|c| c.kind == "image").count();
        Stats {
            count,
            total_bytes,
            image_count,
        }
    }

    fn persist(&self) {
        let _guard = self.write_lock.lock().unwrap();
        let clips = self.clips.read().unwrap();
        let json = serde_json::to_string(&*clips).unwrap_or_else(|_| "[]".to_string());
        let tmp = self.clips_path.with_extension("json.tmp");
        if std::fs::write(&tmp, &json).is_ok() {
            let _ = std::fs::rename(&tmp, &self.clips_path);
        }
    }
}

fn summary_of(c: &Clip) -> ClipSummary {
    ClipSummary {
        id: c.id,
        kind: c.kind.clone(),
        preview: c.preview.clone(),
        source_app: c.source_app.clone(),
        char_count: c.char_count,
        byte_size: c.byte_size,
        created_at: c.created_at,
        expires_at: c.expires_at,
        pinned: c.pinned,
        has_thumb: c.has_thumb,
    }
}

/// Downscale a PNG to at most 160px and return the encoded PNG bytes.
fn make_thumbnail(png: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let thumb = img.thumbnail(160, 160).to_rgba8();
    let mut out = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = image::codecs::png::PngEncoder::new(&mut out);
        encoder
            .write_image(
                thumb.as_raw(),
                thumb.width(),
                thumb.height(),
                image::ExtendedColorType::Rgba8,
            )
            .ok()?;
    }
    Some(out)
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::CapturedClip;

    fn text_clip(s: &str) -> CapturedClip {
        CapturedClip {
            kind: "text".to_string(),
            text: Some(s.to_string()),
            html: None,
            rtf: None,
            png_bytes: None,
            preview: s.to_string(),
            char_count: s.len() as i64,
            byte_size: s.len() as i64,
            source_app: None,
        }
    }

    #[test]
    fn dedup_only_consecutive() {
        let dir = std::env::temp_dir().join("clipcache_dedup_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(&dir).unwrap();
        assert!(store.insert(text_clip("alpha"), None).is_some());
        // consecutive identical copy is skipped
        assert!(store.insert(text_clip("alpha"), None).is_none());
        // different content inserts
        assert!(store.insert(text_clip("beta"), None).is_some());
        // same content as earlier is allowed again (not consecutive)
        assert!(store.insert(text_clip("alpha"), None).is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    fn png_clip() -> CapturedClip {
        use image::ImageEncoder;
        let rgba = vec![
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&rgba, 2, 2, image::ExtendedColorType::Rgba8)
            .unwrap();
        CapturedClip {
            kind: "image".to_string(),
            text: None,
            html: None,
            rtf: None,
            png_bytes: Some(png),
            preview: "图片 2×2".to_string(),
            char_count: 0,
            byte_size: 16,
            source_app: None,
        }
    }

    #[test]
    fn thumbnails_live_on_disk_not_in_summary() {
        let dir = std::env::temp_dir().join("clipcache_thumb_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(&dir).unwrap();
        let summary = store.insert(png_clip(), None).unwrap();
        assert!(summary.has_thumb, "insert must record that a thumb exists");

        // The summary no longer carries the bytes, but they are fetchable.
        let thumb = store.thumb(summary.id).expect("thumb must be readable");
        assert!(!thumb.is_empty());

        // ...and clips.json must not contain them.
        let json = std::fs::read_to_string(dir.join("clips.json")).unwrap();
        assert!(
            !json.contains("thumb_base64"),
            "thumbnail bytes leaked into clips.json"
        );
        assert!(
            json.len() < 4096,
            "clips.json should stay tiny, got {} bytes",
            json.len()
        );

        // Deleting the record deletes the thumbnail file too.
        assert!(store.delete(summary.id));
        assert!(store.thumb(summary.id).is_none());
        assert!(!dir
            .join("images")
            .join(format!("thumb_{}.png", summary.id))
            .exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_inline_thumbnails_are_migrated() {
        let dir = std::env::temp_dir().join("clipcache_migrate_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("images")).unwrap();
        // 0.2.0 shaped record: thumbnail bytes inline, no `has_thumb`.
        let legacy = r#"[{"id":7,"kind":"image","text":null,"html":null,"rtf":null,
            "image_path":null,"source_app":null,"preview":"x","char_count":0,
            "byte_size":0,"created_at":1,"expires_at":null,"content_hash":"h",
            "pinned":false,"thumb_base64":"QUJD"}]"#;
        std::fs::write(dir.join("clips.json"), legacy).unwrap();

        let store = Store::open(&dir).unwrap();
        let thumb = store.thumb(7).expect("migrated thumb must be readable");
        assert_eq!(thumb, "QUJD");
        // The rewritten file no longer carries the blob.
        let json = std::fs::read_to_string(dir.join("clips.json")).unwrap();
        assert!(
            !json.contains("thumb_base64"),
            "migration did not strip inline thumb"
        );
        assert!(json.contains("\"has_thumb\":true"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_retention_spares_pinned_records() {
        let dir = std::env::temp_dir().join("clipcache_pin_retention_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(&dir).unwrap();
        let a = store.insert(text_clip("pinned"), Some(1)).unwrap();
        let b = store.insert(text_clip("normal"), Some(1)).unwrap();
        assert!(store.set_pinned(a.id, true, Some(1)));

        store.apply_retention(Some(168));
        let clips = store.clips.read().unwrap();
        let pinned = clips.iter().find(|c| c.id == a.id).unwrap();
        let normal = clips.iter().find(|c| c.id == b.id).unwrap();
        assert_eq!(pinned.expires_at, None, "pinned must stay never-expire");
        assert!(
            normal.expires_at.is_some(),
            "normal record takes the new window"
        );
        drop(clips);
        std::fs::remove_dir_all(&dir).ok();
    }
}
