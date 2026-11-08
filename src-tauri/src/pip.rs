//! Picture-in-picture: an always-on-top mini window for streams and guides.
//! Ctrl+Shift+O makes it click-through so it can sit over a game.

use crate::{adblock, is_newtab, launch_args, Browser};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindowBuilder};

static CLICK_THROUGH: AtomicBool = AtomicBool::new(false);

const PAUSE_MEDIA: &str = "document.querySelectorAll('video,audio').forEach(m=>{try{m.pause()}catch(e){}})";

pub fn toggle(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("pip") {
        let _ = w.close();
        CLICK_THROUGH.store(false, Ordering::Relaxed);
        return;
    }
    let (id, url, account) = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        match b.active.and_then(|a| b.tabs.iter().find(|t| t.id == a)) {
            Some(t) => (t.id, t.url.clone(), t.account),
            None => return,
        }
    };
    if is_newtab(&url) {
        let _ = app.emit("toast", "Open a stream or video first, then pop it out");
        return;
    }
    let Ok(u) = Url::parse(&url) else { return };

    // Pause the source tab so audio doesn't double up.
    if let Some(wv) = app.get_webview(&format!("tab-{id}")) {
        let _ = wv.eval(PAUSE_MEDIA);
    }

    let mut b = WebviewWindowBuilder::new(app, "pip", WebviewUrl::External(u))
        .title("Respawn PiP")
        .inner_size(560.0, 330.0)
        .min_inner_size(240.0, 140.0)
        .always_on_top(true)
        .additional_browser_args(launch_args());
    if adblock::enabled() {
        b = b.initialization_script(adblock::COSMETIC_JS);
    }
    // Same account (logins) as the tab it came from.
    if let Some(dir) = crate::account_dir(app, account) {
        b = b.data_directory(dir);
    }
    match b.build() {
        Ok(_) => {
            if let Some(wv) = app.get_webview("pip") {
                crate::wv2::configure(&wv);
            }
            let _ = app.emit("toast", "PiP open — Ctrl+Shift+O toggles click-through, Ctrl+Shift+P closes");
        }
        Err(e) => {
            let _ = app.emit("toast", format!("Couldn't open PiP: {e}"));
        }
    }
}

pub fn toggle_click_through(app: &AppHandle) {
    let Some(w) = app.get_webview_window("pip") else { return };
    let on = !CLICK_THROUGH.load(Ordering::Relaxed);
    if w.set_ignore_cursor_events(on).is_ok() {
        CLICK_THROUGH.store(on, Ordering::Relaxed);
        let _ = app.emit("toast", if on { "PiP click-through ON" } else { "PiP click-through OFF" });
    }
}
