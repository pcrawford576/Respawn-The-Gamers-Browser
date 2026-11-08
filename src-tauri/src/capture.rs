//! Screen capture for the markup editor.
//!
//! Grabs exactly what's on screen in the page area (games included) using the
//! Windows desktop compositor, and saves finished markups as PNG files.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Copies a screen rectangle (physical pixels) as RGBA.
#[cfg(windows)]
pub fn grab(x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u8>> {
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, HGDIOBJ, SRCCOPY,
    };
    if w <= 0 || h <= 0 {
        return None;
    }
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            return None;
        }
        let mem = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp as HGDIOBJ);
        let copied = BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY | CAPTUREBLT);

        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h; // top-down rows
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB as _;
        let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
        let lines = GetDIBits(mem, bmp, 0, h as u32, buf.as_mut_ptr().cast(), &mut bi, DIB_RGB_COLORS);

        SelectObject(mem, old);
        DeleteObject(bmp as HGDIOBJ);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);

        if copied == 0 || lines == 0 {
            return None;
        }
        // BGRA -> RGBA, opaque.
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        Some(buf)
    }
}

#[cfg(not(windows))]
pub fn grab(_x: i32, _y: i32, _w: i32, _h: i32) -> Option<Vec<u8>> {
    None
}

/// Pictures\Respawn
pub fn captures_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().picture_dir().ok().map(|d| d.join("Respawn"))
}

/// Keeps only safe filename characters and makes sure it ends in .png.
pub fn clean_name(name: &str) -> String {
    let mut n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' '))
        .take(80)
        .collect();
    n = n.trim().trim_matches('.').to_string();
    if n.is_empty() {
        n = "Respawn capture".into();
    }
    if !n.to_ascii_lowercase().ends_with(".png") {
        n.push_str(".png");
    }
    n
}
