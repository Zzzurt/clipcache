//! DIB (device-independent bitmap) <-> RGBA conversion for clipboard images.

fn read_u32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn read_i32(b: &[u8], i: usize) -> i32 {
    i32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn read_u16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn write_u32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_le_bytes());
}
fn write_i32(b: &mut [u8], i: usize, v: i32) {
    b[i..i + 4].copy_from_slice(&v.to_le_bytes());
}
fn write_u16(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}

/// Expand a `bits`-width channel value to a full 8-bit value.
fn expand(v: u8, bits: u32) -> u8 {
    match bits {
        0 => 0,
        1 => if v != 0 { 255 } else { 0 },
        2 => (v << 6) | (v << 4) | (v << 2) | v,
        3 => (v << 5) | (v << 2) | (v >> 1),
        4 => (v << 4) | v,
        5 => (v << 3) | (v >> 2),
        6 => (v << 2) | (v >> 4),
        7 => (v << 1) | (v >> 6),
        _ => v,
    }
}

fn extract(px: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let mut m = mask;
    let mut shift = 0u32;
    while m & 1 == 0 {
        m >>= 1;
        shift += 1;
    }
    let bits = m.count_ones();
    let v = ((px & mask) >> shift) as u8;
    expand(v, bits)
}

/// Convert a CF_DIB / CF_DIBV5 byte buffer to RGBA8 pixels.
/// Returns `(width, height, rgba_pixels)`.
pub fn dib_to_rgba(dib: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if dib.len() < 40 {
        return None;
    }
    let header_size = read_u32(dib, 0) as usize;
    if header_size < 40 || dib.len() < header_size {
        return None;
    }
    let width = read_i32(dib, 4);
    let height = read_i32(dib, 8);
    let bit_count = read_u16(dib, 14);
    let compression = read_u32(dib, 16);
    if width <= 0 || height == 0 {
        return None;
    }
    let abs_height = height.unsigned_abs();
    let w = width as usize;
    let h = abs_height as usize;

    let mut offset = header_size;
    let mut masks: [u32; 4] = [0; 4];
    let mut palette: Vec<[u8; 3]> = Vec::new();

    if compression == 3 || compression == 6 {
        // BI_BITFIELDS / BI_ALPHABITFIELDS: masks follow the header
        let mask_count = if compression == 6 { 4 } else { 3 };
        if dib.len() < offset + mask_count * 4 {
            return None;
        }
        for i in 0..mask_count {
            masks[i] = read_u32(dib, offset + i * 4);
        }
        offset += mask_count * 4;
    } else if bit_count <= 8 {
        let colors_used = read_u32(dib, 32);
        let count = if colors_used != 0 {
            colors_used as usize
        } else {
            1usize << bit_count
        };
        if dib.len() < offset + count * 4 {
            return None;
        }
        for i in 0..count {
            let b = dib[offset + i * 4];
            let g = dib[offset + i * 4 + 1];
            let r = dib[offset + i * 4 + 2];
            palette.push([r, g, b]);
        }
        offset += count * 4;
    }

    let row_stride = ((w * bit_count as usize + 31) / 32) * 4;
    let needed = row_stride * h;
    if dib.len() < offset + needed {
        return None;
    }
    let bottom_up = height > 0;
    let src = &dib[offset..offset + needed];
    let mut rgba = vec![0u8; w * h * 4];

    for row in 0..h {
        let src_row = if bottom_up { h - 1 - row } else { row };
        let srow = src_row * row_stride;
        for col in 0..w {
            let px: u32 = match bit_count {
                32 => read_u32(src, srow + col * 4),
                24 => {
                    let i = srow + col * 3;
                    ((src[i + 2] as u32) << 16) | ((src[i + 1] as u32) << 8) | (src[i] as u32)
                }
                16 => read_u16(src, srow + col * 2) as u32,
                8 => src[srow + col] as u32,
                4 => {
                    let byte = src[srow + col / 2];
                    if col % 2 == 0 {
                        (byte >> 4) as u32
                    } else {
                        (byte & 0x0F) as u32
                    }
                }
                1 => {
                    let byte = src[srow + col / 8];
                    ((byte >> (7 - (col % 8))) & 1) as u32
                }
                _ => return None,
            };

            let (r, g, b) = if bit_count <= 8 {
                let idx = px as usize;
                if idx < palette.len() {
                    (palette[idx][0], palette[idx][1], palette[idx][2])
                } else {
                    (0, 0, 0)
                }
            } else {
                match bit_count {
                    32 => {
                        if compression == 3 || compression == 6 {
                            (extract(px, masks[0]), extract(px, masks[1]), extract(px, masks[2]))
                        } else {
                            // BI_RGB 32bpp is BGRA
                            let b = (px & 0xFF) as u8;
                            let g = ((px >> 8) & 0xFF) as u8;
                            let r = ((px >> 16) & 0xFF) as u8;
                            (r, g, b)
                        }
                    }
                    24 => {
                        let r = ((px >> 16) & 0xFF) as u8;
                        let g = ((px >> 8) & 0xFF) as u8;
                        let b = (px & 0xFF) as u8;
                        (r, g, b)
                    }
                    16 => {
                        if compression == 3 {
                            (extract(px, masks[0]), extract(px, masks[1]), extract(px, masks[2]))
                        } else {
                            // default 5-5-5 (XRGB1555)
                            let r = ((px >> 10) & 0x1F) as u8;
                            let g = ((px >> 5) & 0x1F) as u8;
                            let b = (px & 0x1F) as u8;
                            (expand(r, 5), expand(g, 5), expand(b, 5))
                        }
                    }
                    _ => (0, 0, 0),
                }
            };

            let d = (row * w + col) * 4;
            rgba[d] = r;
            rgba[d + 1] = g;
            rgba[d + 2] = b;
            rgba[d + 3] = 255;
        }
    }

    Some((w as u32, h as u32, rgba))
}

/// Encode RGBA8 pixels into a 32bpp BI_RGB bottom-up DIB (CF_DIB).
pub fn rgba_to_dib(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let stride = w * 4;
    let image_size = stride * h;
    let total = 40 + image_size;
    let mut out = vec![0u8; total];

    write_u32(&mut out, 0, 40); // biSize
    write_i32(&mut out, 4, width as i32); // biWidth
    write_i32(&mut out, 8, height as i32); // biHeight (positive = bottom-up)
    write_u16(&mut out, 12, 1); // biPlanes
    write_u16(&mut out, 14, 32); // biBitCount
    write_u32(&mut out, 16, 0); // biCompression BI_RGB
    write_u32(&mut out, 20, image_size as u32); // biSizeImage

    for row in 0..h {
        let src_row = h - 1 - row; // bottom-up
        for col in 0..w {
            let s = (src_row * w + col) * 4;
            let d = 40 + (row * w + col) * 4;
            out[d] = rgba[s + 2]; // B
            out[d + 1] = rgba[s + 1]; // G
            out[d + 2] = rgba[s]; // R
            out[d + 3] = rgba[s + 3]; // A
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dib_roundtrip_32bpp() {
        let w = 16u32;
        let h = 8u32;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                rgba[i] = (x * 16) as u8;
                rgba[i + 1] = (y * 32) as u8;
                rgba[i + 2] = ((x + y) * 10) as u8;
                rgba[i + 3] = 255;
            }
        }
        let dib = rgba_to_dib(w, h, &rgba);
        let (w2, h2, back) = dib_to_rgba(&dib).unwrap();
        assert_eq!((w2, h2), (w, h));
        assert_eq!(back, rgba);
    }

    #[test]
    fn dib_topdown_24bpp() {
        // top-down (negative height), 3 bytes/pixel BGR
        let w = 4u32;
        let h = 3u32;
        let stride = w as usize * 3;
        let mut dib = vec![0u8; 40 + stride * h as usize];
        write_u32(&mut dib, 0, 40);
        write_i32(&mut dib, 4, w as i32);
        write_i32(&mut dib, 8, -(h as i32));
        write_u16(&mut dib, 12, 1);
        write_u16(&mut dib, 14, 24);
        for y in 0..h as usize {
            for x in 0..w as usize {
                let p = 40 + y * stride + x * 3;
                dib[p] = 10; // B
                dib[p + 1] = 20; // G
                dib[p + 2] = 30; // R
            }
        }
        let (w2, h2, rgba) = dib_to_rgba(&dib).unwrap();
        assert_eq!((w2, h2), (w, h));
        assert_eq!(&rgba[0..4], &[30, 20, 10, 255]);
    }
}
