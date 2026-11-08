# Respawn for Android

Native Android version of Respawn (Kotlin + Android System WebView), built around
playing Total Battle on a phone or tablet.

## Features

- **Account bar.** The top bar shows the current account's name. Tap it (▾) for the list of
  saved accounts and open tabs, and tap an account to load it. **Swipe left/right on the bar**
  (or tap ‹ ›) to switch between loaded tabs.
- **Each tab has its own login**, so every tab can be signed in to a different Total Battle
  account. Use "Name this account…" to save it: named accounts stay in the ▾ list
  and keep their login even after you close their tab.
  (Needs Android System WebView 123 or newer; update it in the Play Store.)
- "＋ Add account" opens a new tab with a fresh login. "Tab options…" covers duplicate,
  keep loaded, desktop site, move and close.
- **Desktop site automatically for Total Battle**, plus a per-tab "Desktop site" toggle.
- **Hibernating tabs:** background tabs are paused but stay loaded. If Android runs low
  on memory, unpinned background tabs are unloaded first; "Keep loaded" tabs are spared.
- **Game crash recovery:** Unity "memory access out of bounds" crashes reload the game
  automatically, and a crashed or killed page reloads without closing the app.
- **Sign-in pop-ups** (Google/Facebook) open in a full-screen window, signed in like the tab.
- **Fullscreen mode** (⋮ menu; Back exits) and **Keep screen on**.
- **Ad & tracker blocker** (same list as the Windows version).
- **History suggestions** in the address bar; tabs are restored when you reopen the app.
- **Gaming sidebar** (chat button, top-left): Discord, Twitch, YouTube, Steam.
- Back on the last page sends Respawn to the background instead of closing it, so games keep running.
- Hardware keyboard: Ctrl+Tab / Ctrl+Shift+Tab switch tabs.

## Build and install (Windows)

1. Install **Android Studio**: <https://developer.android.com/studio>.
2. **File → Open…** and choose `C:\Users\Pat\source\respawn\android`.
   Let Gradle sync finish. The first sync downloads the Android SDK and libraries and
   takes a while. If Android Studio offers to upgrade the Android Gradle Plugin, accept.
3. On your phone, turn on **Developer options → USB debugging**
   (Settings → About phone → tap *Build number* 7 times). Then plug it in.
4. Select your phone at the top of Android Studio and press **Run ▶**.

### Release APK / Play Store bundle

**Build → Generate Signed App Bundle or APK…**, then create a keystore when asked.
**Keep the keystore file and passwords safe**: every future update must be signed
with the same key.

- **APK**: to install directly or share the file.
- **App Bundle (.aab)**: for the Google Play Store.

Before each new release, raise `versionCode` (and `versionName`) in `app/build.gradle.kts`.
