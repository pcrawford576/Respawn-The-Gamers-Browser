// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adblock;
mod capture;
mod discord;
mod history;
mod keys;
mod perf;
mod session;
mod settings;
mod wv2;

use serde::Serialize;
use settings::Settings;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, Url, Webview, WebviewBuilder,
    WebviewUrl, WindowEvent,
};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri_plugin_global_shortcut::ShortcutState;

// Layout of the browser chrome, in logical pixels. Must match ui/style.css.
const TOP: f64 = 84.0; // tab strip (38) + toolbar (46)
const RAIL: f64 = 52.0; // left icon rail
const PANEL: f64 = 400.0; // sidebar panel width
const STATUS: f64 = 26.0; // bottom status bar

/// Sidebar panels: (id, url). Mobile/compact pages fit the narrow panel best.
const PANELS: &[(&str, &str)] = &[
    ("discord", "https://discord.com/app"),
    ("twitch", "https://www.twitch.tv/directory/following"),
    ("youtube", "https://m.youtube.com/"),
    ("steam", "https://store.steampowered.com/"),
];

/// Fake host used by injected page scripts to talk to Rust without giving
/// websites any IPC access: navigations to it are intercepted and cancelled.
const BRIDGE_HOST: &str = "respawn.invalid";

pub struct Tab {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub discarded: bool,
    pub loading: bool,
    pub last_active: Instant,
    /// "Keep loaded": never unloaded by the RAM cap, idle timer or Game Mode.
    pub pinned: bool,
    /// Login storage slot: 0 = shared main storage, n = this tab's own storage.
    pub account: u32,
    /// Name you gave this tab (e.g. the in-game account name).
    pub name: Option<String>,
}

pub struct Browser {
    pub tabs: Vec<Tab>,
    pub active: Option<u32>,
    pub next_id: u32,
    pub sidebar: Option<String>,
    pub game_mode: bool,
    /// Full-screen game view (F12): only the page, filling the screen.
    pub fullscreen: bool,
    pub overlay: bool,
    pub settings: Settings,
}

type State<'a> = tauri::State<'a, Mutex<Browser>>;

static LAUNCH_ARGS: OnceLock<String> = OnceLock::new();

pub fn launch_args() -> &'static str {
    LAUNCH_ARGS.get().map(|s| s.as_str()).unwrap_or("")
}

pub fn is_newtab(url: &str) -> bool {
    url == "newtab" || url.contains("tauri.localhost/newtab") || url.starts_with("tauri://localhost/newtab")
}

fn newtab_url() -> Url {
    #[cfg(windows)]
    let s = "http://tauri.localhost/newtab.html";
    #[cfg(not(windows))]
    let s = "tauri://localhost/newtab.html";
    Url::parse(s).unwrap()
}

// ---------- state snapshot sent to the UI ----------

#[derive(Serialize, Clone)]
struct TabInfo {
    id: u32,
    url: String,
    title: String,
    discarded: bool,
    loading: bool,
    pinned: bool,
    account: u32,
    name: Option<String>,
}

#[derive(Serialize, Clone)]
struct Snapshot {
    tabs: Vec<TabInfo>,
    active: Option<u32>,
    sidebar: Option<String>,
    game_mode: bool,
    fullscreen: bool,
    adblock_active: bool,
    settings: Settings,
}

fn snapshot(b: &Browser) -> Snapshot {
    Snapshot {
        tabs: b
            .tabs
            .iter()
            .map(|t| TabInfo {
                id: t.id,
                url: if is_newtab(&t.url) { String::new() } else { t.url.clone() },
                title: if is_newtab(&t.url) { "New Tab".into() } else { t.title.clone() },
                discarded: t.discarded,
                loading: t.loading,
                pinned: t.pinned,
                account: t.account,
                name: t.name.clone(),
            })
            .collect(),
        active: b.active,
        sidebar: b.sidebar.clone(),
        game_mode: b.game_mode,
        fullscreen: b.fullscreen,
        adblock_active: adblock::enabled(),
        settings: b.settings.clone(),
    }
}

/// Never call while holding the state lock.
pub fn emit_state(app: &AppHandle) {
    let snap = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        snapshot(&b)
    };
    let _ = app.emit("state", snap);
}

fn update_tab(app: &AppHandle, id: u32, f: impl FnOnce(&mut Tab)) {
    {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        match b.tabs.iter_mut().find(|t| t.id == id) {
            Some(t) => f(t),
            None => return,
        }
    }
    emit_state(app);
}

// ---------- layout ----------

/// Positions the active tab and open sidebar panel; hides everything else
/// (hidden WebView2 instances are throttled by Chromium, saving CPU/GPU).
fn relayout(app: &AppHandle) {
    let (active, sidebar, overlay, full) = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        (b.active, b.sidebar.clone(), b.overlay, b.fullscreen)
    };
    let Some(win) = app.get_window("main") else { return };
    let (Ok(size), Ok(scale)) = (win.inner_size(), win.scale_factor()) else { return };
    let size = size.to_logical::<f64>(scale);

    // Full-screen game view: no bars, no sidebar; the page gets every pixel.
    let sidebar = if full { None } else { sidebar };
    let (top, status, rail) = if full { (0.0, 0.0, 0.0) } else { (TOP, STATUS, RAIL) };
    let cx = rail + if sidebar.is_some() { PANEL } else { 0.0 };
    let ch = (size.height - top - status).max(50.0);
    let cw = (size.width - cx).max(100.0);

    for (label, wv) in app.webviews() {
        if let Some(id) = label.strip_prefix("tab-").and_then(|s| s.parse::<u32>().ok()) {
            if Some(id) == active && !overlay {
                let _ = wv.set_position(LogicalPosition::new(cx, top));
                let _ = wv.set_size(LogicalSize::new(cw, ch));
                let _ = wv.show();
                wv2::set_hibernate(&wv, false);
            } else {
                // Hibernate: hidden (Chromium stops painting and throttles timers)
                // and asked to use as little memory as possible. Still loaded.
                let _ = wv.hide();
                if !overlay {
                    wv2::set_hibernate(&wv, true);
                }
            }
        } else if let Some(panel) = label.strip_prefix("side-") {
            if sidebar.as_deref() == Some(panel) && !overlay {
                let _ = wv.set_position(LogicalPosition::new(RAIL, TOP));
                let _ = wv.set_size(LogicalSize::new(PANEL, ch));
                let _ = wv.show();
                wv2::set_hibernate(&wv, false);
            } else {
                let _ = wv.hide();
                if !overlay {
                    wv2::set_hibernate(&wv, true);
                }
            }
        }
    }
}

// ---------- webview creation ----------

fn base_builder(label: &str, url: WebviewUrl) -> WebviewBuilder<tauri::Wry> {
    let mut b = WebviewBuilder::new(label, url).additional_browser_args(launch_args());
    if adblock::enabled() {
        b = b.initialization_script(adblock::COSMETIC_JS);
    }
    b
}

/// Links with target=_blank open as a Respawn tab instead of a bare popup window.
const LINK_JS: &str = r#"
(() => {
  const open = (e) => {
    const a = e.target && e.target.closest && e.target.closest('a[href]');
    if (!a) return;
    const newTab = a.target === '_blank' || e.button === 1 || e.ctrlKey;
    if (!newTab || !/^https?:/i.test(a.href)) return;
    e.preventDefault(); e.stopPropagation();
    location.href = 'https://respawn.invalid/open?u=' + encodeURIComponent(a.href);
  };
  addEventListener('click', open, true);
  addEventListener('auxclick', (e) => { if (e.button === 1) open(e); }, true);
})();
"#;

/// Auto-recover from Unity WebGL game crashes (e.g. "RuntimeError: memory access
/// out of bounds"). Unity reports these with alert(); instead of leaving a dead
/// game behind the dialog, reload right away. Capped at 3 reloads per 2 minutes
/// so a game that crashes on startup can't loop forever.
const GAME_RECOVER_JS: &str = r#"
(() => {
  const KEY = '__respawn_crash_reloads';
  const isUnityCrash = (m) =>
    typeof m === 'string' && m.includes('error occurred running the Unity content');
  const nativeAlert = window.alert;
  window.alert = function (msg) {
    if (isUnityCrash(msg)) {
      try {
        const now = Date.now();
        const recent = JSON.parse(sessionStorage.getItem(KEY) || '[]').filter((t) => now - t < 120000);
        if (recent.length < 3) {
          recent.push(now);
          sessionStorage.setItem(KEY, JSON.stringify(recent));
          console.warn('[Respawn] Game crashed, reloading:', msg);
          setTimeout(() => location.reload(), 250);
          return;
        }
      } catch (e) {}
    }
    return nativeAlert.apply(this, arguments);
  };
})();
"#;

/// Right-click on a link, a Discord server or a channel shows Respawn's menu
/// ("Open in new tab", "More options…" for the site's own menu). Shift+right-click
/// skips straight to the site's menu.
const LINK_MENU_JS: &str = r#"
(() => {
  const open = window.open.bind(window);
  let bypass = false;
  const linkFor = (el) => {
    if (!el || !el.closest) return null;
    const a = el.closest('a[href]');
    if (a && /^https?:/i.test(a.href)) return a.href;
    // Discord server icons aren't links: build the server's address.
    const g = el.closest('[data-list-item-id^="guildsnav___"]');
    if (g && location.hostname.endsWith('discord.com')) {
      const id = (g.getAttribute('data-list-item-id') || '').split('___')[1];
      if (/^\d+$/.test(id)) return location.origin + '/channels/' + id;
      if (id === 'home') return location.origin + '/channels/@me';
    }
    return null;
  };
  addEventListener('contextmenu', (e) => {
    if (bypass || e.shiftKey) return;
    const url = linkFor(e.target);
    if (!url) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    window.__respawnCtx = { el: e.target, x: e.clientX, y: e.clientY };
    open('https://respawn.invalid/linkmenu?u=' + encodeURIComponent(url), '_blank');
  }, true);
  // "More options…": show the site's own right-click menu at the same spot.
  window.__respawnMore = () => {
    const c = window.__respawnCtx;
    if (!c) return;
    bypass = true;
    try {
      c.el.dispatchEvent(new MouseEvent('contextmenu', {
        bubbles: true, cancelable: true, composed: true, button: 2, buttons: 2, clientX: c.x, clientY: c.y,
      }));
    } finally { bypass = false; }
  };
})();
"#;

/// The link that was right-clicked: (webview label, url, login slot).
static PENDING_LINK: Mutex<Option<(String, String, u32)>> = Mutex::new(None);

/// Native right-click menu for a link (drawn above web content).
fn show_link_menu(app: &AppHandle, source: String, url: String, account: u32) {
    *PENDING_LINK.lock().unwrap() = Some((source, url, account));
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        let build = || -> tauri::Result<Menu<tauri::Wry>> {
            Menu::with_items(
                &a,
                &[
                    &MenuItem::with_id(&a, "link:open", "Open in new tab", true, None::<&str>)?,
                    &MenuItem::with_id(&a, "link:bg", "Open in new tab (stay here)", true, None::<&str>)?,
                    &PredefinedMenuItem::separator(&a)?,
                    &MenuItem::with_id(&a, "link:more", "More options…", true, None::<&str>)?,
                ],
            )
        };
        if let (Ok(menu), Some(win)) = (build(), a.get_window("main")) {
            let _ = win.popup_menu(&menu);
        }
    });
}

fn handle_link_menu(app: &AppHandle, menu_id: &str) {
    if !menu_id.starts_with("link:") {
        return;
    }
    let Some((source, url, account)) = PENDING_LINK.lock().unwrap().clone() else { return };
    let action = menu_id.to_string();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match action.as_str() {
            "link:open" => {
                add_tab(&app, &url, None, false, true, true, account);
            }
            "link:bg" => {
                add_tab(&app, &url, None, false, false, true, account);
                let _ = app.emit("toast", "Opened in a new tab");
            }
            "link:more" => {
                if let Some(wv) = app.get_webview(&source) {
                    let _ = wv.eval("window.__respawnMore && window.__respawnMore()");
                }
            }
            _ => {}
        }
    });
}

/// Handles the page-to-Respawn signals sent with window.open(); true if handled.
fn handle_bridge_window(app: &AppHandle, url: &Url, source: &str, account: u32) -> bool {
    if url.host_str() != Some(BRIDGE_HOST) {
        return false;
    }
    let get = |k: &str| url.query_pairs().find(|(key, _)| key == k).map(|(_, v)| v.into_owned());
    match url.path() {
        "/key" => {
            // A keyboard shortcut pressed inside a page or game.
            if let Some(action) = get("a").filter(|a| keys::is_action(a) && a != keys::GLOBAL_ACTION) {
                let a = app.clone();
                tauri::async_runtime::spawn(async move { keys::run_action(&a, &action) });
            }
        }
        "/acctmenu" => {
            // The full-screen account badge was clicked.
            let a = app.clone();
            tauri::async_runtime::spawn(async move { show_accounts_menu(&a) });
        }
        "/linkmenu" => {
            if let Some(target) = get("u").filter(|u| u.starts_with("http")) {
                show_link_menu(app, source.to_string(), target, account);
            }
        }
        _ => {}
    }
    true
}

/// Where an account slot keeps its cookies, logins and saved passwords.
/// Account 0 uses the default profile.
pub fn account_dir(app: &AppHandle, account: u32) -> Option<std::path::PathBuf> {
    if account == 0 {
        return None;
    }
    app.path().app_local_data_dir().ok().map(|d| d.join("accounts").join(format!("account-{}", account + 1)))
}

fn cycle_tab(app: &AppHandle, step: i32) {
    let next = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        let n = b.tabs.len() as i32;
        if n < 2 {
            return;
        }
        let i = b.tabs.iter().position(|t| Some(t.id) == b.active).unwrap_or(0) as i32;
        b.tabs[(i + step).rem_euclid(n) as usize].id
    };
    do_activate_tab(app, next);
    // Give the new tab keyboard focus so the next Q/W press reaches it.
    if let Some(wv) = app.get_webview(&format!("tab-{next}")) {
        let _ = wv.set_focus();
    }
}

enum Bridge {
    Open(String),
    Search(String),
}

fn bridge_request(u: &Url) -> Option<Bridge> {
    if u.host_str() != Some(BRIDGE_HOST) {
        return None;
    }
    let get = |k: &str| u.query_pairs().find(|(key, _)| key == k).map(|(_, v)| v.into_owned());
    match u.path() {
        "/open" => get("u").map(Bridge::Open),
        "/search" => get("q").map(Bridge::Search),
        _ => None,
    }
}

/// Sign-in pop-ups ("Continue with Google/Facebook/Discord") use window.open and
/// need window.opener to report back, so they must open as real pop-up windows
/// (not tabs). Pop-ups to blocked ad domains are denied.
fn popup_policy(url: Url) -> NewWindowResponse<tauri::Wry> {
    if adblock::is_blocked_url(&url) {
        NewWindowResponse::Deny
    } else {
        NewWindowResponse::Allow
    }
}

fn spawn_tab_webview(app: &AppHandle, id: u32, url: &str) {
    let Some(win) = app.get_window("main") else { return };
    let label = format!("tab-{id}");
    let wurl = match Url::parse(url) {
        Ok(u) if !is_newtab(url) => WebviewUrl::External(u),
        _ => WebviewUrl::App("newtab.html".into()),
    };
    let (account, binds) = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        (b.tabs.iter().find(|t| t.id == id).map(|t| t.account).unwrap_or(0), b.settings.keybinds.clone())
    };
    let (a1, a2, a3, a4) = (app.clone(), app.clone(), app.clone(), app.clone());

    let mut builder = base_builder(&label, wurl)
        .initialization_script(LINK_JS)
        .initialization_script(LINK_MENU_JS)
        .initialization_script(GAME_RECOVER_JS)
        .initialization_script(keys::FS_BADGE_JS)
        // Discord: chat-only view + translator (does nothing on other sites).
        .initialization_script(discord::script("tab"));
    // Your keyboard shortcuts work inside pages and games too.
    builder = builder.initialization_script(keys::page_script(&binds));
    if let Some(dir) = account_dir(app, account) {
        let _ = std::fs::create_dir_all(&dir);
        builder = builder.data_directory(dir);
    }
    let builder = builder
        .on_new_window(move |url, _features| {
            if handle_bridge_window(&a4, &url, &format!("tab-{id}"), account) {
                return NewWindowResponse::Deny;
            }
            popup_policy(url)
        })
        .on_navigation(move |u| {
            if let Some(req) = bridge_request(u) {
                let a = a1.clone();
                tauri::async_runtime::spawn(async move {
                    match req {
                        // Links opened from this tab stay signed in to the same account.
                        Bridge::Open(target) => {
                            add_tab(&a, &target, None, false, true, true, account);
                        }
                        Bridge::Search(q) => navigate_tab(&a, id, &q),
                    }
                });
                return false;
            }
            !adblock::is_blocked_url(u)
        })
        .on_page_load(move |_wv, payload| {
            let url = payload.url().to_string();
            let loading = matches!(payload.event(), PageLoadEvent::Started);
            if !loading {
                history::record(&a2, &url);
            }
            update_tab(&a2, id, |t| {
                t.url = url;
                t.loading = loading;
            });
            if !loading {
                keys::push_fs_badge(&a2);
            }
        })
        .on_document_title_changed(move |_wv, title| {
            update_tab(&a3, id, |t| t.title = title.clone());
            // Single-page apps (YouTube, Twitch) change URL without a page load;
            // a title change is a good moment to resync the address bar + history.
            let a = a3.clone();
            std::thread::spawn(move || {
                if let Some(u) = a.get_webview(&format!("tab-{id}")).and_then(|w| w.url().ok()) {
                    let s = u.to_string();
                    let mut changed = false;
                    update_tab(&a, id, |t| {
                        if t.url != s {
                            t.url = s.clone();
                            changed = true;
                        }
                    });
                    if changed {
                        history::record(&a, &s);
                    }
                    history::set_title(&a, &s, &title);
                }
            });
        });

    match win.add_child(builder, LogicalPosition::new(0.0, 0.0), LogicalSize::new(1.0, 1.0)) {
        Ok(wv) => wv2::configure(&wv),
        Err(e) => eprintln!("failed to create tab webview: {e}"),
    }
}

/// Sites a sidebar panel may navigate within (including their sign-in pages).
fn panel_allows(panel: &str, u: &Url) -> bool {
    let Some(host) = u.host_str() else { return true };
    let own: &[&str] = match panel {
        "discord" => &["discord.com", "discordapp.com", "discord.gg", "hcaptcha.com"],
        "twitch" => &["twitch.tv"],
        "youtube" => &["youtube.com", "youtu.be"],
        "steam" => &["steampowered.com", "steamcommunity.com"],
        _ => return true,
    };
    // Sign-in providers may be used by any panel.
    let login = ["accounts.google.com", "google.com", "appleid.apple.com", "facebook.com", "login.live.com"];
    let matches = |d: &&str| host == *d || host.ends_with(&format!(".{d}"));
    u.scheme() != "http" && u.scheme() != "https" || own.iter().any(|d| matches(d)) || login.iter().any(|d| matches(d))
}

fn ensure_panel(app: &AppHandle, panel: &str) {
    let label = format!("side-{panel}");
    if app.get_webview(&label).is_some() {
        return;
    }
    let Some(url) = PANELS.iter().find(|(p, _)| *p == panel).and_then(|(_, u)| Url::parse(u).ok()) else {
        return;
    };
    let Some(win) = app.get_window("main") else { return };
    let (a, a2) = (app.clone(), app.clone());
    let src = label.clone();
    let panel_id = panel.to_string();
    let mut builder = base_builder(&label, WebviewUrl::External(url))
        .initialization_script(LINK_JS)
        .initialization_script(LINK_MENU_JS);
    let binds = app.state::<Mutex<Browser>>().lock().unwrap().settings.keybinds.clone();
    builder = builder.initialization_script(keys::page_script(&binds));
    if panel == "discord" {
        // Sidebar shows servers + channels; picking a channel shows it in the chat tab.
        builder = builder.initialization_script(discord::script("panel"));
    }
    let builder = builder
        .on_new_window(move |url, _features| {
            if handle_bridge_window(&a2, &url, &src, 0) {
                return NewWindowResponse::Deny;
            }
            popup_policy(url)
        })
        .on_navigation(move |u| {
            if let Some(Bridge::Open(target)) = bridge_request(u) {
                let a = a.clone();
                tauri::async_runtime::spawn(async move {
                    open_tab(&a, &target, true);
                });
                return false;
            }
            if adblock::is_blocked_url(u) {
                return false;
            }
            // The panel stays on its own site (switching Discord servers and
            // channels happens right here). Links to other websites open as a tab.
            if !panel_allows(&panel_id, u) {
                let a = a.clone();
                let target = u.to_string();
                tauri::async_runtime::spawn(async move {
                    open_tab(&a, &target, true);
                });
                return false;
            }
            true
        });
    if let Ok(wv) = win.add_child(builder, LogicalPosition::new(0.0, 0.0), LogicalSize::new(1.0, 1.0)) {
        wv2::configure(&wv);
    }
}

// ---------- tab operations ----------

/// Turns address-bar input into a URL or a search.
fn resolve_input(input: &str, engine: &str) -> String {
    let t = input.trim();
    if t.is_empty() {
        return "newtab".into();
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("file:") {
        return t.into();
    }
    let looks_like_host = !t.contains(' ')
        && (lower.starts_with("localhost") || (t.contains('.') && !t.ends_with('.') && !t.starts_with('.')));
    if looks_like_host {
        format!("https://{t}")
    } else {
        settings::search_url(engine, t)
    }
}

pub fn open_tab(app: &AppHandle, url: &str, activate: bool) -> u32 {
    add_tab(app, url, None, false, activate, true, 0)
}

/// `load = false` creates the tab asleep (no webview until it's clicked);
/// used when restoring a session so startup stays fast.
fn add_tab(
    app: &AppHandle,
    url: &str,
    title: Option<&str>,
    pinned: bool,
    activate: bool,
    load: bool,
    account: u32,
) -> u32 {
    let load = load || activate;
    let id = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        let id = b.next_id;
        b.next_id += 1;
        let now = Instant::now();
        if activate {
            if let Some(prev) = b.active {
                if let Some(t) = b.tabs.iter_mut().find(|t| t.id == prev) {
                    t.last_active = now;
                }
            }
            b.active = Some(id);
        }
        b.tabs.push(Tab {
            id,
            url: url.into(),
            title: match title {
                Some(t) if !t.is_empty() => t.into(),
                _ if is_newtab(url) => "New Tab".into(),
                _ => url.into(),
            },
            discarded: !load,
            loading: load,
            last_active: now,
            pinned,
            account,
            name: None,
        });
        id
    };
    if load {
        spawn_tab_webview(app, id, url);
    }
    relayout(app);
    emit_state(app);
    id
}

fn navigate_tab(app: &AppHandle, id: u32, input: &str) {
    let target = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        resolve_input(input, &b.settings.search_engine)
    };
    let url = if is_newtab(&target) { Some(newtab_url()) } else { Url::parse(&target).ok() };
    let Some(url) = url else { return };
    update_tab(app, id, |t| {
        t.url = url.to_string();
        t.loading = true;
    });
    match app.get_webview(&format!("tab-{id}")) {
        #[allow(unused_mut)]
        Some(mut wv) => {
            let _ = wv.navigate(url);
        }
        None => {
            // Tab was asleep: wake it straight onto the new URL.
            update_tab(app, id, |t| t.discarded = false);
            spawn_tab_webview(app, id, url.as_str());
            relayout(app);
        }
    }
}

fn guard(wv: &Webview) -> Result<(), String> {
    if wv.label() == "ui" {
        Ok(())
    } else {
        Err("not allowed".into())
    }
}

fn active_id(app: &AppHandle) -> Option<u32> {
    app.state::<Mutex<Browser>>().lock().unwrap().active
}

// ---------- commands (only the 'ui' webview may call these) ----------

#[tauri::command]
async fn get_state(webview: Webview, state: State<'_>) -> Result<Snapshot, String> {
    guard(&webview)?;
    let b = state.lock().unwrap();
    Ok(snapshot(&b))
}

#[tauri::command]
async fn new_tab(app: AppHandle, webview: Webview, url: Option<String>) -> Result<u32, String> {
    guard(&webview)?;
    let target = match url {
        Some(u) => {
            let engine = app.state::<Mutex<Browser>>().lock().unwrap().settings.search_engine.clone();
            resolve_input(&u, &engine)
        }
        None => "newtab".into(),
    };
    let account = new_tab_slot(&app);
    Ok(add_tab(&app, &target, None, false, true, true, account))
}

/// A brand-new, empty login storage slot (each tab keeps its own logins).
fn new_slot(app: &AppHandle) -> u32 {
    let s = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        let n = b.settings.accounts.max(1);
        b.settings.accounts = n + 1;
        b.settings.clone()
    };
    settings::save(app, &s);
    s.accounts - 1
}

/// Slot for a tab you open yourself: its own storage, or the shared one if
/// "separate logins per tab" is turned off.
fn new_tab_slot(app: &AppHandle) -> u32 {
    let isolate = app.state::<Mutex<Browser>>().lock().unwrap().settings.isolate_tabs;
    if isolate {
        new_slot(app)
    } else {
        0
    }
}

#[tauri::command]
async fn activate_tab(app: AppHandle, webview: Webview, id: u32) -> Result<(), String> {
    guard(&webview)?;
    do_activate_tab(&app, id);
    Ok(())
}

fn do_activate_tab(app: &AppHandle, id: u32) {
    let wake = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        if !b.tabs.iter().any(|t| t.id == id) {
            return;
        }
        let now = Instant::now();
        if let Some(prev) = b.active {
            if let Some(t) = b.tabs.iter_mut().find(|t| t.id == prev) {
                t.last_active = now;
            }
        }
        b.active = Some(id);
        let t = b.tabs.iter_mut().find(|t| t.id == id).unwrap();
        t.last_active = now;
        if t.discarded {
            t.discarded = false;
            Some(t.url.clone())
        } else {
            None
        }
    };
    if let Some(url) = wake {
        spawn_tab_webview(app, id, &url);
    }
    relayout(app);
    emit_state(app);
    keys::push_fs_badge(app);
}

#[tauri::command]
async fn close_tab(app: AppHandle, webview: Webview, id: u32) -> Result<(), String> {
    guard(&webview)?;
    do_close_tab(&app, id);
    Ok(())
}

fn do_close_tab(app: &AppHandle, id: u32) {
    let empty = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        if let Some(pos) = b.tabs.iter().position(|t| t.id == id) {
            b.tabs.remove(pos);
            if b.active == Some(id) {
                b.active = b.tabs.get(pos.min(b.tabs.len().saturating_sub(1))).map(|t| t.id);
            }
        }
        b.tabs.is_empty()
    };
    if let Some(wv) = app.get_webview(&format!("tab-{id}")) {
        let _ = wv.close();
    }
    if empty {
        open_tab(app, "newtab", true);
        return;
    }
    // The newly active tab may have been asleep.
    if let Some(next) = active_id(app) {
        let wake = {
            let st = app.state::<Mutex<Browser>>();
            let mut b = st.lock().unwrap();
            b.tabs.iter_mut().find(|t| t.id == next && t.discarded).map(|t| {
                t.discarded = false;
                t.url.clone()
            })
        };
        if let Some(url) = wake {
            spawn_tab_webview(app, next, &url);
        }
    }
    relayout(app);
    emit_state(app);
}

#[tauri::command]
async fn navigate(app: AppHandle, webview: Webview, input: String) -> Result<(), String> {
    guard(&webview)?;
    match active_id(&app) {
        Some(id) => navigate_tab(&app, id, &input),
        None => {
            let engine = app.state::<Mutex<Browser>>().lock().unwrap().settings.search_engine.clone();
            open_tab(&app, &resolve_input(&input, &engine), true);
        }
    }
    Ok(())
}

#[tauri::command]
async fn nav_action(app: AppHandle, webview: Webview, action: String) -> Result<(), String> {
    guard(&webview)?;
    let Some(id) = active_id(&app) else { return Ok(()) };
    if action == "home" {
        navigate_tab(&app, id, "");
        return Ok(());
    }
    let js = match action.as_str() {
        "back" => "history.back()",
        "forward" => "history.forward()",
        "reload" => "location.reload()",
        "stop" => "window.stop()",
        _ => return Ok(()),
    };
    if let Some(wv) = app.get_webview(&format!("tab-{id}")) {
        let _ = wv.eval(js);
    }
    Ok(())
}

#[tauri::command]
async fn toggle_sidebar(app: AppHandle, webview: Webview, panel: String) -> Result<(), String> {
    guard(&webview)?;
    let open = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        b.sidebar = if b.sidebar.as_deref() == Some(panel.as_str()) { None } else { Some(panel.clone()) };
        b.sidebar.is_some()
    };
    if open {
        ensure_panel(&app, &panel);
    }
    relayout(&app);
    emit_state(&app);
    Ok(())
}

#[tauri::command]
async fn toggle_game_mode(app: AppHandle, webview: Webview) -> Result<(), String> {
    guard(&webview)?;
    let on = !app.state::<Mutex<Browser>>().lock().unwrap().game_mode;
    perf::set_game_mode(&app, on);
    Ok(())
}

#[tauri::command]
async fn save_settings(app: AppHandle, webview: Webview, settings: Settings) -> Result<(), String> {
    guard(&webview)?;
    let mut settings = settings;
    keys::complete(&mut settings.keybinds);
    settings::save(&app, &settings);
    app.state::<Mutex<Browser>>().lock().unwrap().settings = settings;
    emit_state(&app);
    discord::refresh_pages(&app);
    keys::apply(&app);
    Ok(())
}

/// "Keep loaded" pin: pinned tabs hibernate in the background but are never unloaded.
#[tauri::command]
async fn toggle_pin(app: AppHandle, webview: Webview, id: u32) -> Result<(), String> {
    guard(&webview)?;
    do_toggle_pin(&app, id);
    Ok(())
}

fn do_toggle_pin(app: &AppHandle, id: u32) {
    let wake = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        let Some(t) = b.tabs.iter_mut().find(|t| t.id == id) else { return };
        t.pinned = !t.pinned;
        let _ = app.emit("toast", if t.pinned { "Keep loaded: this tab is never unloaded" } else { "Tab may be unloaded when auto-unload is on" });
        // Pinning a sleeping tab loads it so it's ready when you come back.
        if t.pinned && t.discarded {
            t.discarded = false;
            Some(t.url.clone())
        } else {
            None
        }
    };
    if let Some(url) = wake {
        spawn_tab_webview(app, id, &url);
        relayout(app);
    }
    emit_state(app);
}

/// Drag-and-drop reordering from the tab strip.
#[tauri::command]
async fn move_tab(app: AppHandle, webview: Webview, id: u32, index: usize) -> Result<(), String> {
    guard(&webview)?;
    {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        if let Some(from) = b.tabs.iter().position(|t| t.id == id) {
            let t = b.tabs.remove(from);
            let to = index.min(b.tabs.len());
            b.tabs.insert(to, t);
        }
    }
    emit_state(&app);
    Ok(())
}

// ---------- tab right-click menu ----------

/// Shows a native context menu for a tab (native so it draws above web content).
#[tauri::command]
async fn tab_menu(app: AppHandle, webview: Webview, id: u32) -> Result<(), String> {
    guard(&webview)?;
    let (pinned, others) = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        let Some(t) = b.tabs.iter().find(|t| t.id == id) else { return Ok(()) };
        (t.pinned, b.tabs.len() > 1)
    };
    let a = app.clone();
    app.run_on_main_thread(move || {
        let build = || -> tauri::Result<Menu<tauri::Wry>> {
            Menu::with_items(
                &a,
                &[
                    &MenuItem::with_id(&a, format!("tab:rename:{id}"), "Rename tab…", true, None::<&str>)?,
                    &MenuItem::with_id(&a, format!("tab:dup:{id}"), "Duplicate tab", true, None::<&str>)?,
                    &MenuItem::with_id(&a, format!("tab:reload:{id}"), "Reload", true, None::<&str>)?,
                    &CheckMenuItem::with_id(&a, format!("tab:pin:{id}"), "Keep loaded", true, pinned, None::<&str>)?,
                    &PredefinedMenuItem::separator(&a)?,
                    &MenuItem::with_id(&a, format!("tab:freshlogin:{id}"), "Log in with a different account (fresh login)", true, None::<&str>)?,
                    &PredefinedMenuItem::separator(&a)?,
                    &MenuItem::with_id(&a, format!("tab:close:{id}"), "Close tab", true, None::<&str>)?,
                    &MenuItem::with_id(&a, format!("tab:others:{id}"), "Close other tabs", others, None::<&str>)?,
                ],
            )
        };
        if let (Ok(menu), Some(win)) = (build(), a.get_window("main")) {
            let _ = win.popup_menu(&menu);
        }
    })
    .map_err(|e| e.to_string())
}

fn handle_tab_menu(app: &AppHandle, menu_id: &str) {
    let mut parts = menu_id.split(':');
    if parts.next() != Some("tab") {
        return;
    }
    let (Some(action), Some(id)) = (parts.next().map(str::to_string), parts.next().and_then(|s| s.parse::<u32>().ok()))
    else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match action.as_str() {
            "rename" => {
                let (name, slot) = {
                    let st = app.state::<Mutex<Browser>>();
                    let b = st.lock().unwrap();
                    b.tabs
                        .iter()
                        .find(|t| t.id == id)
                        .map(|t| (t.name.clone().unwrap_or_default(), t.account))
                        .unwrap_or_default()
                };
                // The name is typed (or an account picked) in a box in the browser UI.
                let _ = app.emit("rename-tab", serde_json::json!({ "id": id, "name": name, "slot": slot }));
            }
            "dup" => duplicate_tab(&app, id),
            "reload" => {
                if let Some(wv) = app.get_webview(&format!("tab-{id}")) {
                    let _ = wv.eval("location.reload()");
                } else {
                    do_activate_tab(&app, id);
                }
            }
            "pin" => do_toggle_pin(&app, id),
            "freshlogin" => {
                let slot = new_slot(&app);
                set_tab_account(&app, id, slot);
                let _ = app.emit("toast", "This tab now has its own empty login — sign in with the account you want");
            }
            "close" => do_close_tab(&app, id),
            "others" => {
                do_activate_tab(&app, id);
                let others: Vec<u32> = {
                    let st = app.state::<Mutex<Browser>>();
                    let b = st.lock().unwrap();
                    b.tabs.iter().filter(|t| t.id != id).map(|t| t.id).collect()
                };
                for other in others {
                    do_close_tab(&app, other);
                }
            }
            _ => {}
        }
    });
}

/// Renames one tab (other tabs are not affected) and saves the name as an
/// account. `slot` = an existing saved account picked from the dropdown: the
/// tab switches to that account's login.
#[tauri::command]
async fn rename_tab(app: AppHandle, webview: Webview, id: u32, name: String, slot: Option<u32>) -> Result<(), String> {
    guard(&webview)?;
    let name: String = name.trim().chars().take(40).collect();

    if let Some(slot) = slot {
        // Already open in another tab? Go there instead of logging in twice.
        let other = {
            let st = app.state::<Mutex<Browser>>();
            let b = st.lock().unwrap();
            b.tabs.iter().find(|t| t.account == slot && t.id != id).map(|t| t.id)
        };
        if let Some(other) = other {
            do_activate_tab(&app, other);
            let _ = app.emit("toast", "That account is already open — switched to its tab");
            return Ok(());
        }
        set_tab_account(&app, id, slot);
    }
    update_tab(&app, id, |t| t.name = (!name.is_empty()).then(|| name.clone()));
    if !name.is_empty() {
        save_account_for_tab(&app, id, &name);
    }
    keys::push_fs_badge(&app);
    Ok(())
}

/// Records (or renames) the saved account for a tab's login slot.
fn save_account_for_tab(app: &AppHandle, id: u32, name: &str) {
    let s = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        let Some((slot, url)) = b.tabs.iter().find(|t| t.id == id).map(|t| (t.account, t.url.clone())) else { return };
        let url = if url.starts_with("http") { url } else { String::new() };
        match b.settings.saved_accounts.iter_mut().find(|a| a.slot == slot) {
            Some(a) => {
                a.name = name.into();
                if !url.is_empty() {
                    a.url = url;
                }
            }
            None => b.settings.saved_accounts.push(settings::SavedAccount { slot, name: name.into(), url }),
        }
        b.settings.clone()
    };
    settings::save(app, &s);
    emit_state(app);
}

/// Opens a saved account: switches to its tab if open, otherwise opens a new
/// tab signed in to it.
fn open_account(app: &AppHandle, slot: u32) {
    let (open, acct) = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        (
            b.tabs.iter().find(|t| t.account == slot).map(|t| t.id),
            b.settings.saved_accounts.iter().find(|a| a.slot == slot).cloned(),
        )
    };
    if let Some(id) = open {
        do_activate_tab(app, id);
        return;
    }
    let Some(acct) = acct else { return };
    let url = if acct.url.is_empty() { "https://totalbattle.com/en/".to_string() } else { acct.url };
    let id = add_tab(app, &url, None, false, true, true, slot);
    update_tab(app, id, |t| t.name = Some(acct.name.clone()));
}

fn forget_account(app: &AppHandle, slot: u32) {
    let s = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        b.settings.saved_accounts.retain(|a| a.slot != slot);
        b.settings.clone()
    };
    settings::save(app, &s);
    let _ = app.emit("toast", "Account forgotten (its login is deleted once no tab uses it)");
    emit_state(app);
}

/// The toolbar's Accounts ▾ menu.
#[tauri::command]
async fn accounts_menu(app: AppHandle, webview: Webview) -> Result<(), String> {
    guard(&webview)?;
    show_accounts_menu(&app);
    Ok(())
}

/// Saved accounts menu (toolbar button, or the full-screen account badge).
pub fn show_accounts_menu(app: &AppHandle) {
    let (accounts, open_slots, active_slot, full, others) = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        let open: std::collections::HashSet<u32> = b.tabs.iter().map(|t| t.account).collect();
        let active = b.active.and_then(|a| b.tabs.iter().find(|t| t.id == a)).map(|t| t.account);
        // In full screen also list open tabs that aren't saved accounts.
        let others: Vec<(u32, String, bool)> = b
            .tabs
            .iter()
            .filter(|t| !b.settings.saved_accounts.iter().any(|a| a.slot == t.account))
            .map(|t| (t.id, t.name.clone().unwrap_or_else(|| t.title.clone()), Some(t.id) == b.active))
            .collect();
        (b.settings.saved_accounts.clone(), open, active, b.fullscreen, others)
    };
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        let build = || -> tauri::Result<Menu<tauri::Wry>> {
            let menu = Menu::new(&a)?;
            if accounts.is_empty() {
                menu.append(&MenuItem::with_id(&a, "acct:none", "No saved accounts yet — right-click a tab → Rename tab…", false, None::<&str>)?)?;
            }
            for acc in &accounts {
                let label = if open_slots.contains(&acc.slot) { format!("{}   (open)", acc.name) } else { acc.name.clone() };
                menu.append(&CheckMenuItem::with_id(&a, format!("acct:open:{}", acc.slot), label, true, active_slot == Some(acc.slot), None::<&str>)?)?;
            }
            if full && !others.is_empty() {
                menu.append(&PredefinedMenuItem::separator(&a)?)?;
                for (id, label, on) in &others {
                    let label: String = label.chars().take(50).collect();
                    menu.append(&CheckMenuItem::with_id(&a, format!("acct:tab:{id}"), label, true, *on, None::<&str>)?)?;
                }
            }
            menu.append(&PredefinedMenuItem::separator(&a)?)?;
            menu.append(&MenuItem::with_id(&a, "acct:new", "New tab with a fresh login", true, None::<&str>)?)?;
            if !accounts.is_empty() {
                let forget = Submenu::new(&a, "Forget account", true)?;
                for acc in &accounts {
                    forget.append(&MenuItem::with_id(&a, format!("acct:forget:{}", acc.slot), &acc.name, true, None::<&str>)?)?;
                }
                menu.append(&forget)?;
            }
            if full {
                menu.append(&PredefinedMenuItem::separator(&a)?)?;
                menu.append(&MenuItem::with_id(&a, "acct:exitfs", "Exit full screen", true, None::<&str>)?)?;
            }
            Ok(menu)
        };
        if let (Ok(menu), Some(win)) = (build(), a.get_window("main")) {
            let _ = win.popup_menu(&menu);
        }
    });
}

fn handle_accounts_menu(app: &AppHandle, menu_id: &str) {
    let parts: Vec<&str> = menu_id.split(':').collect();
    if parts.first() != Some(&"acct") {
        return;
    }
    let app = app.clone();
    let action = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    let slot = parts.get(2).and_then(|s| s.parse::<u32>().ok());
    tauri::async_runtime::spawn(async move {
        match (action.as_str(), slot) {
            ("open", Some(slot)) => open_account(&app, slot),
            ("tab", Some(id)) => do_activate_tab(&app, id),
            ("exitfs", _) => keys::toggle_fullscreen(&app),
            ("forget", Some(slot)) => forget_account(&app, slot),
            ("new", _) => {
                let slot = new_slot(&app);
                add_tab(&app, "https://totalbattle.com/en/", None, false, true, true, slot);
            }
            _ => {}
        }
    });
}

/// Moves a tab to another login storage slot and reloads it there.
fn set_tab_account(app: &AppHandle, id: u32, account: u32) {
    let reload = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        let Some(t) = b.tabs.iter_mut().find(|t| t.id == id) else { return };
        if t.account == account {
            return;
        }
        t.account = account;
        t.loading = !t.discarded;
        (!t.discarded).then(|| t.url.clone())
    };
    if let Some(wv) = app.get_webview(&format!("tab-{id}")) {
        let _ = wv.close();
    }
    if let Some(url) = reload {
        spawn_tab_webview(app, id, &url);
        relayout(app);
    }
    emit_state(app);
}

/// Opens a copy of the tab right next to it and switches to it.
fn duplicate_tab(app: &AppHandle, id: u32) {
    let src = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        b.tabs.iter().find(|t| t.id == id).map(|t| (t.url.clone(), t.title.clone(), t.account))
    };
    let Some((url, title, _account)) = src else { return };
    let account = new_tab_slot(app);
    let new_id = add_tab(app, &url, Some(&title), false, true, true, account);
    {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        let src_pos = b.tabs.iter().position(|t| t.id == id);
        let new_pos = b.tabs.iter().position(|t| t.id == new_id);
        if let (Some(sp), Some(np)) = (src_pos, new_pos) {
            let t = b.tabs.remove(np);
            let at = (sp + 1).min(b.tabs.len());
            b.tabs.insert(at, t);
        }
    }
    emit_state(app);
}

// ---------- screen capture & markup ----------

/// Captures what's on screen in the page area (the game included) and returns
/// it as raw RGBA: 4 bytes width, 4 bytes height (little-endian), then pixels.
#[tauri::command]
async fn capture_page(app: AppHandle, webview: Webview) -> Result<tauri::ipc::Response, String> {
    guard(&webview)?;
    let (sidebar_open, full) = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        (b.sidebar.is_some(), b.fullscreen)
    };
    let win = app.get_window("main").ok_or("no window")?;
    let pos = win.inner_position().map_err(|e| e.to_string())?;
    let size = win.inner_size().map_err(|e| e.to_string())?;
    let scale = win.scale_factor().map_err(|e| e.to_string())?;

    let (left, top, bottom) = if full {
        (0, 0, 0)
    } else {
        (
            ((RAIL + if sidebar_open { PANEL } else { 0.0 }) * scale).round() as i32,
            (TOP * scale).round() as i32,
            (STATUS * scale).round() as i32,
        )
    };
    let w = size.width as i32 - left;
    let h = size.height as i32 - top - bottom;

    let rgba = capture::grab(pos.x + left, pos.y + top, w, h).ok_or("Screen capture failed")?;
    let mut out = Vec::with_capacity(rgba.len() + 8);
    out.extend_from_slice(&(w as u32).to_le_bytes());
    out.extend_from_slice(&(h as u32).to_le_bytes());
    out.extend_from_slice(&rgba);
    Ok(tauri::ipc::Response::new(out))
}

/// Saves a finished markup (PNG bytes in the request body) to Pictures\Respawn.
#[tauri::command]
async fn save_capture(app: AppHandle, webview: Webview, request: tauri::ipc::Request<'_>) -> Result<String, String> {
    guard(&webview)?;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected image bytes".into());
    };
    let name = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map(capture::clean_name)
        .unwrap_or_else(|| capture::clean_name(""));
    let dir = capture::captures_dir(&app).ok_or("no Pictures folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut path = dir.join(&name);
    let mut n = 2;
    while path.exists() {
        path = dir.join(name.replace(".png", &format!(" ({n}).png")));
        n += 1;
    }
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
async fn open_captures_folder(app: AppHandle, webview: Webview) -> Result<(), String> {
    guard(&webview)?;
    let dir = capture::captures_dir(&app).ok_or("no Pictures folder")?;
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
    Ok(())
}

// ---------- version & "What's new" ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    last_seen: String,
}

#[tauri::command]
async fn app_info(app: AppHandle, webview: Webview) -> Result<AppInfo, String> {
    guard(&webview)?;
    let last_seen = app.state::<Mutex<Browser>>().lock().unwrap().settings.last_seen_version.clone();
    Ok(AppInfo { version: app.package_info().version.to_string(), last_seen })
}

/// Called once the "What's new" notes for this version have been shown.
#[tauri::command]
async fn mark_version_seen(app: AppHandle, webview: Webview) -> Result<(), String> {
    guard(&webview)?;
    let version = app.package_info().version.to_string();
    let s = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        b.settings.last_seen_version = version;
        b.settings.clone()
    };
    settings::save(&app, &s);
    Ok(())
}

/// A keyboard shortcut pressed in the browser bar.
#[tauri::command]
async fn run_action(app: AppHandle, webview: Webview, action: String) -> Result<(), String> {
    guard(&webview)?;
    if keys::is_action(&action) {
        keys::run_action(&app, &action);
    }
    Ok(())
}

/// Inline autofill for the address bar.
#[tauri::command]
async fn complete(app: AppHandle, webview: Webview, input: String) -> Result<Option<history::Completion>, String> {
    guard(&webview)?;
    Ok(history::complete(&app, &input))
}

#[tauri::command]
async fn clear_history(app: AppHandle, webview: Webview) -> Result<(), String> {
    guard(&webview)?;
    history::clear(&app);
    let _ = app.emit("toast", "Browsing history cleared");
    Ok(())
}

/// The UI calls this while a modal is open so web content doesn't cover it.
#[tauri::command]
async fn set_overlay(app: AppHandle, webview: Webview, open: bool) -> Result<(), String> {
    guard(&webview)?;
    app.state::<Mutex<Browser>>().lock().unwrap().overlay = open;
    relayout(&app);
    Ok(())
}

// ---------- global hotkeys (work while a game has focus) ----------

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let app = app.clone();
                    let sc = shortcut.clone();
                    tauri::async_runtime::spawn(async move {
                        // The Game Mode shortcut works in any program.
                        if keys::is_global(&sc) {
                            keys::run_action(&app, keys::GLOBAL_ACTION);
                        }
                    });
                })
                .build(),
        )
        .manage(Mutex::new(Browser {
            tabs: vec![],
            active: None,
            next_id: 1,
            sidebar: None,
            game_mode: false,
            fullscreen: false,
            overlay: false,
            settings: Settings::default(),
        }))
        .invoke_handler(tauri::generate_handler![
            get_state,
            new_tab,
            activate_tab,
            close_tab,
            navigate,
            nav_action,
            toggle_sidebar,
            toggle_game_mode,
            save_settings,
            set_overlay,
            complete,
            clear_history,
            toggle_pin,
            tab_menu,
            move_tab,
            rename_tab,
            accounts_menu,
            capture_page,
            save_capture,
            open_captures_folder,
            discord::discord_prefs,
            discord::discord_set_prefs,
            discord::discord_translate,
            discord::discord_navigated,
            run_action,
            app_info,
            mark_version_seen
        ])
        .on_menu_event(|app, event| {
            handle_tab_menu(app, event.id().as_ref());
            handle_accounts_menu(app, event.id().as_ref());
            handle_link_menu(app, event.id().as_ref());
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let s = settings::load(&handle);
            let _ = LAUNCH_ARGS.set(adblock::init(&s));
            app.state::<Mutex<Browser>>().lock().unwrap().settings = s;
            app.manage(Mutex::new(history::load(&handle)));

            let win = tauri::window::WindowBuilder::new(app, "main")
                .title("Respawn · The Gamers Browser")
                .inner_size(1360.0, 860.0)
                .min_inner_size(900.0, 560.0)
                .build()?;

            // The browser chrome fills the window; tabs and panels sit on top of it.
            let ui = WebviewBuilder::new("ui", WebviewUrl::App("index.html".into()))
                .additional_browser_args(launch_args())
                .auto_resize();
            win.add_child(ui, PhysicalPosition::new(0, 0), win.inner_size()?)?;

            let h = handle.clone();
            win.on_window_event(move |event| match event {
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    let h = h.clone();
                    tauri::async_runtime::spawn(async move { relayout(&h) });
                }
                WindowEvent::Destroyed => {
                    history::flush(&h);
                    session::save(&h);
                    h.exit(0)
                }
                _ => {}
            });

            // Reopen last session's tabs: the active one and any "Keep loaded"
            // tabs load now, the rest stay asleep until clicked.
            let restore = app.state::<Mutex<Browser>>().lock().unwrap().settings.restore_session;
            let mut saved = if restore { session::load(&handle) } else { session::Session::default() };
            session::prepare_slots(&handle, &mut saved);
            if saved.tabs.is_empty() {
                open_tab(&handle, "newtab", true);
            } else {
                let active = saved.active.min(saved.tabs.len() - 1);
                let mut active_id = None;
                for (i, t) in saved.tabs.iter().enumerate() {
                    let id = add_tab(&handle, &t.url, Some(&t.title), t.pinned, false, t.pinned, t.account);
                    if t.name.is_some() {
                        update_tab(&handle, id, |tab| tab.name = t.name.clone());
                    }
                    if i == active {
                        active_id = Some(id);
                    }
                }
                if let Some(id) = active_id {
                    let wake = {
                        let st = app.state::<Mutex<Browser>>();
                        let mut b = st.lock().unwrap();
                        b.active = Some(id);
                        b.tabs.iter_mut().find(|t| t.id == id && t.discarded).map(|t| {
                            t.discarded = false;
                            t.loading = true;
                            t.url.clone()
                        })
                    };
                    if let Some(url) = wake {
                        spawn_tab_webview(&handle, id, &url);
                    }
                    relayout(&handle);
                    emit_state(&handle);
                }
            }

            let game_keys = app.state::<Mutex<Browser>>().lock().unwrap().settings.keybinds
                .get(keys::GLOBAL_ACTION).cloned().unwrap_or_default();
            keys::register_global(&handle, &game_keys);

            perf::start_monitor(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Respawn");
}
