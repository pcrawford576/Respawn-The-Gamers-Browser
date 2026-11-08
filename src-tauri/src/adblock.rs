//! Ad & tracker blocking.
//!
//! Two layers:
//! 1. Network: every blocked domain is mapped to ~NOTFOUND through Chromium's
//!    `--host-resolver-rules` switch, which WebView2 accepts as an additional
//!    browser argument. Requests to those hosts fail instantly, in every frame,
//!    for every resource type, with zero per-request overhead.
//! 2. Cosmetic: a small stylesheet hides the empty boxes ads leave behind.
//!
//! Top-level navigations (popunders, redirect chains) are also checked in Rust.

use crate::settings::Settings;
use std::collections::HashSet;
use std::sync::OnceLock;
use tauri::Url;

static BLOCKED: OnceLock<HashSet<String>> = OnceLock::new();
static ENABLED: OnceLock<bool> = OnceLock::new();

const BUILTIN: &str = include_str!("blocklist.txt");
/// Windows command lines max out at 32k chars; stay well under.
const MAX_RULES_LEN: usize = 26_000;

/// Base switches Tauri itself passes to WebView2 (kept when we add our own).
const BASE_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

/// Builds the browser-argument string used for EVERY webview. WebView2 requires
/// all webviews sharing a profile to use identical arguments, so this is computed
/// once at launch; changes made in Settings apply on the next start.
pub fn init(s: &Settings) -> String {
    let mut set = HashSet::new();
    if s.adblock {
        for line in BUILTIN.lines().map(str::trim).chain(s.extra_blocked.iter().map(|x| x.trim())) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let d = line
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_start_matches("*.")
                .trim_end_matches('/')
                .to_ascii_lowercase();
            if d.contains('.') && !d.contains(' ') {
                set.insert(d);
            }
        }
    }

    let mut args = String::from(BASE_ARGS);
    if s.renderer_limit > 0 {
        args.push_str(&format!(" --renderer-process-limit={}", s.renderer_limit));
    }

    if !set.is_empty() {
        let mut domains: Vec<&String> = set.iter().collect();
        domains.sort();
        let mut rules = String::new();
        for d in domains {
            let r = format!("MAP {d} ~NOTFOUND,MAP *.{d} ~NOTFOUND");
            if rules.len() + r.len() + 1 > MAX_RULES_LEN {
                break;
            }
            if !rules.is_empty() {
                rules.push(',');
            }
            rules.push_str(&r);
        }
        args.push_str(&format!(" --host-resolver-rules=\"{rules}\""));
    }

    let _ = ENABLED.set(s.adblock);
    let _ = BLOCKED.set(set);
    args
}

pub fn enabled() -> bool {
    *ENABLED.get().unwrap_or(&false)
}

pub fn blocked_count() -> usize {
    BLOCKED.get().map(|s| s.len()).unwrap_or(0)
}

/// True if the host or any parent domain is on the list.
pub fn is_blocked_host(host: &str) -> bool {
    let Some(set) = BLOCKED.get() else { return false };
    let mut h = host.trim_end_matches('.').to_ascii_lowercase();
    loop {
        if set.contains(&h) {
            return true;
        }
        match h.find('.') {
            Some(i) => h = h[i + 1..].to_string(),
            None => return false,
        }
    }
}

pub fn is_blocked_url(u: &Url) -> bool {
    enabled() && u.host_str().map(is_blocked_host).unwrap_or(false)
}

/// Injected at document start in every tab when ad blocking is on.
pub const COSMETIC_JS: &str = r#"
(() => {
  const css = `
    ins.adsbygoogle, .adsbygoogle, [id^="google_ads_"], [id^="div-gpt-ad"],
    [data-ad-slot], [data-google-query-id], iframe[src*="doubleclick.net"],
    iframe[src*="googlesyndication"], .trc_rbox_container, [id^="taboola-"],
    .OUTBRAIN, .ob-widget, [class*="AdSlot"], .ad-banner, .advertisement,
    #ad-leaderboard, .leaderboard-ad, .sticky-ad, .adthrive-ad, .mv-ad-box,
    [id^="nitropay"], [id^="venatus"], .pw-tag, .ezoic-ad
    { display: none !important; }`;
  const add = () => {
    const s = document.createElement('style');
    s.id = '__respawn_cosmetic';
    s.textContent = css;
    (document.head || document.documentElement).appendChild(s);
  };
  if (document.documentElement) add();
  else document.addEventListener('DOMContentLoaded', add, { once: true });
})();
"#;
