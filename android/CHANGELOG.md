# Respawn for Android - changes

## 0.1.1
- New **Game resolution** setting (menu > Game resolution): 720p (default, fits inside 720 x 1280), 1080p, or the phone's own screen.
- New **Game performance** setting (menu > Game performance): Balanced (default, 60 FPS), Battery saver (30 FPS) or Sharp (no limits).
  Uses much less memory, battery and heat on phones with sharp or 120 Hz screens.
- Total Battle now fits the phone screen without zooming out (laid out 1280 px wide), and draws at most at your screen's real resolution.
- Fixed: if the phone ran out of memory while a sidebar panel (e.g. Discord) was open, Respawn could close instead of recovering. Sidebar panels are also freed when memory runs low.
- New **Game memory report** (menu > Game memory report): shows what Total Battle is using (game engine, textures, sound, drawing size) and keeps the last reading from before it runs out of memory. Copy button to share it.
- New **Game sound** switch: turning it off skips unpacking the game's sound clips, which can save 100 MB or more.
- Anti-aliasing is turned off for games on Balanced and Battery saver, saving the extra drawing memory it needs.
- New **Turn games sideways** switch (off by default): Total Battle is built for wide screens; upright it draws about 3x more pixels.
- New **Lower-quality game graphics** switch: large game textures are stored at half width and height, using about a quarter of the memory. Small icons and text are untouched.
- New **Experimental: tighter game memory**: the game engine grows its memory in small steps instead of grabbing ~20% extra each time.
- The memory report now subtracts textures the game has deleted, and shows how often the engine grew.
- Lower-quality graphics now also halves textures made without mipmaps (it waits to see whether the game fills them with pixels or draws into them) and 16-bit colour textures.
- The memory report shows why textures weren't halved, and which outside sites the game page loads.
- The blocker now also blocks Google Tag Manager and Google Analytics; the memory report marks which outside sites are blocked.
- Fixed: panning the Total Battle map with a finger jumped back to the last tap. A finger now works as the mouse in games (drag to pan, tap to click) and a two-finger pinch zooms; the page no longer scrolls or zooms underneath. Switch: menu > Touch works as mouse in games.
- Games now pause while Respawn is in the background (menu > Pause games when you leave the app).
- Hidden tabs no longer keep the page renderer at high priority, so Android frees their memory before it has to close Respawn.

## 0.1.0
- First Android version.
