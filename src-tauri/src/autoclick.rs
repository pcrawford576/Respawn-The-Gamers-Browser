//! Auto clicker: repeatedly left-clicks wherever the mouse pointer is.
//!
//! Started and stopped with a system-wide hotkey (F11 by default, changeable in
//! Settings → Keyboard shortcuts), so it works in web games and desktop games.

use crate::{emit_state, Browser};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Fastest allowed speed, so a typo can't flood the system with clicks.
pub const MIN_INTERVAL_MS: u64 = 10;

static RUNNING: AtomicBool = AtomicBool::new(false);
/// Bumped on every start so an old click thread that is still sleeping
/// exits instead of running alongside the new one.
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

pub fn toggle(app: &AppHandle) {
    set(app, !running());
}

pub fn set(app: &AppHandle, on: bool) {
    if on == running() {
        return;
    }
    if on {
        let gen = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        RUNNING.store(true, Ordering::SeqCst);
        let app2 = app.clone();
        std::thread::spawn(move || click_loop(app2, gen));
    } else {
        RUNNING.store(false, Ordering::SeqCst);
    }
    let key = app
        .state::<Mutex<Browser>>()
        .lock()
        .unwrap()
        .settings
        .keybinds
        .get("autoclick")
        .cloned()
        .unwrap_or_default();
    let msg = match (on, key.is_empty()) {
        (true, false) => format!("Auto clicker ON — press {key} to stop"),
        (true, true) => "Auto clicker ON".to_string(),
        (false, _) => "Auto clicker OFF".to_string(),
    };
    let _ = app.emit("toast", msg);
    emit_state(app);
}

fn interval(app: &AppHandle) -> u64 {
    let ms = app.state::<Mutex<Browser>>().lock().unwrap().settings.autoclick_ms;
    ms.max(MIN_INTERVAL_MS)
}

fn click_loop(app: AppHandle, gen: u64) {
    let alive = || RUNNING.load(Ordering::SeqCst) && GENERATION.load(Ordering::SeqCst) == gen;
    while alive() {
        let every = interval(&app);
        // Hold the button briefly: some games ignore a down+up in the same instant.
        let hold = (every / 2).min(15);
        mouse(true);
        std::thread::sleep(Duration::from_millis(hold));
        mouse(false);
        // Sleep in small steps so stopping feels instant even at slow speeds.
        let mut left = every.saturating_sub(hold);
        while left > 0 && alive() {
            let step = left.min(25);
            std::thread::sleep(Duration::from_millis(step));
            left -= step;
        }
    }
}

#[cfg(windows)]
fn mouse(down: bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    };
    unsafe {
        let mut input: INPUT = std::mem::zeroed();
        input.r#type = INPUT_MOUSE;
        input.Anonymous.mi.dwFlags = if down { MOUSEEVENTF_LEFTDOWN } else { MOUSEEVENTF_LEFTUP };
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(not(windows))]
fn mouse(_down: bool) {}
