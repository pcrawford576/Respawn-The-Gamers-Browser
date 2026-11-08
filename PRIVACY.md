# Privacy

This program will not transfer any information to other networked systems unless specifically
requested by the user or the person installing or operating it.

Respawn is a web browser, so in practice:

- **Websites you open** receive what any browser sends them (your requests, cookies for that
  site, and so on). Respawn has no account, server or analytics of its own.
- **Searches** typed in the address bar go to the search engine you picked in Settings
  (DuckDuckGo by default), just like visiting that site.
- **Everything Respawn keeps stays on your computer**: settings, history and open tabs in
  `%APPDATA%\gg.respawn.browser`, logins and cookies for each account in
  `%LOCALAPPDATA%\gg.respawn.browser\accounts`, and screenshots in Pictures\Respawn.
  Removing those folders removes the data.
- **DeepL translator (optional):** only if you add your own DeepL API key and use the Discord
  translator, the text being translated is sent to DeepL (api.deepl.com or api-free.deepl.com)
  under your DeepL account. Your key is stored only in your local settings.
- **Ad blocking** happens locally; no lists of visited sites are sent anywhere.
- **Microsoft WebView2**, the page engine built into Windows, may send diagnostic data to
  Microsoft according to your Windows privacy settings. If WebView2 is missing, the installer
  downloads it from Microsoft.
- Respawn does not check for updates or contact any server on its own.
