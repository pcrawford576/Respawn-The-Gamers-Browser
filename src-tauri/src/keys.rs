//! Keyboard shortcuts (changeable in Settings) and the full-screen game view.
//!
//! Shortcuts work everywhere in Respawn: in the browser bar (app.js), and inside
//! web pages and games via a small injected script that signals Rust through the
//! respawn.invalid bridge. Game Mode is a system-wide hotkey so it also works
//! while a desktop game has focus.

use crate::{add_tab, cycle_tab, do_close_tab, emit_state, perf, relayout, Browser};
use std::collections::BTreeMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

/// (action, default keys). Labels live in the Settings screen.
pub const ACTIONS: &[(&str, &str)] = &[
    ("prev_tab", "Q"),
    ("next_tab", "W"),
    ("fullscreen", "F12"),
    ("game_mode", "Ctrl+Shift+G"),
    ("capture", "Ctrl+Shift+X"),
    ("new_tab", "Ctrl+T"),
    ("close_tab", "Ctrl+W"),
    ("reload", "Ctrl+R"),
    ("address", "Ctrl+L"),
];

/// Handled system-wide (works in any program), so pages and the bar skip it.
pub const GLOBAL_ACTION: &str = "game_mode";

pub fn defaults() -> BTreeMap<String, String> {
    ACTIONS.iter().map(|(a, k)| (a.to_string(), k.to_string())).collect()
}

/// Fills in any action missing from saved settings (e.g. added in an update).
pub fn complete(map: &mut BTreeMap<String, String>) {
    for (a, k) in ACTIONS {
        map.entry(a.to_string()).or_insert_with(|| k.to_string());
    }
    map.retain(|a, _| ACTIONS.iter().any(|(x, _)| x == a));
}

pub fn is_action(a: &str) -> bool {
    ACTIONS.iter().any(|(x, _)| *x == a)
}

/// Script for web pages: keys → action, ignoring single keys while typing.
const KEYS_JS: &str = r#"
(() => {
  const open = window.open.bind(window);
  let map = __MAP__;
  window.__respawnSetKeys = (m) => { map = m; };
  const name = (e) => {
    const c = e.code || '';
    if (/^Key[A-Z]$/.test(c)) return c.slice(3);
    if (/^Digit\d$/.test(c)) return c.slice(5);
    if (/^F\d{1,2}$/.test(c)) return c;
    const named = { Tab: 'Tab', Space: 'Space', Enter: 'Enter', Backquote: '`', Minus: '-', Equal: '=',
      BracketLeft: '[', BracketRight: ']', Semicolon: ';', Quote: "'", Comma: ',', Period: '.', Slash: '/',
      Backslash: '\\', PageUp: 'PageUp', PageDown: 'PageDown', Home: 'Home', End: 'End', Insert: 'Insert',
      Delete: 'Delete', ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down' };
    if (named[c]) return named[c];
    if (/^Numpad\d$/.test(c)) return 'Num' + c.slice(6);
    return null;
  };
  addEventListener('keydown', (e) => {
    if (e.repeat) return;
    const k = name(e);
    if (!k) return;
    const combo = (e.ctrlKey ? 'Ctrl+' : '') + (e.altKey ? 'Alt+' : '') + (e.shiftKey ? 'Shift+' : '') + k;
    const action = map[combo];
    if (!action) return;
    // Plain keys (like Q) still type normally in text boxes and chat.
    const plain = !e.ctrlKey && !e.altKey && !/^F\d/.test(k);
    const t = e.composedPath ? e.composedPath()[0] : e.target;
    if (plain && t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName || ''))) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    open('https://respawn.invalid/key?a=' + action, '_blank');
  }, true);
})();
"#;

/// combo → action, without the system-wide one.
fn page_map(binds: &BTreeMap<String, String>) -> String {
    let m: BTreeMap<&str, &str> = binds
        .iter()
        .filter(|(a, k)| a.as_str() != GLOBAL_ACTION && !k.is_empty())
        .map(|(a, k)| (k.as_str(), a.as_str()))
        .collect();
    serde_json::to_string(&m).unwrap_or_else(|_| "{}".into())
}

pub fn page_script(binds: &BTreeMap<String, String>) -> String {
    KEYS_JS.replace("__MAP__", &page_map(binds))
}

/// After the shortcuts change: update every open page and the global hotkey.
pub fn apply(app: &AppHandle) {
    let binds = app.state::<Mutex<Browser>>().lock().unwrap().settings.keybinds.clone();
    let js = format!("window.__respawnSetKeys && window.__respawnSetKeys({})", page_map(&binds));
    for (label, wv) in app.webviews() {
        if label.starts_with("tab-") || label.starts_with("side-") {
            let _ = wv.eval(&js);
        }
    }
    register_global(app, binds.get(GLOBAL_ACTION).map(String::as_str).unwrap_or(""));
}

static GLOBAL: Mutex<Option<Shortcut>> = Mutex::new(None);

/// (Re)registers the system-wide Game Mode hotkey.
pub fn register_global(app: &AppHandle, combo: &str) {
    let gs = app.global_shortcut();
    let mut cur = GLOBAL.lock().unwrap();
    if let Some(old) = cur.take() {
        let _ = gs.unregister(old);
    }
    if combo.is_empty() {
        return;
    }
    match combo.parse::<Shortcut>() {
        Ok(sc) => match gs.register(sc.clone()) {
            Ok(()) => *cur = Some(sc),
            Err(e) => {
                let _ = app.emit("toast", format!("{combo} is already used by another program: {e}"));
            }
        },
        Err(_) => {
            let _ = app.emit("toast", format!("{combo} can't be used as a system-wide shortcut"));
        }
    }
}

pub fn is_global(sc: &Shortcut) -> bool {
    GLOBAL.lock().unwrap().as_ref() == Some(sc)
}

/// Runs a shortcut's action.
pub fn run_action(app: &AppHandle, action: &str) {
    match action {
        "prev_tab" => cycle_tab(app, -1),
        "next_tab" => cycle_tab(app, 1),
        "fullscreen" => toggle_fullscreen(app),
        "game_mode" => {
            let on = !app.state::<Mutex<Browser>>().lock().unwrap().game_mode;
            perf::set_game_mode(app, on);
        }
        "capture" => {
            let _ = app.emit("capture-request", ());
        }
        "new_tab" => {
            let slot = crate::new_tab_slot(app);
            add_tab(app, "newtab", None, false, true, true, slot);
        }
        "close_tab" => {
            let active = app.state::<Mutex<Browser>>().lock().unwrap().active;
            if let Some(id) = active {
                do_close_tab(app, id);
            }
        }
        "reload" => {
            let active = app.state::<Mutex<Browser>>().lock().unwrap().active;
            if let Some(wv) = active.and_then(|id| app.get_webview(&format!("tab-{id}"))) {
                let _ = wv.eval("location.reload()");
            }
        }
        "address" => {
            if app.state::<Mutex<Browser>>().lock().unwrap().fullscreen {
                toggle_fullscreen(app);
            }
            if let Some(ui) = app.get_webview("ui") {
                let _ = ui.set_focus();
            }
            let _ = app.emit("focus-address", ());
        }
        _ => {}
    }
}

/// The floating account badge shown on top of the page in full screen.
/// Clicking it opens the saved-accounts menu.
pub const FS_BADGE_JS: &str = r#"
(() => {
  if (window.top !== window) return;
  const open = window.open.bind(window);
  let host = null, nameEl = null, posEl = null, fade = 0;
  const wake = () => {
    if (!host) return;
    host.style.opacity = '1';
    clearTimeout(fade);
    fade = setTimeout(() => { if (host) host.style.opacity = '.35'; }, 3000);
  };
  const build = () => {
    host = document.createElement('respawn-badge');
    host.style.cssText = 'position:fixed;top:10px;left:50%;transform:translateX(-50%);z-index:2147483647;' +
      'transition:opacity .4s;opacity:1;pointer-events:auto;';
    const root = host.attachShadow({ mode: 'closed' });
    root.innerHTML = `<style>
      button { display:flex; align-items:center; gap:8px; height:30px; padding:0 12px 0 10px; border-radius:99px;
        background:rgba(10,11,16,.82); color:#e7e9f3; border:1px solid rgba(61,245,167,.55); cursor:pointer;
        font:600 13px "Segoe UI", system-ui, sans-serif; box-shadow:0 4px 14px rgba(0,0,0,.45); backdrop-filter:blur(6px); }
      button:hover { border-color:#3df5a7; background:rgba(10,11,16,.95); }
      .dot { width:8px; height:8px; border-radius:50%; background:#3df5a7; box-shadow:0 0 8px #3df5a7; }
      .pos { color:#8b90a8; font-weight:500; font-size:12px; }
      .caret { color:#3df5a7; font-size:11px; }
    </style><button title="Switch account (Respawn)"><span class="dot"></span><span class="name"></span><span class="pos"></span><span class="caret">▼</span></button>`;
    nameEl = root.querySelector('.name');
    posEl = root.querySelector('.pos');
    root.querySelector('button').addEventListener('click', (e) => {
      e.stopPropagation();
      open('https://respawn.invalid/acctmenu', '_blank');
    });
    host.addEventListener('mouseenter', wake);
  };
  // Esc leaves full screen (the badge is only shown in full screen).
  // Not while typing in a text box, where Esc usually cancels the typing.
  addEventListener('keydown', (e) => {
    if (e.key !== 'Escape' || e.repeat || !host || !host.isConnected) return;
    const t = e.composedPath ? e.composedPath()[0] : e.target;
    if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName || ''))) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    open('https://respawn.invalid/key?a=fullscreen', '_blank');
  }, true);
  window.__respawnFs = (s) => {
    if (!s || !s.show) { if (host) { host.remove(); host = null; } return; }
    if (!host) build();
    if (!host.isConnected) (document.body || document.documentElement).appendChild(host);
    nameEl.textContent = s.name;
    posEl.textContent = s.pos || '';
    wake();
  };
})();
"#;

/// Shows the account badge on the active tab in full screen (hides it elsewhere).
pub fn push_fs_badge(app: &AppHandle) {
    let (full, active, info) = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        let info = b.active.and_then(|id| {
            let i = b.tabs.iter().position(|t| t.id == id)?;
            let t = &b.tabs[i];
            let saved = b.settings.saved_accounts.iter().find(|a| a.slot == t.account).map(|a| a.name.clone());
            let name = t.name.clone().or(saved).unwrap_or_else(|| {
                let title = t.title.split([':', '|']).next().unwrap_or("").trim().to_string();
                if title.is_empty() { "Tab".into() } else { title }
            });
            Some((name, if b.tabs.len() > 1 { format!("{}/{}", i + 1, b.tabs.len()) } else { String::new() }))
        });
        (b.fullscreen, b.active, info)
    };
    for (label, wv) in app.webviews() {
        let Some(id) = label.strip_prefix("tab-").and_then(|s| s.parse::<u32>().ok()) else { continue };
        let js = match (&info, full && Some(id) == active) {
            (Some((name, pos)), true) => format!(
                "window.__respawnFs && window.__respawnFs({})",
                serde_json::json!({ "show": true, "name": name, "pos": pos })
            ),
            _ => "window.__respawnFs && window.__respawnFs({show:false})".to_string(),
        };
        let _ = wv.eval(&js);
    }
}

/// Full-screen game view: the page fills the whole screen, all browser bars hide.
pub fn toggle_fullscreen(app: &AppHandle) {
    let on = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        b.fullscreen = !b.fullscreen;
        b.fullscreen
    };
    if let Some(win) = app.get_window("main") {
        let _ = win.set_fullscreen(on);
    }
    relayout(app);
    emit_state(app);
    push_fs_badge(app);
    // Keep the keyboard in the page so the tab keys and F12 keep working.
    let active = app.state::<Mutex<Browser>>().lock().unwrap().active;
    if let Some(wv) = active.and_then(|id| app.get_webview(&format!("tab-{id}"))) {
        let _ = wv.set_focus();
    }
}
