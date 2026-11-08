//! Direct WebView2 tweaks that Tauri doesn't expose.
//!
//! - `configure`: turn on saved passwords + form autofill (off by default in
//!   WebView2), so logins for Google, Facebook, game sites etc. are remembered.
//! - `set_hibernate`: background tabs get MemoryUsageTargetLevel = Low, which
//!   makes WebView2 trim caches and hand memory back to Windows while the page
//!   stays loaded (a running game keeps its state and connection).

use tauri::Webview;

#[cfg(windows)]
pub fn configure(wv: &Webview) {
    let _ = wv.with_webview(|pw| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
        use windows_core::Interface;
        let Ok(core) = pw.controller().CoreWebView2() else { return };
        let Ok(settings) = core.Settings() else { return };
        if let Ok(s4) = settings.cast::<ICoreWebView2Settings4>() {
            let _ = s4.SetIsPasswordAutosaveEnabled(true);
            let _ = s4.SetIsGeneralAutofillEnabled(true);
        }
    });
}

#[cfg(windows)]
pub fn set_hibernate(wv: &Webview, on: bool) {
    let _ = wv.with_webview(move |pw| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        };
        use windows_core::Interface;
        let Ok(core) = pw.controller().CoreWebView2() else { return };
        if let Ok(c19) = core.cast::<ICoreWebView2_19>() {
            let level = if on {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            };
            let _ = c19.SetMemoryUsageTargetLevel(level);
        }
    });
}

#[cfg(not(windows))]
pub fn configure(_wv: &Webview) {}

#[cfg(not(windows))]
pub fn set_hibernate(_wv: &Webview, _on: bool) {}
