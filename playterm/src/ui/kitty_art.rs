//! Kitty terminal graphics protocol helpers.
//!
//! Provides detection, rendering, and clearing of album art using the
//! [Kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/).
//!
//! Images are transmitted with `a=T` (transmit-and-display), `f=32` (RGBA8),
//! `o=z` (zlib-compressed), and positioned via a preceding cursor-move escape.

use std::io::{self, Write};

use anyhow::Result;
use ratatui::layout::Rect;

// ── Detection ──────────────────────────────────────────────────────────────────

/// Returns `true` if the running terminal supports the Kitty graphics protocol.
///
/// Sends a Kitty graphics query to `/dev/tty` and checks the response.
/// Also appends a DA1 device-attributes query (`\x1b[c`) which every VT100+
/// terminal answers unconditionally, guaranteeing the read thread terminates
/// even on terminals that ignore the Kitty probe.
///
/// Must be called before `enable_raw_mode()` / `EnterAlternateScreen` so that
/// the temporary raw-mode toggle does not interfere with the TUI startup.
pub fn detect_kitty_support() -> bool {
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    // Open /dev/tty for bidirectional I/O (works even when stdin/stdout are pipes).
    let mut tty = match OpenOptions::new().read(true).write(true).open("/dev/tty") {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut tty_read = match tty.try_clone() {
        Ok(f) => f,
        Err(_) => return false,
    };

    // Raw mode lets us read the response characters without waiting for Enter.
    if crossterm::terminal::enable_raw_mode().is_err() {
        return false;
    }

    // 1. Kitty graphics probe   – terminal replies \x1b_Gi=31;OK\x1b\\ if supported.
    // 2. DA1 device-attributes  – always answered; provides a guaranteed read terminator.
    let probe = b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c";
    let write_ok = tty.write_all(probe).is_ok() && tty.flush().is_ok();
    if !write_ok {
        let _ = crossterm::terminal::disable_raw_mode();
        return false;
    }

    // Read the response in a background thread so we can apply a hard timeout.
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut response = Vec::with_capacity(128);
        let mut byte = [0u8; 1];
        loop {
            match tty_read.read(&mut byte) {
                Ok(1) => {
                    response.push(byte[0]);
                    // DA1 response format: \x1b[?{digits}c  — stop on 'c' after \x1b[?
                    if byte[0] == b'c'
                        && response.windows(3).any(|w| w == b"\x1b[?")
                    {
                        break;
                    }
                    if response.len() >= 256 {
                        break;
                    }
                }
                _ => break,
            }
        }
        let _ = tx.send(String::from_utf8_lossy(&response).into_owned());
    });

    let result = rx
        .recv_timeout(Duration::from_millis(500))
        .map(|r| r.contains("_Gi=31;OK"))
        .unwrap_or(false);

    let _ = crossterm::terminal::disable_raw_mode();
    result
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render image `bytes` (JPEG/PNG/etc.) into `area` using the Kitty graphics protocol.
///
/// `area` is the full widget rect (including borders); the image is placed in the
/// inner area (1-cell border inset on all sides).  Writes directly to stdout.
pub fn render_image(bytes: &[u8], area: Rect) -> Result<()> {
    use base64::Engine;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    let inner_x = area.x + 1;
    let inner_y = area.y + 1;
    let inner_w = area.width.saturating_sub(2);
    let inner_h = area.height.saturating_sub(2);
    if inner_w == 0 || inner_h == 0 {
        return Ok(());
    }

    // Decode image from raw bytes.
    let img = image::load_from_memory(bytes)?;

    // Resize to fit the inner area.  We estimate 10 px per column, 20 px per row
    // (a reasonable approximation for most terminals).  Cap at 1024 to avoid
    // transferring enormous payloads on very large terminals.
    let px_w = (inner_w as u32 * 10).min(1024);
    let px_h = (inner_h as u32 * 20).min(1024);
    let img = img.resize(px_w, px_h, image::imageops::FilterType::Lanczos3);
    let img_rgba = img.to_rgba8();
    let (w, h) = img_rgba.dimensions();
    let raw = img_rgba.into_raw();

    // Zlib-compress the raw RGBA bytes.
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&raw)?;
    let compressed = enc.finish()?;

    // Base64-encode.
    let b64 = base64::engine::general_purpose::STANDARD.encode(&compressed);

    // Write to stdout.
    let mut out = io::stdout().lock();

    // Move cursor to the inner-area top-left (terminal coords are 1-based).
    write!(out, "\x1b[{};{}H", inner_y + 1, inner_x + 1)?;

    // Transmit the image in ≤4096-char chunks.
    const CHUNK: usize = 4096;
    let chunks: Vec<&[u8]> = b64.as_bytes().chunks(CHUNK).collect();
    let n = chunks.len();

    for (i, chunk) in chunks.iter().enumerate() {
        let is_last = i == n - 1;
        let m = if is_last { 0u8 } else { 1u8 };
        // SAFETY: b64 is ASCII, so each chunk is valid UTF-8.
        let chunk_str = unsafe { std::str::from_utf8_unchecked(chunk) };
        if i == 0 {
            // First chunk: include all control parameters.
            // i=1 assigns a persistent image ID so the terminal stores the image
            // and we can redisplay it later with a=p,i=1 without re-transmitting.
            write!(
                out,
                "\x1b_Ga=T,f=32,i=1,s={w},v={h},c={inner_w},r={inner_h},o=z,m={m},q=2;{chunk_str}\x1b\\"
            )?;
        } else {
            write!(out, "\x1b_Gm={m};{chunk_str}\x1b\\")?;
        }
    }

    out.flush()?;
    Ok(())
}

// ── Redisplay ─────────────────────────────────────────────────────────────────

/// Redisplay the image previously stored under ID 1 without re-transmitting
/// its pixel data.  The terminal must have received a prior `render_image`
/// call so that the image is in its store.
///
/// Use this after switching back to the NowPlaying tab when the album and
/// terminal area are unchanged — it is orders of magnitude faster than
/// re-encoding and re-sending the full image.
pub fn display_image(area: Rect) -> Result<()> {
    let inner_x = area.x + 1;
    let inner_y = area.y + 1;
    let inner_w = area.width.saturating_sub(2);
    let inner_h = area.height.saturating_sub(2);
    if inner_w == 0 || inner_h == 0 {
        return Ok(());
    }
    let mut out = io::stdout().lock();
    write!(out, "\x1b[{};{}H", inner_y + 1, inner_x + 1)?;
    write!(out, "\x1b_Ga=p,i=1,c={inner_w},r={inner_h},q=2;\x1b\\")?;
    out.flush()?;
    Ok(())
}

// ── Clearing ──────────────────────────────────────────────────────────────────

/// Delete all Kitty images currently displayed in the terminal.
pub fn clear_image() -> Result<()> {
    let mut out = io::stdout().lock();
    write!(out, "\x1b_Ga=d,d=A,q=2\x1b\\")?;
    out.flush()?;
    Ok(())
}

// ── Cell pixel size query ─────────────────────────────────────────────────────

/// Query the terminal for the cell pixel dimensions via `CSI 16 t`.
///
/// Uses the same `/dev/tty` + background-thread pattern as `detect_kitty_support`.
/// Must be called before `enable_raw_mode()` / `EnterAlternateScreen`.
///
/// Returns `Some((cell_width_px, cell_height_px))` on success, `None` on
/// timeout or parse failure.
pub fn query_cell_pixel_size() -> Option<(u16, u16)> {
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    let mut tty = OpenOptions::new().read(true).write(true).open("/dev/tty").ok()?;
    let mut tty_read = tty.try_clone().ok()?;

    if crossterm::terminal::enable_raw_mode().is_err() {
        return None;
    }

    // CSI 16 t — terminal responds with \x1b[6;{height};{width}t
    let write_ok = tty.write_all(b"\x1b[16t").is_ok() && tty.flush().is_ok();
    if !write_ok {
        let _ = crossterm::terminal::disable_raw_mode();
        return None;
    }

    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut response = Vec::with_capacity(64);
        let mut byte = [0u8; 1];
        loop {
            match tty_read.read(&mut byte) {
                Ok(1) => {
                    response.push(byte[0]);
                    // Response ends with 't'
                    if byte[0] == b't' {
                        break;
                    }
                    if response.len() >= 64 {
                        break;
                    }
                }
                _ => break,
            }
        }
        let _ = tx.send(String::from_utf8_lossy(&response).into_owned());
    });

    let result = rx
        .recv_timeout(Duration::from_millis(100))
        .ok()
        .and_then(|r| parse_cell_size_response(&r));

    let _ = crossterm::terminal::disable_raw_mode();
    result
}

/// Parse the terminal response to `CSI 16 t`.
/// Expected format: `\x1b[6;{height};{width}t`
fn parse_cell_size_response(response: &str) -> Option<(u16, u16)> {
    // Strip leading ESC[ if present
    let s = response
        .trim_start_matches('\x1b')
        .trim_start_matches('[');
    // Should be "6;{height};{width}t"
    let s = s.strip_prefix("6;")?;
    let t_pos = s.rfind('t')?;
    let nums = &s[..t_pos];
    let mut parts = nums.splitn(2, ';');
    let height: u16 = parts.next()?.parse().ok()?;
    let width: u16  = parts.next()?.parse().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

// ── Art strip sizing helpers ──────────────────────────────────────────────────

/// Compute thumbnail size for the home art strip.
///
/// Returns `(thumb_cols, thumb_rows)` in terminal cell units.
/// Both dimensions equal `strip_rows` (square in cell count) — the
/// `cell_px` parameter is ignored for sizing because CSI 16 t returns
/// unreliable values on Ghostty macOS.  A fixed 32 px/cell assumption
/// is used instead when computing pixel dimensions in `render_art_strip`.
pub fn art_strip_thumbnail_size(_cell_px: Option<(u16, u16)>, strip_rows: u16) -> (u16, u16) {
    // Square: same number of columns as rows.
    (strip_rows, strip_rows)
}

/// How many thumbnails fit horizontally in `terminal_cols` columns.
pub fn visible_thumbnail_count(terminal_cols: u16, thumb_cols: u16, gap_cols: u16) -> usize {
    if thumb_cols + gap_cols == 0 {
        return 1;
    }
    let count = ((terminal_cols.saturating_sub(2)) / (thumb_cols + gap_cols)) as usize;
    count.max(1)
}

// ── Art strip rendering ───────────────────────────────────────────────────────

/// Render the home tab art strip using Kitty protocol.
///
/// Kitty image IDs 100–115 are reserved for the art strip slots.
/// Writes escape sequences directly to stdout (same as `render_image`).
#[allow(clippy::too_many_arguments)]
pub fn render_art_strip(
    albums: &[crate::app::RecentAlbum],
    scroll_offset: usize,
    _selected_index: usize,
    art_cache: &std::collections::HashMap<String, Vec<u8>>,
    strip_area: ratatui::layout::Rect,
    cell_px: Option<(u16, u16)>,
    terminal_col_offset: u16,
    terminal_row_offset: u16,
) {
    use base64::Engine;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    let (thumb_cols, thumb_rows) = art_strip_thumbnail_size(cell_px, strip_area.height);
    let visible_count = visible_thumbnail_count(strip_area.width, thumb_cols, 1);
    // Use strip height in cells as both width and height (square grid).
    // Fixed 32 px/cell — reliable on HiDPI Ghostty; avoids trusting CSI 16 t.
    let thumb_px = thumb_rows as u32 * 32;
    let px_w = thumb_px;
    let px_h = thumb_px;

    for i in 0..visible_count {
        let album_index = scroll_offset + i;
        if album_index >= albums.len() {
            break;
        }
        let album_id = &albums[album_index].album_id;
        let kitty_id: u32 = 100 + i as u32;

        if let Some(bytes) = art_cache.get(album_id) {
            // Decode and resize.
            let img = match image::load_from_memory(bytes) {
                Ok(i) => i,
                Err(_) => continue,
            };
            let img = img.resize_exact(px_w, px_h, image::imageops::FilterType::Lanczos3);
            let img_rgba = img.to_rgba8();
            let (w, h) = img_rgba.dimensions();
            let raw = img_rgba.into_raw();

            // Zlib-compress.
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
            if enc.write_all(&raw).is_err() { continue; }
            let compressed = match enc.finish() { Ok(c) => c, Err(_) => continue };

            // Base64-encode.
            let b64 = base64::engine::general_purpose::STANDARD.encode(&compressed);

            let col = terminal_col_offset + i as u16 * (thumb_cols + 1);
            let row = terminal_row_offset;

            let mut out = io::stdout().lock();

            // Move cursor to placement position (1-based).
            let _ = write!(out, "\x1b[{};{}H", row + 1, col + 1);

            // Transmit image in chunks.
            const CHUNK: usize = 4096;
            let chunks: Vec<&[u8]> = b64.as_bytes().chunks(CHUNK).collect();
            let n = chunks.len();
            for (ci, chunk) in chunks.iter().enumerate() {
                let is_last = ci == n - 1;
                let m = if is_last { 0u8 } else { 1u8 };
                let chunk_str = unsafe { std::str::from_utf8_unchecked(chunk) };
                if ci == 0 {
                    let _ = write!(
                        out,
                        "\x1b_Ga=t,f=32,i={kitty_id},s={w},v={h},o=z,m={m},q=2;{chunk_str}\x1b\\"
                    );
                } else {
                    let _ = write!(out, "\x1b_Gm={m};{chunk_str}\x1b\\");
                }
            }

            // Place the transmitted image.
            let _ = write!(
                out,
                "\x1b[{};{}H\x1b_Ga=p,i={kitty_id},p=1,c={thumb_cols},r={thumb_rows},q=2;\x1b\\",
                row + 1,
                col + 1,
            );
            let _ = out.flush();
        }
        // If bytes are NOT in cache, leave the cells blank — ratatui has already
        // drawn the placeholder character(s) via the text fallback path in home_tab.rs.
    }
}

/// Delete all Kitty art-strip placements (IDs 100–115).
///
/// Call on tab departure or terminal resize to remove strip images from screen.
pub fn clear_art_strip() -> Result<()> {
    let mut out = io::stdout().lock();
    for id in 100u32..=115 {
        write!(out, "\x1b_Ga=d,d=I,i={id},q=2\x1b\\")?;
    }
    out.flush()?;
    Ok(())
}
