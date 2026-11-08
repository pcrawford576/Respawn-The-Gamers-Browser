# Respawn — The Gamers Browser

A lightweight web browser built for gamers.

Built with Tauri 2 on Windows' built-in WebView2 engine, so the app itself stays a
few MB instead of shipping a whole copy of Chromium.

## Download

Get the latest Windows installer from the [Releases](../../releases) page. Running a newer
installer over an existing install upgrades Respawn and keeps your settings, accounts and tabs.

## Features

| Feature | How it works |
|---|---|
| **Resource limiter** | Status bar shows live RAM for all browser processes. Over your RAM cap, the least-recently-used background tab is put to sleep (webview closed, URL kept, wakes on click). Idle tabs sleep after N minutes. Optional renderer-process limit. |
| **Game Mode** (`Ctrl+Shift+G`) | Sleeps every background tab, switches to a tighter RAM cap, and drops all browser processes to *Below Normal* CPU priority so your game wins the scheduler. Works while the game has focus. |
| **Gaming sidebar** | Left rail opens Discord, Twitch, YouTube or Steam in a side panel beside the page. Panels stay logged in and keep running when hidden. |
| **Ad & tracker blocker** | About 170 ad/tracker domains (plus your own) are blackholed at DNS level through Chromium's `--host-resolver-rules`, so blocking costs nothing per request. A cosmetic stylesheet hides leftover ad boxes, and popunder redirects to blocked hosts are cancelled. |

Also: new-tab page with gaming quick-launch tiles, `target=_blank` links open as tabs, settings saved to `%APPDATA%\gg.respawn.browser\settings.json`.

## Build on Windows

1. Install prerequisites (one time):
   - **Microsoft C++ Build Tools** — <https://visualstudio.microsoft.com/visual-cpp-build-tools/>, pick "Desktop development with C++".
   - **Rust** — <https://rustup.rs>
   - **WebView2** is already on Windows 10/11.
   - Tauri CLI: `cargo install tauri-cli --version "^2" --locked`
2. Run in dev mode:
   ```powershell
   cd respawn\src-tauri
   cargo tauri dev
   ```
3. Build an installer (raises the version number automatically):
   ```powershell
   cd respawn
   powershell -ExecutionPolicy Bypass -File .\release.ps1
   ```
   - Each build goes up one step (0.1.0 → 0.1.1 → 0.1.2…). Use `-Bump minor` (0.2.0) or `-Bump major` (1.0.0) for bigger releases.
   - Release notes come from the **Unreleased** section of `ui\changelog.md` (one `- ` line per change).
     If it's empty, the script asks you what changed. After an update, Respawn shows these notes once ("What's new").
   - The installer lands in `src-tauri\target\release\bundle\nsis\`. Running it on a PC that already has
     Respawn upgrades it in place and keeps settings, accounts, logins and tabs.

## Project layout

```
respawn/
├─ ui/                     browser chrome (plain HTML/CSS/JS, no build step)
│  ├─ index.html, style.css, app.js
│  └─ newtab.html          new-tab page
└─ src-tauri/
   ├─ src/main.rs          tabs, layout, sidebar, commands, hotkeys
   ├─ src/perf.rs          RAM monitor, tab sleeping, Game Mode, CPU priority
   ├─ src/adblock.rs       blocklist -> WebView2 args, cosmetic filter
   ├─ src/blocklist.txt    built-in blocked domains (edit freely)
   └─ src/settings.rs      persisted settings
```

## How it fits together

- One native window. The `ui` webview draws the chrome and fills the window; each tab
  and sidebar panel is its own child WebView2 placed on top of the content area.
  Inactive tabs are hidden, which lets Chromium throttle them.
- Websites get **no** access to Tauri IPC. Commands also check that the caller is the
  `ui` webview. Page scripts "talk" to Rust only by navigating to the fake host
  `respawn.invalid`, which Rust intercepts and cancels (used for "open in new tab"
  and new-tab-page search).
- WebView2 needs every webview in a profile to use identical launch arguments, so
  ad-block and renderer-limit settings apply on restart.

## Known limitations / next steps

- YouTube and Twitch serve video ads from their own domains, so DNS-level blocking
  won't stop those. Adding a real filter engine (for example the `adblock` crate on
  WebView2's `WebResourceRequested`) is the next upgrade.
- No bookmarks, history page or downloads UI yet. Downloads use WebView2's default handling.
- Hotkeys may clash with a game's bindings. Change them in Settings → Keyboard shortcuts.
- Sizing constants in `main.rs` (`TOP`, `RAIL`, `PANEL`, `STATUS`) must match `ui/style.css`.

## Publishing a release

Releases are built and published by GitHub Actions (`.github/workflows/release.yml`), so
every installer can be traced back to the code in this repository.

```powershell
cd respawn
powershell -ExecutionPolicy Bypass -File .\release.ps1 -Publish
```

This raises the version, turns the **Unreleased** notes into that version's notes, test-builds
locally, then commits, tags (`v0.1.3`, …) and pushes. GitHub builds the installer, has it
signed, and posts it on the Releases page with the notes. Setup steps are in
[docs/signing-setup.md](docs/signing-setup.md).

## Code signing

Free code signing provided by [SignPath.io](https://signpath.io), certificate by
[SignPath Foundation](https://signpath.org). See the [code signing policy](CODE_SIGNING_POLICY.md).

## Privacy

Respawn has no servers, accounts or analytics of its own. See [PRIVACY.md](PRIVACY.md).

## License

[MIT](LICENSE)
