//! Clipboard capture and write-back via raw Win32 FFI (no external crate).

use crate::dib;

const CF_TEXT: u32 = 1;
const CF_BITMAP: u32 = 2;
const CF_DIB: u32 = 8;
const CF_DIBV5: u32 = 17;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;
const GMEM_MOVEABLE: u32 = 0x0002;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hwnd: isize) -> i32;
    fn CloseClipboard() -> i32;
    fn GetClipboardData(format: u32) -> isize;
    fn SetClipboardData(format: u32, mem: isize) -> isize;
    fn EmptyClipboard() -> i32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn RegisterClipboardFormatW(name: *const u16) -> u32;
    fn GetClipboardSequenceNumber() -> u32;
    fn GetClipboardOwner() -> isize;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    fn EnumClipboardFormats(format: u32) -> u32;
    fn GetClipboardFormatNameW(format: u32, name: *mut u16, size: i32) -> i32;
    fn GetDC(hwnd: isize) -> isize;
    fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: isize) -> isize;
    fn SelectObject(hdc: isize, obj: isize) -> isize;
    fn GetObjectW(obj: isize, size: i32, out: *mut u8) -> i32;
    fn GetDIBits(
        hdc: isize,
        hbmp: isize,
        start: u32,
        lines: u32,
        bits: *mut u8,
        bmi: *mut u8,
        usage: u32,
    ) -> i32;
    fn DeleteDC(hdc: isize) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
    fn GlobalLock(mem: isize) -> *mut u8;
    fn GlobalUnlock(mem: isize) -> i32;
    fn GlobalSize(mem: isize) -> usize;
    fn GlobalFree(mem: isize) -> isize;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn QueryFullProcessImageNameW(proc: isize, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    fn CloseHandle(obj: isize) -> i32;
    fn MultiByteToWideChar(
        code_page: u32,
        flags: u32,
        mb_str: *const u8,
        mb_len: i32,
        wide_str: *mut u16,
        wide_len: i32,
    ) -> i32;
}

#[derive(Clone)]
pub struct CapturedClip {
    pub kind: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub png_bytes: Option<Vec<u8>>,
    pub preview: String,
    pub char_count: i64,
    pub byte_size: i64,
    pub source_app: Option<String>,
}

pub fn clipboard_sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Enumerate the current clipboard formats (id, name, byte size). For debugging.
pub fn clipboard_formats() -> Vec<(u32, String, usize)> {
    unsafe {
        let mut out = Vec::new();
        if OpenClipboard(0) == 0 {
            return out;
        }
        let mut fmt = 0u32;
        loop {
            fmt = EnumClipboardFormats(fmt);
            if fmt == 0 {
                break;
            }
            let name = format_name(fmt);
            let h = GetClipboardData(fmt);
            let size = if h == 0 { 0 } else { GlobalSize(h) };
            out.push((fmt, name, size));
        }
        CloseClipboard();
        out
    }
}

fn format_name(fmt: u32) -> String {
    let std_name = match fmt {
        1 => "CF_TEXT",
        2 => "CF_BITMAP",
        3 => "CF_METAFILEPICT",
        4 => "CF_SYLK",
        5 => "CF_DIF",
        6 => "CF_TIFF",
        7 => "CF_OEMTEXT",
        8 => "CF_DIB",
        9 => "CF_PALETTE",
        10 => "CF_PENDATA",
        11 => "CF_RIFF",
        12 => "CF_WAVE",
        13 => "CF_UNICODETEXT",
        14 => "CF_ENHMETAFILE",
        15 => "CF_HDROP",
        16 => "CF_LOCALE",
        17 => "CF_DIBV5",
        _ => "",
    };
    if !std_name.is_empty() {
        return std_name.to_string();
    }
    unsafe {
        let mut buf = vec![0u16; 256];
        let n = GetClipboardFormatNameW(fmt, buf.as_mut_ptr(), 256);
        if n > 0 {
            String::from_utf16_lossy(&buf[..n as usize])
        } else {
            format!("#{}", fmt)
        }
    }
}

/// Capture the current clipboard contents (text / html / rtf / image).
pub fn capture() -> Option<CapturedClip> {
    unsafe {
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(0) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !opened {
            return None;
        }
        let result = capture_inner();
        CloseClipboard();
        result
    }
}

unsafe fn capture_inner() -> Option<CapturedClip> {
    let source_app = owner_process_name();
    let png_fmt = reg("PNG");
    let html_fmt = reg("HTML Format");
    let rtf_fmt = reg("Rich Text Format");

    // Gather every available representation (a single copy may carry text + image).
    let text = if IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 {
        read_format(CF_UNICODETEXT).map(|b| utf16_to_string(&b))
    } else if IsClipboardFormatAvailable(CF_TEXT) != 0 {
        read_format(CF_TEXT).map(|b| ansi_to_string(&b))
    } else {
        None
    };

    let html = if html_fmt != 0 && IsClipboardFormatAvailable(html_fmt) != 0 {
        read_format(html_fmt).map(|b| parse_cf_html(&b))
    } else {
        None
    };

    let rtf = if rtf_fmt != 0 && IsClipboardFormatAvailable(rtf_fmt) != 0 {
        read_format(rtf_fmt).map(|b| rtf_to_string(&b))
    } else {
        None
    };

    // Image, in priority order: PNG -> DIB/DIBV5 -> files from Explorer -> embedded in HTML.
    let mut image: Option<(Vec<u8>, u32, u32)> = None;
    if png_fmt != 0 && IsClipboardFormatAvailable(png_fmt) != 0 {
        if let Some(bytes) = read_format(png_fmt) {
            if !bytes.is_empty() {
                let dims = png_dimensions(&bytes).unwrap_or((0, 0));
                image = Some((bytes, dims.0, dims.1));
            }
        }
    }
    if image.is_none() {
        for fmt in [CF_DIBV5, CF_DIB] {
            if IsClipboardFormatAvailable(fmt) != 0 {
                if let Some(dib_bytes) = read_format(fmt) {
                    if let Some((w, h, rgba)) = dib::dib_to_rgba(&dib_bytes) {
                        if let Some(png) = encode_png(w, h, &rgba) {
                            image = Some((png, w, h));
                            break;
                        }
                    }
                }
            }
        }
    }
    // CF_BITMAP (device-dependent bitmap handle) via GDI GetDIBits
    if image.is_none() {
        if IsClipboardFormatAvailable(CF_BITMAP) != 0 {
            let hbmp = GetClipboardData(CF_BITMAP);
            if hbmp != 0 {
                if let Some(dib_bytes) = hbitmap_to_dib(hbmp) {
                    if let Some((w, h, rgba)) = dib::dib_to_rgba(&dib_bytes) {
                        if let Some(png) = encode_png(w, h, &rgba) {
                            image = Some((png, w, h));
                        }
                    }
                }
            }
        }
    }
    if image.is_none() {
        if let Some(paths) = read_hdrop_paths() {
            for path in paths {
                if let Some(png) = image_file_to_png(&path) {
                    image = Some(png);
                    break;
                }
            }
        }
    }
    if image.is_none() {
        if let Some(h) = &html {
            if let Some(png) = extract_html_image(h) {
                image = Some(png);
            }
        }
    }

    if text.is_none() && html.is_none() && rtf.is_none() && image.is_none() {
        return None;
    }

    let kind = if image.is_some() {
        "image"
    } else if html.is_some() {
        "html"
    } else if rtf.is_some() {
        "rtf"
    } else {
        "text"
    }
    .to_string();

    let preview = {
        let base = text
            .clone()
            .or_else(|| html.as_ref().map(|h| strip_html(h)))
            .unwrap_or_default();
        if base.trim().is_empty() {
            image
                .as_ref()
                .map(|(_, w, h)| format!("图片 {}×{}", w, h))
                .unwrap_or_default()
        } else {
            make_preview(&base)
        }
    };

    let char_count = text
        .as_ref()
        .map(|t| t.chars().count() as i64)
        .or_else(|| html.as_ref().map(|h| h.chars().count() as i64))
        .unwrap_or(0);
    let byte_size = text.as_ref().map(|t| t.len() as i64).unwrap_or(0)
        + html.as_ref().map(|h| h.len() as i64).unwrap_or(0)
        + rtf.as_ref().map(|r| r.len() as i64).unwrap_or(0)
        + image.as_ref().map(|(png, _, _)| png.len() as i64).unwrap_or(0);

    Some(CapturedClip {
        kind,
        text,
        html,
        rtf,
        png_bytes: image.map(|(png, _, _)| png),
        preview,
        char_count,
        byte_size,
        source_app,
    })
}

/// Write a captured clip back into the clipboard (all representations it carries).
pub fn write_to_clipboard(
    text: Option<&str>,
    html: Option<&str>,
    rtf: Option<&str>,
    png: Option<&[u8]>,
) -> bool {
    unsafe {
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(0) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !opened {
            return false;
        }
        let ok = EmptyClipboard() != 0;
        if ok {
            if let Some(t) = text {
                let data = utf8_to_utf16_null(t);
                set_global(CF_UNICODETEXT, &data);
            }
            if let Some(write_html) = build_write_html(text, html, png) {
                let cf = build_cf_html(&write_html);
                set_global(reg("HTML Format"), &cf);
            }
            if let Some(r) = rtf {
                set_global(reg("Rich Text Format"), r.as_bytes());
            }
            // For combined text+image clips, skip bitmap formats so rich
            // targets (chat apps etc.) use the HTML path and paste both.
            let combined = text.is_some() && png.is_some();
            if !combined {
                if let Some(png_bytes) = png {
                    if let Some((w, h, rgba)) = decode_png(png_bytes) {
                        let dib_bytes = dib::rgba_to_dib(w, h, &rgba);
                        set_global(CF_DIB, &dib_bytes);
                    }
                    set_global(reg("PNG"), png_bytes);
                }
            }
        }
        CloseClipboard();
        ok
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

unsafe fn reg(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    RegisterClipboardFormatW(wide.as_ptr())
}

unsafe fn read_format(format: u32) -> Option<Vec<u8>> {
    let h = GetClipboardData(format);
    if h == 0 {
        return None;
    }
    let size = GlobalSize(h);
    if size == 0 {
        return None;
    }
    let ptr = GlobalLock(h);
    if ptr.is_null() {
        return None;
    }
    let data = std::slice::from_raw_parts(ptr, size).to_vec();
    GlobalUnlock(h);
    Some(data)
}

unsafe fn set_global(format: u32, data: &[u8]) -> bool {
    if format == 0 {
        return false;
    }
    let h = GlobalAlloc(GMEM_MOVEABLE, data.len());
    if h == 0 {
        return false;
    }
    let ptr = GlobalLock(h);
    if ptr.is_null() {
        GlobalFree(h);
        return false;
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
    GlobalUnlock(h);
    if SetClipboardData(format, h) == 0 {
        GlobalFree(h);
        false
    } else {
        true
    }
}

unsafe fn owner_process_name() -> Option<String> {
    let owner = GetClipboardOwner();
    if owner == 0 {
        return None;
    }
    let mut pid: u32 = 0;
    if GetWindowThreadProcessId(owner, &mut pid) == 0 || pid == 0 {
        return None;
    }
    let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if proc == 0 {
        return None;
    }
    let mut buf = vec![0u16; 260];
    let mut size = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(proc, 0, buf.as_mut_ptr(), &mut size);
    CloseHandle(proc);
    if ok == 0 || size == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..size as usize]);
    let name = path.rsplit(['\\', '/']).next().unwrap_or(&path);
    let stem = name.strip_suffix(".exe").unwrap_or(name);
    if stem.is_empty() {
        None
    } else {
        Some(stem.to_string())
    }
}

unsafe fn read_hdrop_paths() -> Option<Vec<std::path::PathBuf>> {
    if IsClipboardFormatAvailable(CF_HDROP) == 0 {
        return None;
    }
    let data = read_format(CF_HDROP)?;
    if data.len() < 20 {
        return None;
    }
    let pfiles = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let fwide = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) != 0;
    if pfiles >= data.len() {
        return None;
    }
    let list = &data[pfiles..];
    let mut paths = Vec::new();
    if fwide {
        let mut i = 0usize;
        while i + 2 <= list.len() {
            let mut end = i;
            while end + 2 <= list.len() && !(list[end] == 0 && list[end + 1] == 0) {
                end += 2;
            }
            if end == i {
                break;
            }
            let units: Vec<u16> = list[i..end]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            paths.push(std::path::PathBuf::from(String::from_utf16_lossy(&units)));
            i = end + 2;
        }
    } else {
        let mut i = 0usize;
        while i < list.len() {
            let mut end = i;
            while end < list.len() && list[end] != 0 {
                end += 1;
            }
            if end == i {
                break;
            }
            paths.push(std::path::PathBuf::from(ansi_to_string(&list[i..end])));
            i = end + 1;
        }
    }
    Some(paths)
}

/// Convert an HBITMAP (CF_BITMAP) to a DIB byte buffer via GDI GetDIBits.
unsafe fn hbitmap_to_dib(hbmp: isize) -> Option<Vec<u8>> {
    if hbmp == 0 {
        return None;
    }
    let mut bm = [0u8; 32]; // BITMAP (x64 layout)
    if GetObjectW(hbmp, 32, bm.as_mut_ptr()) == 0 {
        return None;
    }
    let w = i32::from_le_bytes([bm[4], bm[5], bm[6], bm[7]]);
    let h = i32::from_le_bytes([bm[8], bm[9], bm[10], bm[11]]);
    if w <= 0 || h <= 0 {
        return None;
    }
    let hdc = GetDC(0);
    if hdc == 0 {
        return None;
    }
    let memdc = CreateCompatibleDC(hdc);
    if memdc == 0 {
        ReleaseDC(0, hdc);
        return None;
    }
    let old = SelectObject(memdc, hbmp);
    let w = w as u32;
    let h = h as u32;
    let row = (w as usize) * 4;
    let mut dib = vec![0u8; 40 + row * h as usize];
    dib[0..4].copy_from_slice(&40u32.to_le_bytes()); // biSize
    dib[4..8].copy_from_slice(&(w as i32).to_le_bytes()); // biWidth
    dib[8..12].copy_from_slice(&(h as i32).to_le_bytes()); // biHeight (bottom-up)
    dib[12..14].copy_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib[14..16].copy_from_slice(&32u16.to_le_bytes()); // biBitCount
    dib[20..24].copy_from_slice(&((row * h as usize) as u32).to_le_bytes()); // biSizeImage
    let res = GetDIBits(memdc, hbmp, 0, h, dib[40..].as_mut_ptr(), dib.as_mut_ptr(), 0);
    SelectObject(memdc, old);
    DeleteDC(memdc);
    ReleaseDC(0, hdc);
    if res == 0 {
        return None;
    }
    Some(dib)
}

/// Read an image file from disk and return PNG bytes + dimensions.
fn image_file_to_png(path: &std::path::Path) -> Option<(Vec<u8>, u32, u32)> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 80 * 1024 * 1024 {
        return None; // skip absurdly large files to avoid OOM
    }
    let bytes = std::fs::read(path).ok()?;
    bytes_to_png(&bytes)
}

/// Convert arbitrary image bytes (PNG/JPEG/BMP/GIF/WebP) to PNG bytes + dimensions.
fn bytes_to_png(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let dims = png_dimensions(bytes).unwrap_or((0, 0));
        return Some((bytes.to_vec(), dims.0, dims.1));
    }
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let png = encode_png(w, h, rgba.as_raw())?;
    Some((png, w, h))
}

/// ASCII-only lowercase (byte-length preserving, keeps slicing indices aligned).
fn ascii_lower(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_uppercase() { (c as u8 + 32) as char } else { c })
        .collect()
}

/// Extract an image from HTML: `<img src=...>` data: URLs or local file paths.
fn extract_html_image(html: &str) -> Option<(Vec<u8>, u32, u32)> {
    let lower = ascii_lower(html);
    let mut rest = html;
    let mut rest_lower = lower.as_str();
    while let Some(pos) = rest_lower.find("<img") {
        let after = &rest[pos + 4..];
        let after_lower = &rest_lower[pos + 4..];
        let end = after_lower.find('>').unwrap_or(after.len());
        let tag = &after[..end];
        rest = &after[end..];
        rest_lower = &after_lower[end..];
        if let Some(src) = extract_src(tag) {
            if let Some(png) = html_src_to_png(&src) {
                return Some(png);
            }
        }
    }
    // Fallback: any `data:image/...` URL anywhere in the html.
    embedded_image_to_png(html)
}

fn extract_src(tag: &str) -> Option<String> {
    let lower = ascii_lower(tag);
    let pos = lower.find("src=")?;
    let after = tag[pos + 4..].trim_start();
    let mut chars = after.chars();
    let quote = chars.next()?;
    if quote == '"' || quote == '\'' {
        let rest = &after[1..];
        let end = rest.find(quote)?;
        Some(rest[..end].to_string())
    } else {
        let end = after
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(after.len());
        Some(after[..end].to_string())
    }
}

fn html_src_to_png(src: &str) -> Option<(Vec<u8>, u32, u32)> {
    if ascii_lower(src).starts_with("data:image/") {
        return embedded_image_to_png(src);
    }
    let path_str = src
        .strip_prefix("file:///")
        .or_else(|| src.strip_prefix("file://"))
        .unwrap_or(src);
    let path = std::path::PathBuf::from(path_str);
    if path.is_file() {
        image_file_to_png(&path)
    } else {
        None
    }
}

/// Extract a `data:image/...;base64,...` image embedded in HTML.
fn embedded_image_to_png(html: &str) -> Option<(Vec<u8>, u32, u32)> {
    let lower = ascii_lower(html);
    let marker = "data:image/";
    let pos = lower.find(marker)?;
    let rest = &html[pos + marker.len()..];
    let b64_marker = ";base64,";
    let b64_pos = ascii_lower(rest).find(b64_marker)?;
    let after = &rest[b64_pos + b64_marker.len()..];
    let end = after
        .find(|c: char| c == '"' || c == '\'' || c == ' ' || c == '>' || c == '\n')
        .unwrap_or(after.len());
    let b64 = after[..end].trim();
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    bytes_to_png(&bytes)
}

fn utf16_to_string(bytes: &[u8]) -> String {
    let mut units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    while units.last() == Some(&0) {
        units.pop();
    }
    String::from_utf16_lossy(&units)
}

fn ansi_to_string(bytes: &[u8]) -> String {
    let b = trim_nulls(bytes);
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => ansi_to_unicode(b),
    }
}

fn rtf_to_string(bytes: &[u8]) -> String {
    let b = trim_nulls(bytes);
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => ansi_to_unicode(b),
    }
}

fn trim_nulls(mut b: &[u8]) -> &[u8] {
    while b.last() == Some(&0) {
        b = &b[..b.len() - 1];
    }
    b
}

/// Decode a byte buffer that may be UTF-8, UTF-16 (with BOM), or system ANSI.
fn decode_fragment(bytes: &[u8]) -> String {
    let b = trim_nulls(bytes);
    if b.starts_with(&[0xFF, 0xFE]) {
        let mut units: Vec<u16> = b[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        while units.last() == Some(&0) {
            units.pop();
        }
        return String::from_utf16_lossy(&units);
    }
    if b.starts_with(&[0xFE, 0xFF]) {
        let mut units: Vec<u16> = b[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        while units.last() == Some(&0) {
            units.pop();
        }
        return String::from_utf16_lossy(&units);
    }
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => ansi_to_unicode(b),
    }
}

/// Decode bytes using the system ANSI code page (CP_ACP) via MultiByteToWideChar.
fn ansi_to_unicode(bytes: &[u8]) -> String {
    unsafe {
        let needed =
            MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0);
        if needed <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; needed as usize];
        MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, buf.as_mut_ptr(), needed);
        let mut s = String::from_utf16_lossy(&buf);
        while s.ends_with('\0') {
            s.pop();
        }
        s
    }
}

fn utf8_to_utf16_null(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity((s.len() + 1) * 2);
    for u in s.encode_utf16().chain(std::iter::once(0)) {
        v.extend_from_slice(&u.to_le_bytes());
    }
    v
}

fn parse_cf_html(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    let get = |name: &str| -> Option<usize> {
        let needle = format!("{}:", name);
        let pos = s.find(&needle)?;
        let after = &s[pos + needle.len()..];
        let end = after.find('\n').unwrap_or(after.len());
        after[..end].trim().parse::<usize>().ok()
    };
    let start = get("StartFragment").or_else(|| get("StartHTML"));
    let end = get("EndFragment").or_else(|| get("EndHTML"));
    match (start, end) {
        (Some(a), Some(b)) if b >= a && b <= bytes.len() => decode_fragment(&bytes[a..b]),
        _ => decode_fragment(bytes),
    }
}

fn build_cf_html(html: &str) -> Vec<u8> {
    let frag_start = "<!--StartFragment-->";
    let frag_end = "<!--EndFragment-->";
    let content = format!(
        "<html><body>{}{}{}</body></html>",
        frag_start, html, frag_end
    );
    let start_html = "Version:0.9\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\nStartFragment:0000000000\r\nEndFragment:0000000000\r\n"
        .len();
    let start_frag = start_html + "<html><body>".len() + frag_start.len();
    let end_frag = start_frag + html.len();
    let end_html = start_html + content.len();
    let header = format!(
        "Version:0.9\r\nStartHTML:{:010}\r\nEndHTML:{:010}\r\nStartFragment:{:010}\r\nEndFragment:{:010}\r\n",
        start_html, end_html, start_frag, end_frag
    );
    let mut out = header.into_bytes();
    out.extend_from_slice(content.as_bytes());
    out
}

/// Build a self-contained HTML for copy-back so text + image paste together.
fn build_write_html(
    text: Option<&str>,
    html: Option<&str>,
    png: Option<&[u8]>,
) -> Option<String> {
    let b64 = png.map(|p| {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(p)
    });
    match html {
        Some(h) => {
            if ascii_lower(h).contains("data:image/") {
                // already self-contained
                Some(h.to_string())
            } else if let Some(b) = &b64 {
                Some(embed_image_in_html(h, b))
            } else {
                Some(h.to_string())
            }
        }
        None => match (text, b64) {
            (Some(t), Some(b)) => Some(format!(
                r#"<p>{}</p><img src="data:image/png;base64,{}">"#,
                escape_html(t),
                b
            )),
            (None, Some(b)) => Some(format!(r#"<img src="data:image/png;base64,{}">"#, b)),
            _ => None,
        },
    }
}

/// Replace the first `<img ...>` tag with one embedding the given base64 PNG,
/// or append the image if the html has no img tag.
fn embed_image_in_html(html: &str, png_base64: &str) -> String {
    let new_tag = format!(r#"<img src="data:image/png;base64,{}">"#, png_base64);
    let lower = ascii_lower(html);
    if let Some(pos) = lower.find("<img") {
        let after = &html[pos + 4..];
        let after_lower = &lower[pos + 4..];
        let tag_end = after_lower.find('>').unwrap_or(after.len());
        format!("{}{}{}", &html[..pos], new_tag, &after[tag_end + 1..])
    } else {
        format!("{}{}", html, new_tag)
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        if c == '<' {
            in_tag = true;
            continue;
        }
        if c == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn make_preview(s: &str) -> String {
    let line: String = s.chars().take(240).collect();
    line.replace(['\r', '\n', '\t'], " ")
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    use image::ImageDecoder;
    let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    Some(decoder.dimensions())
}

fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut out);
        encoder
            .write_image(rgba, w, h, image::ExtendedColorType::Rgba8)
            .ok()?;
    }
    Some(out)
}

fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some((w, h, rgba.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cf_html_roundtrip() {
        let html = "<p>hello <b>world</b> 中文</p>";
        let cf = build_cf_html(html);
        assert_eq!(parse_cf_html(&cf), html);
    }

    #[test]
    fn utf16_roundtrip() {
        let s = "中文测试 abc 123";
        let bytes = utf8_to_utf16_null(s);
        assert_eq!(utf16_to_string(&bytes), s);
    }

    #[test]
    fn decode_fragment_variants() {
        // UTF-8 with trailing NULs
        assert_eq!(decode_fragment(b"hello<b>world</b>\0\0"), "hello<b>world</b>");
        // UTF-16LE with BOM
        let mut bytes = vec![0xFF, 0xFE];
        for u in "中文测试".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_fragment(&bytes), "中文测试");
        // System-ANSI bytes must decode without replacement chars
        let ansi = decode_fragment(&[0xD6, 0xD0, 0xCE, 0xC4]);
        assert!(!ansi.contains('\u{FFFD}'));
        // ASCII fast path
        assert_eq!(decode_fragment(b"plain <b>text</b>"), "plain <b>text</b>");
    }

    #[test]
    fn png_encode_decode_roundtrip() {
        let w = 8u32;
        let h = 6u32;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for i in (0..rgba.len()).step_by(4) {
            rgba[i] = 100;
            rgba[i + 1] = 150;
            rgba[i + 2] = 200;
            rgba[i + 3] = 255;
        }
        let png = encode_png(w, h, &rgba).unwrap();
        let (w2, h2, back) = decode_png(&png).unwrap();
        assert_eq!((w2, h2), (w, h));
        assert_eq!(back, rgba);
    }

    #[test]
    fn embedded_image_extraction() {
        use base64::Engine;
        let rgba = vec![
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let png = encode_png(2, 2, &rgba).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        let html = format!(
            r#"<html><body><img src="data:image/png;base64,{}"></body></html>"#,
            b64
        );
        let (bytes, w, h) = embedded_image_to_png(&html).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(bytes, png);
    }

    #[test]
    fn html_file_src_extraction() {
        let rgba = vec![
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let png = encode_png(2, 2, &rgba).unwrap();
        let path = std::env::temp_dir().join("clipcache_test_src.png");
        std::fs::write(&path, &png).unwrap();
        let html = format!(
            r#"<html><body><img src="file:///{}"></body></html>"#,
            path.display()
        );
        let result = extract_html_image(&html);
        std::fs::remove_file(&path).ok();
        let (bytes, w, h) = result.unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(bytes, png);
    }

    #[test]
    fn build_write_html_combines_text_and_image() {
        use base64::Engine;
        let rgba = vec![
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let png = encode_png(2, 2, &rgba).unwrap();
        // no stored html: text + image combined
        let h = build_write_html(Some("你好 <世界>"), None, Some(&png)).unwrap();
        assert!(h.contains("你好 &lt;世界&gt;"));
        assert!(h.contains("data:image/png;base64,"));
        let (_, w, _) = extract_html_image(&h).unwrap();
        assert_eq!(w, 2);
        // existing html without img: image appended
        let h2 = build_write_html(None, Some("<p>text</p>"), Some(&png)).unwrap();
        assert!(h2.contains("data:image/png;base64,"));
        // existing html with data image: left unchanged
        let embedded = format!(
            r#"<p>x</p><img src="data:image/png;base64,{}">"#,
            base64::engine::general_purpose::STANDARD.encode(&png)
        );
        let h3 = build_write_html(None, Some(&embedded), Some(&png)).unwrap();
        assert_eq!(h3, embedded);
    }

    #[test]
    fn write_back_combined_roundtrip() {
        let text = "组合文本";
        let png = encode_png(2, 2, &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255])
            .unwrap();
        let ok = write_to_clipboard(Some(text), None, None, Some(&png));
        assert!(ok, "write_to_clipboard failed");
        let cap = capture().expect("capture failed");
        assert_eq!(cap.kind, "image");
        assert_eq!(cap.text.as_deref(), Some(text));
        assert!(cap.png_bytes.is_some(), "image missing after write-back");
        let html = cap.html.expect("html missing after write-back");
        assert!(
            html.contains("data:image/png;base64,"),
            "html should embed the image"
        );
    }

    #[test]
    fn enumerate_formats() {
        let ok = write_to_clipboard(Some("format-test"), None, None, None);
        assert!(ok);
        let fmts = clipboard_formats();
        assert!(
            fmts.iter().any(|(_, name, _)| name == "CF_UNICODETEXT"),
            "CF_UNICODETEXT missing: {:?}",
            fmts
        );
    }
}
