//! Remembers open tabs so Respawn reopens them (and your game) after a restart.

use crate::{is_newtab, Browser};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone)]
pub struct SavedTab {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub account: u32,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Session {
    pub tabs: Vec<SavedTab>,
    /// Index into `tabs` of the tab that was active.
    pub active: usize,
}

static LAST_WRITTEN: Mutex<String> = Mutex::new(String::new());

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("session.json"))
}

pub fn load(app: &AppHandle) -> Session {
    file(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Remembers the page each named account is on, so reopening it later goes back there.
fn sync_account_urls(app: &AppHandle) {
    let changed = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        let pages: Vec<(u32, String)> = b
            .tabs
            .iter()
            .filter(|t| t.url.starts_with("http") && !t.discarded)
            .map(|t| (t.account, t.url.clone()))
            .collect();
        let mut changed = false;
        for (slot, url) in pages {
            if let Some(a) = b.settings.saved_accounts.iter_mut().find(|a| a.slot == slot) {
                if a.url != url {
                    a.url = url;
                    changed = true;
                }
            }
        }
        changed.then(|| b.settings.clone())
    };
    if let Some(s) = changed {
        crate::settings::save(app, &s);
    }
}

/// Saves the current tabs if they changed since the last save.
pub fn save(app: &AppHandle) {
    sync_account_urls(app);
    let session = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        let mut s = Session::default();
        for t in b.tabs.iter().filter(|t| !is_newtab(&t.url) && t.url.starts_with("http")) {
            if Some(t.id) == b.active {
                s.active = s.tabs.len();
            }
            s.tabs.push(SavedTab { url: t.url.clone(), title: t.title.clone(), pinned: t.pinned, account: t.account, name: t.name.clone() });
        }
        s
    };
    let Ok(json) = serde_json::to_string(&session) else { return };
    let mut last = LAST_WRITTEN.lock().unwrap();
    if *last == json {
        return;
    }
    if let Some(p) = file(app) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(p, &json).is_ok() {
            *last = json;
        }
    }
}

/// Runs once at startup, before any webview exists:
/// - one-time upgrade: tabs that used to share the main login each get their
///   own storage (the first keeps the existing login), so every tab can stay
///   signed in to a different account;
/// - keeps the slot counter ahead of every slot in use;
/// - deletes login storage of tabs that were closed.
pub fn prepare_slots(app: &AppHandle, saved: &mut Session) {
    let (isolate, migrated, mut next) = {
        let st = app.state::<Mutex<Browser>>();
        let b = st.lock().unwrap();
        (b.settings.isolate_tabs, b.settings.isolation_migrated, b.settings.accounts.max(1))
    };
    let max_used = saved.tabs.iter().map(|t| t.account).max().unwrap_or(0);
    next = next.max(max_used + 1);

    if isolate && !migrated {
        let mut first_shared = true;
        for t in saved.tabs.iter_mut().filter(|t| t.account == 0) {
            if first_shared {
                first_shared = false;
            } else {
                t.account = next;
                next += 1;
            }
        }
    }

    let s = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        b.settings.accounts = next;
        if isolate {
            b.settings.isolation_migrated = true;
        }
        b.settings.clone()
    };
    crate::settings::save(app, &s);

    // Remove storage folders no open tab uses any more.
    let mut used: std::collections::HashSet<u32> = saved.tabs.iter().map(|t| t.account).collect();
    // Named accounts keep their login even with no tab open.
    used.extend(s.saved_accounts.iter().map(|a| a.slot));
    if let Some(root) = crate::account_dir(app, 1).and_then(|d| d.parent().map(|p| p.to_path_buf())) {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(n) = name.strip_prefix("account-").and_then(|n| n.parse::<u32>().ok()) {
                    // Folder "account-N" belongs to slot N-1.
                    if n >= 2 && !used.contains(&(n - 1)) {
                        let _ = std::fs::remove_dir_all(e.path());
                    }
                }
            }
        }
    }
}
