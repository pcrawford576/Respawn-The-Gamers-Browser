//! Browsing history + address-bar autofill.
//!
//! Every page that finishes loading is recorded (URL, title, visit count, last
//! visit). Typing in the address bar asks `complete`, which picks the best
//! match by visits and recency, the way Chrome/Edge inline autocomplete does:
//! "twi" -> "twitch.tv", "twitch.tv/d" -> "twitch.tv/directory".

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const MAX_ENTRIES: usize = 5000;

#[derive(Serialize, Deserialize, Clone)]
pub struct Entry {
    pub url: String,
    pub title: String,
    pub visits: u32,
    /// Unix seconds of the last visit.
    pub last: u64,
}

#[derive(Default)]
pub struct History {
    entries: Vec<Entry>,
    dirty: bool,
}

#[derive(Serialize, Clone)]
pub struct Completion {
    /// What the address bar should show (starts with what was typed).
    pub text: String,
    /// Where Enter should go.
    pub url: String,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("history.json"))
}

pub fn load(app: &AppHandle) -> History {
    let entries = file(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    History { entries, dirty: false }
}

/// Writes history to disk if it changed. Called every few seconds and on exit.
pub fn flush(app: &AppHandle) {
    let json = {
        let st = app.state::<Mutex<History>>();
        let mut h = st.lock().unwrap();
        if !h.dirty {
            return;
        }
        h.dirty = false;
        serde_json::to_string(&h.entries).ok()
    };
    if let (Some(p), Some(json)) = (file(app), json) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, json);
    }
}

fn enabled(app: &AppHandle) -> bool {
    app.state::<Mutex<crate::Browser>>().lock().unwrap().settings.save_history
}

fn worth_saving(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && !url.contains("tauri.localhost")
        && !url.contains("respawn.invalid")
        && url.len() < 2048
}

pub fn record(app: &AppHandle, url: &str) {
    if !worth_saving(url) || !enabled(app) {
        return;
    }
    let st = app.state::<Mutex<History>>();
    let mut h = st.lock().unwrap();
    let t = now();
    match h.entries.iter_mut().find(|e| e.url == url) {
        Some(e) => {
            // Ignore reload storms: count a visit at most once per 30s.
            if t.saturating_sub(e.last) > 30 {
                e.visits += 1;
            }
            e.last = t;
        }
        None => h.entries.push(Entry { url: url.into(), title: String::new(), visits: 1, last: t }),
    }
    if h.entries.len() > MAX_ENTRIES {
        if let Some(oldest) = h.entries.iter().enumerate().min_by_key(|(_, e)| e.last).map(|(i, _)| i) {
            h.entries.swap_remove(oldest);
        }
    }
    h.dirty = true;
}

pub fn set_title(app: &AppHandle, url: &str, title: &str) {
    let st = app.state::<Mutex<History>>();
    let mut h = st.lock().unwrap();
    if let Some(e) = h.entries.iter_mut().find(|e| e.url == url) {
        if e.title != title {
            e.title = title.into();
            h.dirty = true;
        }
    }
}

pub fn clear(app: &AppHandle) {
    {
        let st = app.state::<Mutex<History>>();
        let mut h = st.lock().unwrap();
        h.entries.clear();
        h.dirty = true;
    }
    flush(app);
}

fn recency_bonus(age_secs: u64) -> i64 {
    match age_secs {
        a if a < 86_400 => 30,
        a if a < 7 * 86_400 => 15,
        a if a < 30 * 86_400 => 5,
        _ => 0,
    }
}

pub fn complete(app: &AppHandle, input: &str) -> Option<Completion> {
    let typed = input.trim_start().to_ascii_lowercase();
    if typed.is_empty() || typed.contains(' ') {
        return None;
    }
    let st = app.state::<Mutex<History>>();
    let h = st.lock().unwrap();
    let t = now();

    // candidate text -> (score, url). Host-level candidates add up the scores of
    // every page on that site, so "twi" completes to the site, not one page.
    let mut best: HashMap<String, (i64, String)> = HashMap::new();
    for e in &h.entries {
        let score = e.visits as i64 * 10 + recency_bonus(t.saturating_sub(e.last));
        let (scheme, rest) = e.url.split_once("://").unwrap_or(("https", e.url.as_str()));
        let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
        let host_nw = host.strip_prefix("www.").unwrap_or(host);
        let rest_nw = rest.strip_prefix("www.").unwrap_or(rest);
        let site_url = format!("{scheme}://{host}/");

        let candidates = [
            (host_nw.to_string(), site_url.clone()),
            (host.to_string(), site_url),
            (rest_nw.trim_end_matches('/').to_string(), e.url.clone()),
            (rest.trim_end_matches('/').to_string(), e.url.clone()),
            (e.url.trim_end_matches('/').to_string(), e.url.clone()),
        ];
        let mut seen: Vec<String> = Vec::with_capacity(5);
        for (text, url) in candidates {
            let key = text.to_ascii_lowercase();
            if !key.starts_with(&typed) || seen.contains(&key) {
                continue;
            }
            seen.push(key.clone());
            let slot = best.entry(key).or_insert((0, url));
            slot.0 += score;
        }
    }

    best.into_iter()
        .max_by(|(ta, (sa, _)), (tb, (sb, _))| sa.cmp(sb).then(tb.len().cmp(&ta.len())))
        .map(|(text, (_, url))| Completion { text, url })
}
