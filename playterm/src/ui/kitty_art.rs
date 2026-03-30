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

// ── tmux passthrough helper ───────────────────────────────────────────────────

/// Build a Kitty APC sequence, optionally wrapped for tmux DCS passthrough.
///
/// Normal:  `\x1b_G{payload}\x1b\\`
/// tmux:    `\x1bPtmux;\x1b\x1b_G{payload}\x1b\x1b\\\x1b\\`
///
/// When running inside tmux every `\x1b` inside the passthrough payload must be
/// doubled so tmux forwards the inner sequence verbatim to the outer terminal.
fn apc(payload: &str, in_tmux: bool) -> String {
    if in_tmux {
        format!("\x1bPtmux;\x1b\x1b_G{}\x1b\x1b\\\x1b\\", payload)
    } else {
        format!("\x1b_G{}\x1b\\", payload)
    }
}

// ── Unicode placeholder helpers ───────────────────────────────────────────────

/// Combining-above diacritics used to encode the row index in each placeholder
/// cell.  Index N → diacritic for row N.  From the Kitty graphics protocol spec:
/// https://sw.kovidgoyal.net/kitty/graphics-protocol/#unicode-placeholders
const ROW_DIACRITICS: &[char] = &[
    '\u{0305}', '\u{030D}', '\u{030E}', '\u{0310}', '\u{0312}', '\u{033D}',
    '\u{033E}', '\u{033F}', '\u{0346}', '\u{034A}', '\u{034B}', '\u{034C}',
    '\u{0350}', '\u{0351}', '\u{0352}', '\u{0357}', '\u{035B}', '\u{0363}',
    '\u{0364}', '\u{0365}', '\u{0366}', '\u{0367}', '\u{0368}', '\u{0369}',
    '\u{036A}', '\u{036B}', '\u{036C}', '\u{036D}', '\u{036E}', '\u{036F}',
    '\u{0483}', '\u{0484}',
];

/// Build the placeholder string for one row of a Unicode-placeholder image.
///
/// Each cell is U+10EEEE (the Kitty placeholder codepoint) with the foreground
/// colour encoding the image ID as 24-bit RGB.  The first cell of each row also
/// carries the combining row-diacritic so the terminal knows which image row to
/// sample.  Subsequent cells in the same row omit the diacritic — the terminal
/// infers the column from the cell's horizontal position.
fn placeholder_row(cols: u16, image_id: u32, row_index: usize) -> String {
    let r = ((image_id >> 16) & 0xFF) as u8;
    let g = ((image_id >> 8)  & 0xFF) as u8;
    let b = ( image_id        & 0xFF) as u8;
    let diacritic = ROW_DIACRITICS
        .get(row_index)
        .copied()
        .unwrap_or('\u{0305}');

    // Set foreground colour; first cell gets the row diacritic, rest do not.
    let mut s = format!("\x1b[38;2;{r};{g};{b}m");
    s.push('\u{10EEEE}');
    s.push(diacritic);
    for _ in 1..cols {
        s.push('\u{10EEEE}');
    }
    s.push_str("\x1b[0m"); // reset colour
    s
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render image `bytes` (JPEG/PNG/etc.) into `area` using the Kitty graphics protocol.
///
/// `area` is the full widget rect (including borders); the image is placed in the
/// inner area (1-cell border inset on all sides).  Writes directly to stdout.
///
/// When `in_tmux` is `false`: uses direct APC placement (`a=T` with `c=`/`r=`
/// coordinates) — unchanged from the original implementation.
///
/// When `in_tmux` is `true`: uses the Unicode placeholder method so that tmux
/// treats the image cells as normal text.  Transmits with `a=t` (store only),
/// creates a virtual placement with `a=p,U=1`, then writes `U+10EEEE` placeholder
/// characters row-by-row using absolute cursor positioning.
pub fn render_image(bytes: &[u8], area: Rect, in_tmux: bool, tmux_status_offset: u16) -> Result<()> {
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

    // Transmit the image in ≤4096-char chunks.
    const CHUNK: usize = 4096;
    let chunks: Vec<&[u8]> = b64.as_bytes().chunks(CHUNK).collect();
    let n = chunks.len();

    let mut out = io::stdout().lock();

    if in_tmux {
        // ── Unicode placeholder path (tmux) ───────────────────────────────────
        // Delete any existing virtual placement for ID=1 before re-transmitting.
        let _ = write!(out, "{}", apc("a=d,d=i,i=1,q=2", true));

        // Step 1: Transmit image data only (a=t — store, no display).
        // No placement coordinates here; the virtual placement is explicit below.
        for (i, chunk) in chunks.iter().enumerate() {
            let is_last = i == n - 1;
            let m = if is_last { 0u8 } else { 1u8 };
            let chunk_str = unsafe { std::str::from_utf8_unchecked(chunk) };
            if i == 0 {
                write!(
                    out,
                    "{}",
                    apc(&format!("a=t,f=32,i=1,s={w},v={h},o=z,m={m},q=2;{chunk_str}"), true)
                )?;
            } else {
                write!(out, "{}", apc(&format!("m={m};{chunk_str}"), true))?;
            }
        }

        // Step 2: Create virtual placement (U=1 enables Unicode placeholder mode).
        write!(
            out,
            "{}",
            apc(&format!("a=p,U=1,i=1,c={inner_w},r={inner_h},q=2"), true)
        )?;

        // Step 3: Write placeholder characters row-by-row at the image position.
        // These are normal terminal text cells — tmux can overwrite them on window
        // switch, which is exactly what prevents the bleed.
        for row in 0..inner_h {
            write!(
                out,
                "\x1b[{};{}H{}",
                area.y + 1 + row + tmux_status_offset,
                area.x + 1,
                placeholder_row(inner_w, 1, row as usize)
            )?;
        }
    } else {
        // ── Direct placement path (non-tmux) — unchanged ──────────────────────
        // Move cursor to the inner-area top-left (terminal coords are 1-based).
        write!(out, "\x1b[{};{}H", inner_y + 1, inner_x + 1)?;

        for (i, chunk) in chunks.iter().enumerate() {
            let is_last = i == n - 1;
            let m = if is_last { 0u8 } else { 1u8 };
            let chunk_str = unsafe { std::str::from_utf8_unchecked(chunk) };
            if i == 0 {
                // First chunk: include all control parameters.
                // i=1 assigns a persistent image ID so the terminal stores the
                // image and we can redisplay it with a=p,i=1 without re-transmitting.
                write!(
                    out,
                    "{}",
                    apc(&format!("a=T,f=32,i=1,s={w},v={h},c={inner_w},r={inner_h},o=z,m={m},q=2;{chunk_str}"), false)
                )?;
            } else {
                write!(out, "{}", apc(&format!("m={m};{chunk_str}"), false))?;
            }
        }
    }

    out.flush()?;
    Ok(())
}

// ── Clearing ──────────────────────────────────────────────────────────────────

/// Delete the NowPlaying Kitty image (ID=1).
///
/// Non-tmux: `a=d,d=A` (delete all) — same as before.
/// tmux: `a=d,d=i,i=1` — delete virtual placement for ID=1 specifically.
pub fn clear_image(in_tmux: bool) -> Result<()> {
    let mut out = io::stdout().lock();
    if in_tmux {
        write!(out, "{}", apc("a=d,d=i,i=1,q=2", true))?;
    } else {
        write!(out, "{}", apc("a=d,d=A,q=2", false))?;
    }
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
    // Cells are ~2:1 tall-to-wide, so double the column count to produce square thumbnails.
    (strip_rows * 2, strip_rows)
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
    in_tmux: bool,
) {
    use base64::Engine;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    // Pre-clear previous art strip images/placements before drawing new ones.
    if in_tmux {
        // tmux path: delete virtual placements (d=i lowercase).
        let mut out = io::stdout().lock();
        for id in 100u32..=115 {
            let _ = write!(out, "{}", apc(&format!("a=d,d=i,i={id},q=2"), true));
        }
    }

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

            // Transmit image in chunks (same for both paths — a=t: store only).
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
                        "{}",
                        apc(&format!("a=t,f=32,i={kitty_id},s={w},v={h},o=z,m={m},q=2;{chunk_str}"), in_tmux)
                    );
                } else {
                    let _ = write!(out, "{}", apc(&format!("m={m};{chunk_str}"), in_tmux));
                }
            }

            if in_tmux {
                // ── Unicode placeholder path (tmux) ───────────────────────────
                // Virtual placement with U=1 — tmux sees placeholder chars as normal text.
                let _ = write!(
                    out,
                    "{}",
                    apc(&format!("a=p,U=1,i={kitty_id},c={thumb_cols},r={thumb_rows},q=2"), true)
                );
                // Write placeholder characters row-by-row at the thumbnail position.
                for pr in 0..thumb_rows {
                    let _ = write!(
                        out,
                        "\x1b[{};{}H{}",
                        row + 1 + pr,
                        col + 1,
                        placeholder_row(thumb_cols, kitty_id, pr as usize)
                    );
                }
            } else {
                // ── Direct placement path (non-tmux) — unchanged ──────────────
                let _ = write!(
                    out,
                    "\x1b[{};{}H{}",
                    row + 1,
                    col + 1,
                    apc(&format!("a=p,i={kitty_id},p=1,c={thumb_cols},r={thumb_rows},q=2;"), false)
                );
            }
            let _ = out.flush();
        }
        // If bytes are NOT in cache, leave the cells blank — ratatui has already
        // drawn the placeholder character(s) via the text fallback path in home_tab.rs.
    }
}

/// Delete all Kitty art-strip images/placements (IDs 100–115).
///
/// Non-tmux: `a=d,d=I` — deletes image data and all placements.
/// tmux: `a=d,d=i` — deletes virtual placements; image data freed separately.
/// Call on tab departure or terminal resize.
pub fn clear_art_strip(in_tmux: bool) -> Result<()> {
    let mut out = io::stdout().lock();
    for id in 100u32..=115 {
        if in_tmux {
            write!(out, "{}", apc(&format!("a=d,d=i,i={id},q=2"), true))?;
        } else {
            write!(out, "{}", apc(&format!("a=d,d=I,i={id},q=2"), false))?;
        }
    }
    out.flush()?;
    Ok(())
}
