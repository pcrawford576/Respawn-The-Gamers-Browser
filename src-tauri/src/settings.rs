use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// A named login you can reopen even after its tab was closed.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct SavedAccount {
    /// Login storage slot (see `account_dir`).
    pub slot: u32,
    pub name: String,
    /// Page to open it on (last page the account's tab was on).
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// duckduckgo | google | bing | brave
    pub search_engine: String,
    /// Network-level ad/tracker blocking (applies on restart).
    pub adblock: bool,
    /// Automatically unload background tabs (RAM cap, idle timer, Game Mode).
    /// Off by default: background tabs hibernate but stay loaded.
    pub auto_unload: bool,
    /// When the browser uses more RAM than this, the least-recently-used
    /// background tab is put to sleep.
    pub mem_cap_mb: u64,
    /// Tighter cap used while Game Mode is on.
    pub game_mem_cap_mb: u64,
    /// Sleep background tabs untouched for this many minutes (0 = never).
    pub discard_idle_min: u64,
    /// Max renderer processes (0 = Chromium default). Applies on restart.
    pub renderer_limit: u32,
    /// Extra domains to block, one per entry. Applies on restart.
    pub extra_blocked: Vec<String>,
    /// Remember visited sites for address-bar autofill.
    pub save_history: bool,
    /// Reopen the tabs you had open last time.
    pub restore_session: bool,
    /// Q / W switch to the previous / next tab, even inside pages.
    pub tab_keys: bool,
    /// Number of account slots created (1 = only the main account).
    pub accounts: u32,
    /// Names you gave account slots (older versions; no longer shown on tabs).
    pub account_names: std::collections::BTreeMap<u32, String>,
    /// Every tab you open gets its own login storage, so tabs can be signed in
    /// to different accounts of the same site.
    pub isolate_tabs: bool,
    /// One-time upgrade of tabs that used to share the main login has run.
    pub isolation_migrated: bool,
    /// Named accounts (logins kept even when their tab is closed).
    pub saved_accounts: Vec<SavedAccount>,
    /// Discord: servers/channels in the Chat sidebar, conversation in one tab.
    pub discord_split: bool,
    /// Your DeepL API key (stored only in this settings file on your PC).
    pub deepl_key: String,
    /// Language incoming chat is translated into (DeepL code, e.g. EN-US).
    pub translate_to: String,
    /// Language Alt+T translates your own message into.
    pub reply_lang: String,
    /// Translate new chat messages automatically.
    pub auto_translate: bool,
    /// Keyboard shortcuts: action -> keys (e.g. "next_tab" -> "W").
    pub keybinds: std::collections::BTreeMap<String, String>,
    /// Last version whose "What's new" notes you've seen.
    pub last_seen_version: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            search_engine: "duckduckgo".into(),
            adblock: true,
            auto_unload: false,
            mem_cap_mb: 3000,
            game_mem_cap_mb: 1500,
            discard_idle_min: 30,
            renderer_limit: 0,
            extra_blocked: vec![],
            save_history: true,
            restore_session: true,
            tab_keys: true,
            accounts: 1,
            account_names: Default::default(),
            isolate_tabs: true,
            isolation_migrated: false,
            saved_accounts: vec![],
            discord_split: true,
            deepl_key: String::new(),
            translate_to: "EN-US".into(),
            reply_lang: "DE".into(),
            auto_translate: true,
            keybinds: crate::keys::defaults(),
            last_seen_version: String::new(),
        }
    }
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    let mut s: Settings = path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    crate::keys::complete(&mut s.keybinds);
    s
}

pub fn save(app: &AppHandle, s: &Settings) {
    if let Some(p) = path(app) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(s) {
            let _ = std::fs::write(p, json);
        }
    }
}

pub fn search_url(engine: &str, query: &str) -> String {
    let q: String = query
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect();
    match engine {
        "google" => format!("https://www.google.com/search?q={q}"),
        "bing" => format!("https://www.bing.com/search?q={q}"),
        "brave" => format!("https://search.brave.com/search?q={q}"),
        _ => format!("https://duckduckgo.com/?q={q}"),
    }
}
