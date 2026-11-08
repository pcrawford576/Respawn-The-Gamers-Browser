//! Discord split view (lists in the sidebar, chat in one tab) and the DeepL
//! translator. The page side lives in discord.js; these commands are the only
//! ones Discord pages may call (see capabilities/discord.json), and each checks
//! that the caller really is a Discord page inside Respawn.

use crate::{add_tab, do_activate_tab, settings, Browser};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, Url, Webview};

pub const SCRIPT: &str = include_str!("discord.js");

pub fn script(mode: &str) -> String {
    SCRIPT.replace("__MODE__", mode)
}

/// DeepL target languages (code, name).
pub const LANGS: &[(&str, &str)] = &[
    ("AR", "Arabic"), ("BG", "Bulgarian"), ("CS", "Czech"), ("DA", "Danish"), ("DE", "German"),
    ("EL", "Greek"), ("EN-GB", "English (UK)"), ("EN-US", "English (US)"), ("ES", "Spanish"),
    ("ES-419", "Spanish (Latin America)"), ("ET", "Estonian"), ("FI", "Finnish"), ("FR", "French"),
    ("HE", "Hebrew"), ("HU", "Hungarian"), ("ID", "Indonesian"), ("IT", "Italian"), ("JA", "Japanese"),
    ("KO", "Korean"), ("LT", "Lithuanian"), ("LV", "Latvian"), ("NB", "Norwegian"), ("NL", "Dutch"),
    ("PL", "Polish"), ("PT-BR", "Portuguese (Brazil)"), ("PT-PT", "Portuguese (Portugal)"), ("RO", "Romanian"),
    ("RU", "Russian"), ("SK", "Slovak"), ("SL", "Slovenian"), ("SV", "Swedish"), ("TH", "Thai"),
    ("TR", "Turkish"), ("UK", "Ukrainian"), ("VI", "Vietnamese"), ("ZH-HANS", "Chinese (simplified)"),
    ("ZH-HANT", "Chinese (traditional)"),
];

fn is_discord(u: &Url) -> bool {
    u.scheme() == "https" && u.host_str().map(|h| h == "discord.com" || h.ends_with(".discord.com")).unwrap_or(false)
}

/// Only Respawn's own Discord tabs / Chat sidebar may use these commands.
fn guard(wv: &Webview, panel_only: bool) -> Result<(), String> {
    let label = wv.label();
    let label_ok = if panel_only { label == "side-discord" } else { label.starts_with("tab-") || label == "side-discord" };
    let url_ok = wv.url().map(|u| is_discord(&u)).unwrap_or(false);
    if label_ok && url_ok {
        Ok(())
    } else {
        Err("not allowed".into())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    auto: bool,
    to: String,
    reply: String,
    has_key: bool,
    split: bool,
    langs: Vec<(String, String)>,
}

#[tauri::command]
pub async fn discord_prefs(app: AppHandle, webview: Webview) -> Result<Prefs, String> {
    guard(&webview, false)?;
    let s = app.state::<Mutex<Browser>>().lock().unwrap().settings.clone();
    Ok(Prefs {
        auto: s.auto_translate,
        to: s.translate_to,
        reply: s.reply_lang,
        has_key: !s.deepl_key.trim().is_empty(),
        split: s.discord_split,
        langs: LANGS.iter().map(|(c, n)| (c.to_string(), n.to_string())).collect(),
    })
}

#[tauri::command]
pub async fn discord_set_prefs(app: AppHandle, webview: Webview, auto: Option<bool>, reply: Option<String>) -> Result<(), String> {
    guard(&webview, false)?;
    let s = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        if let Some(a) = auto {
            b.settings.auto_translate = a;
        }
        if let Some(r) = reply.filter(|r| LANGS.iter().any(|(c, _)| c == r)) {
            b.settings.reply_lang = r;
        }
        b.settings.clone()
    };
    settings::save(&app, &s);
    Ok(())
}

#[derive(Serialize)]
pub struct Translation {
    text: String,
    /// Detected source language, e.g. "DE".
    src: String,
}

/// Translates up to 50 texts with DeepL. The key never leaves Respawn's Rust side.
#[tauri::command]
pub async fn discord_translate(app: AppHandle, webview: Webview, texts: Vec<String>, target: String) -> Result<Vec<Translation>, String> {
    guard(&webview, false)?;
    if !LANGS.iter().any(|(c, _)| *c == target) {
        return Err("unknown language".into());
    }
    let texts: Vec<String> = texts.into_iter().take(50).map(|t| t.chars().take(5000).collect()).collect();
    if texts.is_empty() {
        return Ok(vec![]);
    }
    let key = app.state::<Mutex<Browser>>().lock().unwrap().settings.deepl_key.trim().to_string();
    if key.is_empty() {
        return Err("Add your DeepL key in Respawn Settings".into());
    }
    translate(&key, &texts, &target).await
}

pub async fn translate(key: &str, texts: &[String], target: &str) -> Result<Vec<Translation>, String> {
    // Free keys end in ":fx" and use the free endpoint.
    let host = if key.ends_with(":fx") { "api-free.deepl.com" } else { "api.deepl.com" };
    let resp = reqwest::Client::new()
        .post(format!("https://{host}/v2/translate"))
        .header("Authorization", format!("DeepL-Auth-Key {key}"))
        .json(&serde_json::json!({ "text": texts, "target_lang": target }))
        .send()
        .await
        .map_err(|e| format!("Couldn't reach DeepL: {e}"))?;
    match resp.status().as_u16() {
        200 => {}
        403 => return Err("DeepL didn't accept the key — check it in Settings".into()),
        456 => return Err("DeepL monthly limit reached".into()),
        429 => return Err("DeepL is busy — retrying shortly".into()),
        code => return Err(format!("DeepL error {code}")),
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(v["translations"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|t| Translation {
                    text: t["text"].as_str().unwrap_or_default().to_string(),
                    src: t["detected_source_language"].as_str().unwrap_or_default().to_string(),
                })
                .collect()
        })
        .unwrap_or_default())
}

/// The Chat sidebar switched channel: show it in the chat tab (one tab, reused).
#[tauri::command]
pub async fn discord_navigated(app: AppHandle, webview: Webview, url: String) -> Result<(), String> {
    guard(&webview, true)?;
    let target = Url::parse(&url).map_err(|e| e.to_string())?;
    if !is_discord(&target) {
        return Err("not a Discord address".into());
    }
    show_in_chat_tab(&app, &url);
    Ok(())
}

pub fn show_in_chat_tab(app: &AppHandle, url: &str) {
    // The tab you're on if it's Discord, otherwise the Discord tab used most recently.
    let found = {
        let st = app.state::<Mutex<Browser>>();
        let mut b = st.lock().unwrap();
        let is_d = |u: &str| Url::parse(u).map(|u| is_discord(&u)).unwrap_or(false);
        let active = b.active;
        let pick = b
            .tabs
            .iter()
            .filter(|t| is_d(&t.url))
            .max_by_key(|t| (Some(t.id) == active, t.last_active))
            .map(|t| t.id);
        if let Some(id) = pick {
            // An unloaded tab simply opens on the new channel.
            if let Some(t) = b.tabs.iter_mut().find(|t| t.id == id && t.discarded) {
                t.url = url.to_string();
            }
        }
        pick
    };
    match found {
        Some(id) => {
            do_activate_tab(app, id);
            if let Some(wv) = app.get_webview(&format!("tab-{id}")) {
                let js = format!("window.__respawnGo && window.__respawnGo({})", serde_json::to_string(url).unwrap());
                let _ = wv.eval(&js);
            }
        }
        None => {
            add_tab(app, url, None, false, true, true, 0);
        }
    }
}

/// Tells open Discord pages that settings changed (key added, language, split view).
pub fn refresh_pages(app: &AppHandle) {
    for (label, wv) in app.webviews() {
        if label.starts_with("tab-") || label == "side-discord" {
            let _ = wv.eval("window.__respawnReloadPrefs && window.__respawnReloadPrefs()");
        }
    }
}
