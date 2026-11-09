package gg.respawn.browser

import android.annotation.SuppressLint
import android.app.ActivityManager
import android.app.Dialog
import android.content.ClipData
import android.content.ClipboardManager
import android.app.DownloadManager
import android.content.ComponentCallbacks2
import android.content.Intent
import android.content.pm.ActivityInfo
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Bundle
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.os.Message
import android.text.TextUtils
import android.util.TypedValue
import android.view.GestureDetector
import android.view.Gravity
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.webkit.CookieManager
import android.webkit.JsResult
import android.webkit.RenderProcessGoneDetail
import android.webkit.URLUtil
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.ArrayAdapter
import android.widget.AutoCompleteTextView
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.PopupMenu
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.Toast
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.drawerlayout.widget.DrawerLayout
import androidx.webkit.ProfileStore
import androidx.webkit.ScriptHandler
import androidx.webkit.WebSettingsCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import kotlin.math.max
import org.json.JSONObject
import org.json.JSONTokener

class MainActivity : AppCompatActivity() {

    private lateinit var store: Store
    private lateinit var history: History

    private lateinit var drawer: DrawerLayout
    private lateinit var mainCol: LinearLayout
    private lateinit var tabBar: View
    private lateinit var toolbar: View
    private lateinit var accountName: TextView
    private lateinit var tabPosition: TextView
    private lateinit var accounts: MutableList<SavedAccount>
    private lateinit var address: AutoCompleteTextView
    private lateinit var progress: ProgressBar
    private lateinit var content: FrameLayout
    private lateinit var panelContainer: FrameLayout

    private val tabs = mutableListOf<Tab>()
    private var active: Tab? = null
    private var fullscreen = false
    private var customView: View? = null
    private var customViewCallback: WebChromeClient.CustomViewCallback? = null

    private val handler = Handler(Looper.getMainLooper())
    private val saveRunnable = Runnable {
        store.saveSession(tabs, active)
        syncAccountUrls()
        history.flush()
    }

    /** Separate logins per tab need Android System WebView 123+. */
    private val multiProfile by lazy { WebViewFeature.isFeatureSupported(WebViewFeature.MULTI_PROFILE) }
    private val docStartScripts by lazy { WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT) }
    /** The Performance script installed in each WebView, so it can be swapped when the setting changes. */
    private val perfScripts = mutableMapOf<WebView, ScriptHandler>()
    private val gameScripts = mutableMapOf<WebView, ScriptHandler>()
    /** Latest Game memory report reading, its peaks, and whether the game has since run out of memory. */
    private var memReport: JSONObject? = null
    private val memPeak = HashMap<String, Int>()
    private var memCrashed = false
    private val memPoll = object : Runnable {
        override fun run() {
            val t = active
            val wv = t?.webView
            if (t != null && wv != null && isTotalBattle(Uri.parse(t.url).host)) {
                wv.evaluateJavascript(MEM_READ_JS) { r -> onMemReading(r) }
            }
            handler.postDelayed(this, 3000)
        }
    }
    private var timersPaused = false
    private lateinit var desktopUa: String
    private lateinit var mobileUa: String

    private val panels = listOf(
        "Chat" to "https://discord.com/app",
        "Live" to "https://m.twitch.tv/",
        "Video" to "https://m.youtube.com/",
        "Store" to "https://store.steampowered.com/",
    )
    private val panelViews = HashMap<String, WebView>()
    private var lastRendererLoss = 0L

    // ------------------------------------------------------------------ setup

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Debug builds: inspect pages from your PC at chrome://inspect.
        if (applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE != 0) {
            WebView.setWebContentsDebuggingEnabled(true)
        }
        WindowCompat.setDecorFitsSystemWindows(window, false)
        setContentView(R.layout.activity_main)

        store = Store(this)
        history = History(this)
        AdBlock.init(this)

        drawer = findViewById(R.id.drawer)
        mainCol = findViewById(R.id.main)
        tabBar = findViewById(R.id.tabBar)
        toolbar = findViewById(R.id.toolbar)
        accountName = findViewById(R.id.accountName)
        tabPosition = findViewById(R.id.tabPosition)
        accounts = store.loadAccounts()
        address = findViewById(R.id.address)
        progress = findViewById(R.id.progress)
        content = findViewById(R.id.content)
        panelContainer = findViewById(R.id.panelContainer)

        applyInsets()
        setupUserAgents()
        setupToolbar()
        setupAccountBar()
        setupSidebar()
        setupBack()
        applyKeepScreenOn()

        // Reopen last session. The active tab and "Keep loaded" tabs load now,
        // the rest load when tapped (keeps startup light).
        val (saved, activeIndex) = store.loadSession()
        cleanupProfiles(saved.map { it.profile }.toSet() + accounts.map { it.profile })
        if (saved.isEmpty()) {
            addTab(NEWTAB_URL, profile = newTabProfile(), activate = true)
        } else {
            saved.forEach { t ->
                if (!multiProfile) t.profile = DEFAULT_PROFILE
                tabs += t
                if (t.pinned && !store.oneAtATime) createWebView(t).loadUrl(t.url)
            }
            activate(saved[activeIndex.coerceIn(0, saved.size - 1)])
        }
        intent?.data?.let { openExternalLink(it) }

        if (!multiProfile && store.isolateTabs) {
            toast("Update \"Android System WebView\" in the Play Store to give each tab its own login")
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        intent.data?.let { openExternalLink(it) }
    }

    private fun openExternalLink(uri: Uri) {
        if (uri.scheme == "http" || uri.scheme == "https") addTab(uri.toString(), profile = DEFAULT_PROFILE, activate = true)
    }

    /** Edge-to-edge: pad the UI so it stays clear of the status bar, nav bar and keyboard. */
    private fun applyInsets() {
        val apply = { v: View, insets: WindowInsetsCompat ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            v.setPadding(bars.left, bars.top, bars.right, max(bars.bottom, ime.bottom))
            insets
        }
        ViewCompat.setOnApplyWindowInsetsListener(mainCol, apply)
        ViewCompat.setOnApplyWindowInsetsListener(findViewById(R.id.sidebar), apply)
    }

    private fun setupUserAgents() {
        val def = WebSettings.getDefaultUserAgent(this)
        // "; wv" marks an in-app WebView; some sign-in pages (Google) refuse those.
        mobileUa = def.replace("; wv", "")
        val chrome = Regex("Chrome/([\\d.]+)").find(def)?.groupValues?.get(1) ?: "130.0.0.0"
        desktopUa = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/$chrome Safari/537.36"
    }

    /** Deletes the login storage of tabs that were closed. */
    private fun cleanupProfiles(inUse: Set<String>) {
        if (!multiProfile) return
        runCatching {
            val ps = ProfileStore.getInstance()
            ps.allProfileNames.filter { it.startsWith("tab-") && it !in inUse }.forEach { runCatching { ps.deleteProfile(it) } }
        }
    }

    private fun newTabProfile(): String =
        if (multiProfile && store.isolateTabs) store.nextProfileName() else DEFAULT_PROFILE

    // --------------------------------------------------------------- webviews

    private fun cookieManager(profile: String): CookieManager =
        if (multiProfile) ProfileStore.getInstance().getOrCreateProfile(profile).cookieManager
        else CookieManager.getInstance()

    @SuppressLint("SetJavaScriptEnabled")
    private fun configure(wv: WebView, profile: String, desktop: Boolean) {
        // The profile must be set before anything else touches the WebView.
        if (multiProfile) {
            ProfileStore.getInstance().getOrCreateProfile(profile)
            WebViewCompat.setProfile(wv, profile)
        }
        with(wv.settings) {
            javaScriptEnabled = true
            domStorageEnabled = true
            mediaPlaybackRequiresUserGesture = false
            setSupportMultipleWindows(true)
            javaScriptCanOpenWindowsAutomatically = true
            loadWithOverviewMode = true
            useWideViewPort = true
            builtInZoomControls = true
            displayZoomControls = false
            mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
            userAgentString = if (desktop) desktopUa else mobileUa
        }
        if (WebViewFeature.isFeatureSupported(WebViewFeature.REQUESTED_WITH_HEADER_ALLOW_LIST)) {
            WebSettingsCompat.setRequestedWithHeaderOriginAllowList(wv.settings, emptySet())
        }
        cookieManager(profile).setAcceptThirdPartyCookies(wv, true)
        wv.setBackgroundColor(ContextCompat.getColor(this, R.color.bg))
    }

    private fun createWebView(tab: Tab): WebView {
        val wv = WebView(this)
        configure(wv, tab.profile, tab.desktop)
        wv.webViewClient = TabClient(tab)
        wv.webChromeClient = TabChrome(tab)
        wv.setDownloadListener { url, ua, disposition, mime, _ -> download(url, ua, disposition, mime, tab.profile) }
        // Hidden tabs don't keep the page renderer's priority up, so Android can
        // reclaim it before it has to close Respawn.
        wv.setRendererPriorityPolicy(WebView.RENDERER_PRIORITY_IMPORTANT, true)
        installPerfScript(wv)
        // A tab created after timers were paused in the background would start frozen.
        if (timersPaused) { wv.resumeTimers(); timersPaused = false }
        wv.visibility = View.GONE
        content.addView(wv, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        tab.webView = wv
        return wv
    }

    private fun destroyWebView(tab: Tab) {
        tab.webView?.let { wv ->
            perfScripts.remove(wv)?.let { runCatching { it.remove() } }
            gameScripts.remove(wv)?.let { runCatching { it.remove() } }
            content.removeView(wv)
            wv.stopLoading()
            wv.destroy()
        }
        tab.webView = null
    }

    // ------------------------------------------------------------ performance

    private fun perfScript(): String {
        val fps = if (store.performance == 2) "30" else "60"   // Battery saver / Balanced (Sharp installs no script)
        val (long, short) = when (store.gameResolution) {
            1 -> "1920" to "1080"
            2 -> "0" to "0"          // phone screen
            else -> "1280" to "720"  // 720p
        }
        return PERF_JS.replace("__LONG__", long).replace("__SHORT__", short)
            .replace("__FPS__", fps).replace("__WIDTH__", "1280")
    }

    @SuppressLint("RequiresFeature")
    private fun installPerfScript(wv: WebView) {
        if (!docStartScripts) return
        perfScripts.remove(wv)?.let { runCatching { it.remove() } }
        gameScripts.remove(wv)?.let { runCatching { it.remove() } }
        val game = GAME_JS.replace("__LITEGL__", if (store.performance != 0) "true" else "false")
            .replace("__MUTE__", if (store.gameSound) "false" else "true")
            .replace("__TRIM__", if (store.trimHeap) "true" else "false")
            .replace("__LOWTEX__", if (store.lowTextures) "true" else "false")
            .replace("__TOUCHMOUSE__", if (store.touchMouse) "true" else "false")
        runCatching { gameScripts[wv] = WebViewCompat.addDocumentStartJavaScript(wv, game, setOf("*")) }
        if (store.performance == 0) return
        runCatching { perfScripts[wv] = WebViewCompat.addDocumentStartJavaScript(wv, perfScript(), setOf("*")) }
    }

    private fun setPerformance(level: Int) {
        if (level == store.performance) return
        store.performance = level
        reinstallGameScripts()
    }

    private fun setGameSound(on: Boolean) {
        store.gameSound = on
        reinstallGameScripts()
    }

    /** Applies changed game settings to every tab, then offers to reload the game. */
    private fun reinstallGameScripts() {
        tabs.forEach { t -> t.webView?.let { installPerfScript(it) } }
        if (!docStartScripts) {
            toast("Update \"Android System WebView\" in the Play Store to use this setting")
            return
        }
        val tab = active
        val onGame = tab?.webView != null && isTotalBattle(Uri.parse(tab.url).host)
        if (!onGame) { toast("Applies the next time a game loads"); return }
        AlertDialog.Builder(this)
            .setTitle("Reload the game?")
            .setMessage("The new setting applies when the game reloads.")
            .setPositiveButton("Reload now") { _, _ -> tab?.webView?.reload() }
            .setNegativeButton("Later", null)
            .show()
    }

    /** Game tabs turn sideways (games are built for wide screens; upright they draw ~3x more). */
    private fun applyOrientation() {
        val t = active
        val game = store.gameLandscape && t != null && isTotalBattle(Uri.parse(t.url).host)
        val want = if (game) ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE else ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        if (requestedOrientation != want) requestedOrientation = want
    }

    private fun onMemReading(raw: String?) {
        val json = runCatching { JSONTokener(raw ?: "").nextValue() as? String }.getOrNull()
        if (json.isNullOrEmpty()) return
        val r = runCatching { JSONObject(json) }.getOrNull() ?: return
        memReport = r
        memCrashed = false
        for (k in listOf("wasmMB", "texMB", "texCompressedMB", "audioMB", "jsMB")) {
            val v = r.optInt(k, -1)
            if (v >= 0) memPeak[k] = max(memPeak[k] ?: 0, v)
        }
    }

    private fun showMemReport() {
        val am = getSystemService(ACTIVITY_SERVICE) as ActivityManager
        val mi = ActivityManager.MemoryInfo().also { am.getMemoryInfo(it) }
        val sb = StringBuilder()
        sb.append("Phone memory: ${mi.totalMem / 1_048_576} MB, ${mi.availMem / 1_048_576} MB free right now\n")
        sb.append("Settings: ${listOf("Sharp", "Balanced", "Battery saver")[store.performance]}, " +
            "${listOf("720p", "1080p", "phone screen")[store.gameResolution]}, game sound ${if (store.gameSound) "on" else "off"}, " +
            "sideways ${if (store.gameLandscape) "on" else "off"}, tighter memory ${if (store.trimHeap) "on" else "off"}, " +
            "lower-quality graphics ${if (store.lowTextures) "on" else "off"}, touch as mouse ${if (store.touchMouse) "on" else "off"}\n\n")
        val r = memReport
        if (r == null) {
            sb.append("No reading yet. Open Total Battle, wait until it loads (or runs out of memory), then open this again.")
        } else {
            if (memCrashed) sb.append("Last reading before the game ran out of memory:\n")
            fun line(label: String, key: String) {
                val v = r.optInt(key, -1)
                if (v >= 0) sb.append("$label: $v MB (highest ${memPeak[key] ?: v} MB)\n")
            }
            line("Game engine", "wasmMB")
            line("Textures, uncompressed (approx.)", "texMB")
            line("Textures, compressed", "texCompressedMB")
            line("Unpacked sound", "audioMB")
            line("JavaScript", "jsMB")
            sb.append("Textures loaded: ${r.optInt("textures")}, sound clips: ${r.optInt("audioClips")}\n")
            sb.append("Engine memory grew ${r.optInt("grows")} times (${r.optInt("trimmed")} big steps refused)\n")
            if (r.optString("wasmAsk").isNotEmpty()) sb.append("Game engine asked for: ${r.optString("wasmAsk")}\n")
            if (r.optBoolean("lowtex")) {
                sb.append("Lower-quality graphics: ${r.optInt("shrunk")} textures halved, about ${r.optInt("savedMB")} MB saved\n")
                sb.append("Not halved: ${r.optInt("skipSmallMB")} MB small, ${r.optInt("skipTargetMB")} MB drawing targets, " +
                    "${r.optInt("skipFormatMB")} MB other formats${r.optString("skipFormats").let { if (it.isEmpty()) "" else " ($it)" }}\n")
                if (r.optInt("rtShrunk") > 0) sb.append("Warning: ${r.optInt("rtShrunk")} halved textures are drawn into; if you see glitches, turn lower-quality graphics off\n")
            }
            sb.append("Drawing size: ${r.optString("canvas").ifEmpty { "unknown" }}, anti-aliasing ${if (r.optBoolean("aa")) "on" else "off"}\n")
            sb.append("Texture types this phone supports: ${r.optString("formats").ifEmpty { "unknown" }}\n")
            if (r.optString("hosts").isNotEmpty()) {
                // "host (n), host (n)": mark the ones the blocker stops.
                val marked = r.optString("hosts").split(", ").joinToString(", ") { item ->
                    val host = item.substringBefore(" (")
                    if (store.adblock && AdBlock.isBlocked(host)) "$item blocked" else item
                }
                sb.append("Outside sites the game page asks for: $marked\n")
            }
        }
        val text = sb.toString()
        AlertDialog.Builder(this)
            .setTitle("Game memory report")
            .setMessage(text)
            .setPositiveButton("Copy") { _, _ ->
                (getSystemService(CLIPBOARD_SERVICE) as ClipboardManager)
                    .setPrimaryClip(ClipData.newPlainText("Respawn memory report", text))
                toast("Copied")
            }
            .setNegativeButton("Close", null)
            .show()
    }

    private inner class TabClient(val tab: Tab) : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            val uri = request.url
            if (uri.host == BRIDGE_HOST) {
                // Search box on the new-tab page.
                if (uri.path == "/search") view.loadUrl(resolveInput(uri.getQueryParameter("q") ?: ""))
                return true
            }
            when (uri.scheme) {
                "http", "https", "file", "about", "data", "blob", "javascript" -> {}
                else -> { openOutside(uri); return true }
            }
            if (store.adblock && AdBlock.isBlocked(uri.host)) return true
            // Total Battle needs the desktop site: switch automatically.
            if (request.isForMainFrame && isTotalBattle(uri.host) && !tab.desktop) {
                tab.desktop = true
                view.settings.userAgentString = desktopUa
                view.loadUrl(uri.toString())
                return true
            }
            return false
        }

        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
            if (store.adblock && AdBlock.isBlocked(request.url.host)) AdBlock.emptyResponse() else null

        override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
            tab.url = url
            if (isTotalBattle(Uri.parse(url).host) && !tab.desktop) {
                tab.desktop = true
                view.settings.userAgentString = desktopUa
                view.post { view.reload() }
            }
            if (tab === active) updateAddress()
        }

        override fun onPageFinished(view: WebView, url: String) {
            tab.url = url
            // Zoom the game out to fit the screen (smallest zoom = page width).
            if (store.performance != 0 && isTotalBattle(Uri.parse(url).host)) {
                view.postDelayed({ runCatching { view.zoomBy(0.01f) } }, 1500)
            }
            view.title?.let { if (it.isNotBlank() && !it.startsWith("http")) tab.title = it }
            if (store.saveHistory) history.record(url)
            renderTabs()
            if (tab === active) updateAddress()
            saveSoon()
        }

        // Catches single-page-app navigation (no full page load).
        override fun doUpdateVisitedHistory(view: WebView, url: String, isReload: Boolean) {
            tab.url = url
            if (tab === active) { updateAddress(); applyOrientation() }
        }

        // The page's renderer crashed or Android killed it to free memory.
        // Without handling this the whole app would close.
        override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
            val wasActive = tab === active
            if (wasActive && memReport != null) memCrashed = true
            destroyWebView(tab)
            if (wasActive) {
                // Every other tab and sidebar panel shares the same page process, so free them all.
                tabs.filter { it !== tab }.forEach { destroyWebView(it) }
                destroyPanels()
                val now = System.currentTimeMillis()
                if (now - lastRendererLoss > 60_000) {
                    lastRendererLoss = now
                    toast(if (detail.didCrash()) "The page crashed — reloading" else "Ran out of memory — closed other tabs and reloading")
                    createWebView(tab).loadUrl(tab.url)
                    showActive()
                } else {
                    // Second time within a minute: stop instead of looping.
                    toast("Not enough memory for this game. See ⋮ → Game memory report.")
                }
            }
            renderTabs()
            return true
        }
    }

    private inner class TabChrome(val tab: Tab) : WebChromeClient() {
        override fun onReceivedTitle(view: WebView, title: String?) {
            if (!title.isNullOrBlank() && !title.startsWith("http") && !title.startsWith("file:")) {
                tab.title = title
                renderTabs()
            }
        }

        override fun onProgressChanged(view: WebView, newProgress: Int) {
            tab.progress = newProgress
            if (tab === active) updateProgress()
        }

        /** Unity games report crashes with alert(): reload instead (max 3 per 2 min). */
        override fun onJsAlert(view: WebView, url: String?, message: String?, result: JsResult): Boolean {
            if (message?.contains("error occurred running the Unity content") == true) {
                val now = System.currentTimeMillis()
                while (tab.crashReloads.isNotEmpty() && now - tab.crashReloads.first() > 120_000) tab.crashReloads.removeFirst()
                if (tab.crashReloads.size < 3) {
                    tab.crashReloads.addLast(now)
                    result.confirm()
                    toast("Game crashed — reloading")
                    view.reload()
                    return true
                }
            }
            return false
        }

        /** Sign-in pop-ups (Google/Facebook) and target=_blank links. */
        override fun onCreateWindow(view: WebView, isDialog: Boolean, isUserGesture: Boolean, resultMsg: Message): Boolean {
            openPopup(tab, resultMsg)
            return true
        }

        // Full-screen video.
        override fun onShowCustomView(view: View, callback: CustomViewCallback) {
            customView?.let { onHideCustomView() }
            customView = view
            customViewCallback = callback
            (window.decorView as FrameLayout).addView(view, FrameLayout.LayoutParams(-1, -1))
            setSystemBarsHidden(true)
        }

        override fun onHideCustomView() {
            customView?.let { (window.decorView as FrameLayout).removeView(it) }
            customView = null
            customViewCallback?.onCustomViewHidden()
            customViewCallback = null
            if (!fullscreen) setSystemBarsHidden(false)
        }
    }

    /** Shows a pop-up window in a full-screen dialog, signed in like the tab that opened it. */
    @SuppressLint("SetJavaScriptEnabled")
    private fun openPopup(tab: Tab, resultMsg: Message) {
        val popup = WebView(this)
        // Sign-in pages (Google/Facebook) reject browsers whose identity doesn't add up,
        // so pop-ups always present as a normal phone browser, never the desktop mode.
        configure(popup, tab.profile, desktop = false)
        popup.settings.setSupportMultipleWindows(false)

        val dialog = Dialog(this, android.R.style.Theme_DeviceDefault_NoActionBar)
        val title = TextView(this).apply {
            setTextColor(ContextCompat.getColor(context, R.color.text))
            textSize = 14f
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
        }
        val openAsTab = TextView(this).apply {
            text = "Open as tab"
            setTextColor(ContextCompat.getColor(context, R.color.accent))
            setPadding(dp(12), dp(8), dp(12), dp(8))
            setOnClickListener {
                popup.url?.takeIf { it.startsWith("http") }?.let { url ->
                    addTab(url, profile = tab.profile, activate = true, after = tab, desktop = tab.desktop)
                }
                dialog.dismiss()
            }
        }
        val close = ImageButton(this).apply {
            setImageResource(R.drawable.ic_close)
            setBackgroundColor(Color.TRANSPARENT)
            setOnClickListener { dialog.dismiss() }
        }
        val header = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(ContextCompat.getColor(context, R.color.panel))
            setPadding(dp(8), 0, 0, 0)
            addView(close, LinearLayout.LayoutParams(dp(44), dp(44)))
            addView(title, LinearLayout.LayoutParams(0, -2, 1f))
            addView(openAsTab)
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(ContextCompat.getColor(context, R.color.bg))
            addView(header, LinearLayout.LayoutParams(-1, dp(48)))
            addView(popup, LinearLayout.LayoutParams(-1, 0, 1f))
        }
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.ime())
            v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            insets
        }
        popup.webViewClient = object : WebViewClient() {
            override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
                title.text = Uri.parse(url).host ?: url
            }
            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
                if (store.adblock && AdBlock.isBlocked(request.url.host)) AdBlock.emptyResponse() else null
        }
        popup.webChromeClient = object : WebChromeClient() {
            override fun onCloseWindow(window: WebView) = dialog.dismiss()
        }
        dialog.setContentView(root)
        dialog.setOnDismissListener { popup.destroy() }
        dialog.show()

        (resultMsg.obj as WebView.WebViewTransport).webView = popup
        resultMsg.sendToTarget()
    }

    // ------------------------------------------------------------------- tabs

    private fun addTab(
        url: String,
        profile: String,
        activate: Boolean,
        after: Tab? = null,
        desktop: Boolean = isTotalBattle(Uri.parse(url).host),
    ): Tab {
        val tab = Tab(id = store.nextTabId(), url = url, profile = profile, desktop = desktop)
        val at = after?.let { tabs.indexOf(it) + 1 } ?: tabs.size
        tabs.add(at.coerceIn(0, tabs.size), tab)
        if (activate) {
            activate(tab) // frees other tabs first (if set), then loads this one
        } else {
            createWebView(tab).loadUrl(url)
            renderTabs()
        }
        saveSoon()
        return tab
    }

    private fun activate(tab: Tab) {
        val prev = active
        prev?.lastActive = System.currentTimeMillis()
        active = tab
        tab.lastActive = System.currentTimeMillis()
        // A browser game can need 1–2 GB, so on phones keep only this tab in
        // memory (plus "Keep loaded" tabs). Free the others BEFORE loading.
        if (store.oneAtATime) {
            tabs.filter { it !== tab && !it.pinned && it.webView != null }.forEach { destroyWebView(it) }
        }
        if (tab.webView == null) createWebView(tab).loadUrl(tab.url) // was unloaded
        showActive()
        applyOrientation()
        renderTabs()
        updateAddress()
        updateProgress()
        saveSoon()
    }

    /**
     * Only the active tab is visible. Background tabs are paused (no drawing,
     * fewer resources) but stay loaded, so games keep their state.
     */
    private fun showActive() {
        for (t in tabs) {
            val wv = t.webView ?: continue
            if (t === active) {
                wv.visibility = View.VISIBLE
                wv.onResume()
            } else {
                wv.visibility = View.GONE
                wv.onPause()
            }
        }
    }

    private fun closeTab(tab: Tab) {
        val i = tabs.indexOf(tab)
        if (i < 0) return
        destroyWebView(tab)
        tabs.removeAt(i)
        if (tabs.isEmpty()) {
            active = null
            addTab(NEWTAB_URL, profile = newTabProfile(), activate = true)
            return
        }
        if (tab === active) activate(tabs[i.coerceAtMost(tabs.size - 1)]) else renderTabs()
        saveSoon()
    }

    /** Frees a background tab's memory; it reloads when tapped. */
    private fun unload(tab: Tab) {
        if (tab === active || tab.pinned || tab.webView == null) return
        destroyWebView(tab)
        renderTabs()
    }

    private fun cycleTab(step: Int) {
        if (tabs.size < 2) return
        val i = tabs.indexOf(active).coerceAtLeast(0)
        activate(tabs[Math.floorMod(i + step, tabs.size)])
    }

    private fun duplicate(tab: Tab) {
        // A duplicate gets its own login, so you can sign in to another account.
        val copy = addTab(tab.url, profile = newTabProfile(), activate = true, after = tab, desktop = tab.desktop)
        copy.title = tab.title
        renderTabs()
    }

    private fun freshLogin(tab: Tab) {
        if (!multiProfile) {
            toast("Needs a newer Android System WebView (update it in the Play Store)")
            return
        }
        destroyWebView(tab)
        tab.profile = store.nextProfileName()
        tab.name = null // new login, so the old account name no longer applies
        createWebView(tab).loadUrl(tab.url)
        if (tab === active) showActive()
        toast("This tab now has its own empty login — sign in with the account you want")
        renderTabs()
        saveSoon()
    }

    private fun setDesktop(tab: Tab, on: Boolean) {
        tab.desktop = on
        tab.webView?.let {
            it.settings.userAgentString = if (on) desktopUa else mobileUa
            it.reload()
        }
        saveSoon()
    }

    private fun renameTab(tab: Tab) {
        val input = EditText(this).apply {
            setText(tab.name ?: "")
            hint = "e.g. your in-game name"
            setSingleLine()
            selectAll()
        }
        val wrap = FrameLayout(this).apply {
            setPadding(dp(20), dp(8), dp(20), 0)
            addView(input)
        }
        AlertDialog.Builder(this)
            .setTitle("Name this account")
            .setMessage("Named accounts stay in the ▾ list, so you can load them again after closing the tab.")
            .setView(wrap)
            .setPositiveButton("Save") { _, _ ->
                tab.name = input.text.toString().trim().take(40).ifBlank { null }
                saveAccount(tab)
                renderTabs()
                saveSoon()
            }
            .setNegativeButton("Cancel", null)
            .show()
        input.requestFocus()
    }

    // ------------------------------------------------------------- account bar

    /** Top bar: current account name ▾ and "2/4". */
    private fun renderTabs() {
        val t = active ?: return
        accountName.text = t.name ?: t.label()
        val i = tabs.indexOf(t)
        tabPosition.text = if (tabs.size > 1) "${i + 1}/${tabs.size}" else ""
        accountName.setCompoundDrawablesRelativeWithIntrinsicBounds(if (t.pinned) R.drawable.ic_pin else 0, 0, 0, 0)
        accountName.compoundDrawablePadding = dp(4)
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun setupAccountBar() {
        val bar = findViewById<View>(R.id.accountBar)
        findViewById<ImageButton>(R.id.btnPrev).setOnClickListener { cycleTab(-1) }
        findViewById<ImageButton>(R.id.btnNext).setOnClickListener { cycleTab(1) }
        // Swipe left/right on the bar to change tab; tap for the account list.
        val detector = GestureDetector(this, object : GestureDetector.SimpleOnGestureListener() {
            override fun onDown(e: MotionEvent) = true
            override fun onSingleTapUp(e: MotionEvent): Boolean {
                showAccountMenu(bar)
                return true
            }
            override fun onFling(e1: MotionEvent?, e2: MotionEvent, velocityX: Float, velocityY: Float): Boolean {
                val dx = e2.x - (e1?.x ?: e2.x)
                if (kotlin.math.abs(dx) > dp(40) && kotlin.math.abs(velocityX) > kotlin.math.abs(velocityY)) {
                    // Swipe left = next tab, swipe right = previous (like turning pages).
                    cycleTab(if (dx < 0) 1 else -1)
                    return true
                }
                return false
            }
        })
        bar.setOnTouchListener { _, ev -> detector.onTouchEvent(ev) }
    }

    /** The ▾ list: saved accounts, other open tabs, and account actions. */
    private fun showAccountMenu(anchor: View) {
        val pm = PopupMenu(this, anchor)
        val m = pm.menu
        var order = 0
        // Saved accounts (open or not).
        accounts.forEachIndexed { i, acc ->
            val open = tabs.any { it.profile == acc.profile }
            m.add(1, 1000 + i, order++, if (open) acc.name else "${acc.name}  (tap to load)").apply {
                isCheckable = true
                isChecked = active?.profile == acc.profile
            }
        }
        // Open tabs that aren't a saved account.
        tabs.forEachIndexed { i, t ->
            if (accounts.none { it.profile == t.profile }) {
                m.add(2, 2000 + i, order++, t.label()).apply { isCheckable = true; isChecked = t === active }
            }
        }
        m.add(3, 1, order++, "＋ Add account (fresh login)")
        m.add(3, 2, order++, "Name this account…")
        m.add(3, 3, order++, "Tab options…")
        m.add(3, 4, order++, "Close this tab")
        if (accounts.isNotEmpty()) {
            val forget = m.addSubMenu(3, 5, order, "Forget an account")
            accounts.forEachIndexed { i, acc -> forget.add(4, 3000 + i, i, acc.name) }
        }
        pm.setOnMenuItemClickListener { item ->
            val id = item.itemId
            when {
                id in 1000 until 2000 -> accounts.getOrNull(id - 1000)?.let { openAccount(it) }
                id in 2000 until 3000 -> tabs.getOrNull(id - 2000)?.let { activate(it) }
                id in 3000 until 4000 -> accounts.getOrNull(id - 3000)?.let { forgetAccount(it) }
                id == 1 -> addTab("https://totalbattle.com/en/", profile = newTabProfile(), activate = true)
                id == 2 -> active?.let { renameTab(it) }
                id == 3 -> active?.let { showTabMenu(it, anchor) }
                id == 4 -> active?.let { closeTab(it) }
            }
            true
        }
        pm.show()
    }

    /** Switches to the account's tab, or opens a new tab signed in to it. */
    private fun openAccount(acc: SavedAccount) {
        tabs.firstOrNull { it.profile == acc.profile }?.let { activate(it); return }
        val url = acc.url.ifBlank { "https://totalbattle.com/en/" }
        val tab = addTab(url, profile = acc.profile, activate = true)
        tab.name = acc.name
        renderTabs()
        saveSoon()
    }

    private fun forgetAccount(acc: SavedAccount) {
        accounts.remove(acc)
        store.saveAccounts(accounts)
        toast("${acc.name} forgotten")
        renderTabs()
    }

    /** Saves a tab's name as an account (its login is then kept after closing the tab). */
    private fun saveAccount(tab: Tab) {
        val name = tab.name ?: return
        val url = tab.url.takeIf { it.startsWith("http") } ?: ""
        val existing = accounts.firstOrNull { it.profile == tab.profile }
        if (existing != null) {
            existing.name = name
            if (url.isNotEmpty()) existing.url = url
        } else {
            accounts += SavedAccount(tab.profile, name, url)
        }
        store.saveAccounts(accounts)
    }

    /** Remembers the page each account is on, so reopening it goes back there. */
    private fun syncAccountUrls() {
        var changed = false
        for (t in tabs) {
            if (!t.url.startsWith("http")) continue
            accounts.firstOrNull { it.profile == t.profile && it.url != t.url }?.let { it.url = t.url; changed = true }
        }
        if (changed) store.saveAccounts(accounts)
    }

    private fun showTabMenu(tab: Tab, anchor: View) {
        val pm = PopupMenu(this, anchor)
        val m = pm.menu
        m.add(0, 1, 0, "Name this account…")
        m.add(0, 2, 1, "Duplicate tab")
        m.add(0, 3, 2, "Reload")
        m.add(0, 4, 3, "Keep loaded").apply { isCheckable = true; isChecked = tab.pinned }
        m.add(0, 5, 4, "Desktop site").apply { isCheckable = true; isChecked = tab.desktop }
        m.add(0, 6, 5, "Log in with a different account (fresh login)")
        m.add(0, 7, 6, "Move left")
        m.add(0, 8, 7, "Move right")
        m.add(0, 9, 8, "Close tab")
        m.add(0, 10, 9, "Close other tabs").isEnabled = tabs.size > 1
        pm.setOnMenuItemClickListener {
            when (it.itemId) {
                1 -> renameTab(tab)
                2 -> duplicate(tab)
                3 -> tab.webView?.reload() ?: activate(tab)
                4 -> { tab.pinned = !tab.pinned; renderTabs(); saveSoon() }
                5 -> setDesktop(tab, !tab.desktop)
                6 -> freshLogin(tab)
                7, 8 -> moveTab(tab, if (it.itemId == 7) -1 else 1)
                9 -> closeTab(tab)
                10 -> { activate(tab); tabs.filter { t -> t !== tab }.forEach { t -> closeTab(t) } }
            }
            true
        }
        pm.show()
    }

    private fun moveTab(tab: Tab, step: Int) {
        val i = tabs.indexOf(tab)
        val j = i + step
        if (i < 0 || j !in tabs.indices) return
        tabs.removeAt(i)
        tabs.add(j, tab)
        renderTabs()
        saveSoon()
    }

    // ---------------------------------------------------------------- toolbar

    private fun setupToolbar() {
        findViewById<ImageButton>(R.id.btnBack).setOnClickListener { active?.webView?.let { if (it.canGoBack()) it.goBack() } }
        findViewById<ImageButton>(R.id.btnReload).setOnClickListener {
            val t = active ?: return@setOnClickListener
            t.webView?.reload() ?: run { createWebView(t).loadUrl(t.url); showActive() }
        }
        findViewById<ImageButton>(R.id.btnNewTab).setOnClickListener {
            addTab(NEWTAB_URL, profile = newTabProfile(), activate = true)
        }
        findViewById<ImageButton>(R.id.btnMenu).setOnClickListener { showMainMenu(it) }

        address.setOnFocusChangeListener { _, has ->
            if (has) {
                address.setAdapter(ArrayAdapter(this, android.R.layout.simple_dropdown_item_1line, history.suggestions()))
            } else {
                updateAddress()
            }
        }
        address.setOnEditorActionListener { _, actionId, event ->
            val enter = event?.keyCode == KeyEvent.KEYCODE_ENTER && event.action == KeyEvent.ACTION_DOWN
            if (actionId == EditorInfo.IME_ACTION_GO || enter) {
                go(address.text.toString())
                true
            } else false
        }
        address.setOnItemClickListener { parent, _, position, _ -> go(parent.getItemAtPosition(position).toString()) }
    }

    private fun go(input: String) {
        val url = resolveInput(input)
        val tab = active ?: return
        address.clearFocus()
        (getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager).hideSoftInputFromWindow(address.windowToken, 0)
        val wv = tab.webView ?: createWebView(tab).also { showActive() }
        if (isTotalBattle(Uri.parse(url).host) && !tab.desktop) setDesktop(tab, true)
        wv.loadUrl(url)
    }

    private fun resolveInput(input: String): String {
        val t = input.trim()
        if (t.isEmpty()) return NEWTAB_URL
        val lower = t.lowercase()
        if (lower.startsWith("http://") || lower.startsWith("https://") || lower.startsWith("file:")) return t
        val looksLikeHost = !t.contains(' ') && (lower.startsWith("localhost") || (t.contains('.') && !t.endsWith('.') && !t.startsWith('.')))
        if (looksLikeHost) return "https://$t"
        val q = Uri.encode(t)
        return when (store.searchEngine) {
            "google" -> "https://www.google.com/search?q=$q"
            "bing" -> "https://www.bing.com/search?q=$q"
            "brave" -> "https://search.brave.com/search?q=$q"
            else -> "https://duckduckgo.com/?q=$q"
        }
    }

    private fun updateAddress() {
        if (address.hasFocus()) return
        val url = active?.url
        address.setText(if (isNewtab(url)) "" else url, false)
    }

    private fun updateProgress() {
        val p = active?.progress ?: 100
        progress.progress = p
        progress.visibility = if (p in 1..99) View.VISIBLE else View.INVISIBLE
    }

    private fun showMainMenu(anchor: View) {
        val tab = active
        val pm = PopupMenu(this, anchor)
        val m = pm.menu
        m.add(0, 1, 0, "New tab")
        m.add(0, 2, 1, "Fullscreen (back button exits)")
        m.add(0, 3, 2, "Desktop site").apply { isCheckable = true; isChecked = tab?.desktop == true }
        m.add(0, 4, 3, "Keep screen on").apply { isCheckable = true; isChecked = store.keepScreenOn }
        m.add(0, 5, 4, "Ad & tracker blocker").apply { isCheckable = true; isChecked = store.adblock }
        m.add(0, 6, 5, "Separate login for each new tab").apply { isCheckable = true; isChecked = store.isolateTabs }
        m.add(0, 7, 6, "Remember sites (history)").apply { isCheckable = true; isChecked = store.saveHistory }
        m.add(0, 11, 10, "Save memory: one tab loaded at a time").apply { isCheckable = true; isChecked = store.oneAtATime }
        m.add(0, 12, 11, "Pause games when you leave the app").apply { isCheckable = true; isChecked = store.pauseInBackground }
        m.add(0, 15, 13, "Game sound (off saves memory)").apply { isCheckable = true; isChecked = store.gameSound }
        m.add(0, 16, 15, "Turn games sideways").apply { isCheckable = true; isChecked = store.gameLandscape }
        m.add(0, 17, 16, "Experimental: tighter game memory").apply { isCheckable = true; isChecked = store.trimHeap }
        m.add(0, 18, 16, "Lower-quality game graphics (saves memory)").apply { isCheckable = true; isChecked = store.lowTextures }
        m.add(0, 20, 16, "Touch works as mouse in games").apply { isCheckable = true; isChecked = store.touchMouse }
        m.add(0, 14, 17, "Game memory report")
        val perf = m.addSubMenu(0, 13, 12, "Game performance")
        listOf("Sharp (no limits, most battery)", "Balanced (60 FPS, recommended)", "Battery saver (30 FPS)")
            .forEachIndexed { i, label -> perf.add(2, 200 + i, i, label).apply { isChecked = store.performance == i } }
        perf.setGroupCheckable(2, true, true)
        val res = m.addSubMenu(0, 19, 12, "Game resolution")
        listOf("720p (recommended)", "1080p", "Phone screen (sharpest)")
            .forEachIndexed { i, label -> res.add(3, 300 + i, i, label).apply { isChecked = store.gameResolution == i } }
        res.setGroupCheckable(3, true, true)
        val engines = m.addSubMenu(0, 8, 7, "Search engine")
        listOf("duckduckgo" to "DuckDuckGo", "google" to "Google", "bing" to "Bing", "brave" to "Brave Search")
            .forEachIndexed { i, (key, label) ->
                engines.add(1, 100 + i, i, label).apply { isCheckable = true; isChecked = store.searchEngine == key }
            }
        m.add(0, 9, 8, "Clear history")
        m.add(0, 10, 9, "Gaming sidebar")
        pm.setOnMenuItemClickListener {
            when (it.itemId) {
                1 -> addTab(NEWTAB_URL, profile = newTabProfile(), activate = true)
                2 -> setFullscreen(true)
                3 -> tab?.let { t -> setDesktop(t, !t.desktop) }
                4 -> { store.keepScreenOn = !store.keepScreenOn; applyKeepScreenOn() }
                5 -> { store.adblock = !store.adblock; toast(if (store.adblock) "Blocker on" else "Blocker off"); active?.webView?.reload() }
                6 -> { store.isolateTabs = !store.isolateTabs; toast(if (store.isolateTabs) "New tabs get their own login" else "New tabs share one login") }
                7 -> store.saveHistory = !store.saveHistory
                in 100..103 -> store.searchEngine = listOf("duckduckgo", "google", "bing", "brave")[it.itemId - 100]
                9 -> { history.clear(); toast("History cleared") }
                10 -> drawer.openDrawer(Gravity.START)
                12 -> {
                    store.pauseInBackground = !store.pauseInBackground
                    toast(if (store.pauseInBackground) "Games pause while Respawn is in the background" else "Games keep running in the background (uses more battery)")
                }
                in 200..202 -> setPerformance(it.itemId - 200)
                in 300..302 -> if (store.gameResolution != it.itemId - 300) {
                    store.gameResolution = it.itemId - 300
                    if (store.performance == 0) toast("Resolution applies on Balanced or Battery saver")
                    reinstallGameScripts()
                }
                14 -> showMemReport()
                15 -> setGameSound(!store.gameSound)
                16 -> { store.gameLandscape = !store.gameLandscape; applyOrientation() }
                20 -> {
                    store.touchMouse = !store.touchMouse
                    reinstallGameScripts()
                }
                18 -> {
                    store.lowTextures = !store.lowTextures
                    if (store.lowTextures) toast("Large game graphics are stored at half size")
                    reinstallGameScripts()
                }
                17 -> {
                    store.trimHeap = !store.trimHeap
                    if (store.trimHeap) toast("If the game stops loading, turn this off again")
                    reinstallGameScripts()
                }
                11 -> {
                    store.oneAtATime = !store.oneAtATime
                    toast(if (store.oneAtATime) "Other tabs unload when you switch (\"Keep loaded\" tabs stay)" else "All visited tabs stay loaded — may run out of memory with games")
                }
            }
            true
        }
        pm.show()
    }

    // ---------------------------------------------------------------- sidebar

    private fun setupSidebar() {
        findViewById<ImageButton>(R.id.btnSidebar).setOnClickListener { drawer.openDrawer(Gravity.START) }
        val row = findViewById<LinearLayout>(R.id.panelButtons)
        for ((label, url) in panels) {
            val b = TextView(this).apply {
                text = label
                gravity = Gravity.CENTER
                textSize = 13f
                setTextColor(ContextCompat.getColor(context, R.color.muted))
                setOnClickListener { showPanel(label, url, row) }
            }
            row.addView(b, LinearLayout.LayoutParams(0, -1, 1f))
        }
    }

    private fun showPanel(label: String, url: String, row: LinearLayout) {
        val wv = panelViews.getOrPut(label) {
            WebView(this).also { v ->
                configure(v, DEFAULT_PROFILE, false)
                v.webViewClient = object : WebViewClient() {
                    override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
                        if (store.adblock && AdBlock.isBlocked(request.url.host)) AdBlock.emptyResponse() else null
                    // Without this, Android closes Respawn when the shared page process runs out of memory.
                    override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
                        destroyPanels()
                        return true
                    }
                }
                v.webChromeClient = WebChromeClient()
                v.loadUrl(url)
                panelContainer.addView(v, FrameLayout.LayoutParams(-1, -1))
            }
        }
        panelViews.values.forEach { it.visibility = if (it === wv) View.VISIBLE else View.GONE }
        for (i in 0 until row.childCount) {
            val tv = row.getChildAt(i) as TextView
            tv.setTextColor(ContextCompat.getColor(this, if (tv.text == label) R.color.accent else R.color.muted))
        }
    }

    /** Frees the sidebar panels (Discord etc.); they reload when opened again. */
    private fun destroyPanels() {
        panelViews.values.forEach { v ->
            panelContainer.removeView(v)
            runCatching { v.stopLoading(); v.destroy() }
        }
        panelViews.clear()
    }

    // ------------------------------------------------------- fullscreen, back

    private fun setSystemBarsHidden(hidden: Boolean) {
        val c = WindowInsetsControllerCompat(window, window.decorView)
        if (hidden) {
            c.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            c.hide(WindowInsetsCompat.Type.systemBars())
        } else {
            c.show(WindowInsetsCompat.Type.systemBars())
        }
    }

    /** Hides tabs, toolbar and Android's bars: the game gets the whole screen. */
    private fun setFullscreen(on: Boolean) {
        fullscreen = on
        tabBar.visibility = if (on) View.GONE else View.VISIBLE
        toolbar.visibility = if (on) View.GONE else View.VISIBLE
        progress.visibility = if (on) View.GONE else View.INVISIBLE
        setSystemBarsHidden(on)
        if (on) toast("Fullscreen — press Back to exit")
    }

    private fun setupBack() {
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                val wv = active?.webView
                when {
                    customView != null -> (active?.webView?.webChromeClient)?.onHideCustomView()
                    drawer.isDrawerOpen(Gravity.START) -> drawer.closeDrawer(Gravity.START)
                    fullscreen -> setFullscreen(false)
                    wv != null && wv.canGoBack() -> wv.goBack()
                    // Don't close: send Respawn to the background so games keep running.
                    else -> moveTaskToBack(true)
                }
            }
        })
    }

    /** Hardware keyboard: Ctrl+Tab / Ctrl+Shift+Tab switch tabs. */
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (event.action == KeyEvent.ACTION_DOWN && event.isCtrlPressed && event.keyCode == KeyEvent.KEYCODE_TAB) {
            cycleTab(if (event.isShiftPressed) -1 else 1)
            return true
        }
        return super.dispatchKeyEvent(event)
    }

    private fun applyKeepScreenOn() {
        if (store.keepScreenOn) window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        else window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    }

    // ----------------------------------------------------------------- memory

    /** Android is short on memory: unload background tabs (never "Keep loaded" ones). */
    @Suppress("DEPRECATION")
    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        val pressure = level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW && level < ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN
        if (pressure) {
            tabs.filter { it !== active && !it.pinned && it.webView != null }.minByOrNull { it.lastActive }?.let { unload(it) }
            if (!drawer.isDrawerOpen(Gravity.START)) destroyPanels()
        }
    }

    override fun onLowMemory() {
        super.onLowMemory()
        tabs.filter { it !== active && !it.pinned }.forEach { unload(it) }
        if (!drawer.isDrawerOpen(Gravity.START)) destroyPanels()
    }

    // -------------------------------------------------------------- lifecycle

    override fun onResume() {
        super.onResume()
        if (timersPaused) {
            tabs.firstNotNullOfOrNull { it.webView }?.resumeTimers()
            timersPaused = false
        }
        showActive()
        handler.removeCallbacks(memPoll)
        handler.post(memPoll)
    }

    override fun onPause() {
        super.onPause()
        handler.removeCallbacks(memPoll)
        // Stop games drawing and running timers while Respawn isn't on screen.
        if (store.pauseInBackground) {
            tabs.forEach { it.webView?.onPause() }
            tabs.firstNotNullOfOrNull { it.webView }?.let { it.pauseTimers(); timersPaused = true }
        }
        handler.removeCallbacks(saveRunnable)
        saveRunnable.run()
        // Make sure logins are written to disk.
        if (multiProfile) {
            runCatching { tabs.map { it.profile }.toSet().forEach { cookieManager(it).flush() } }
        } else {
            CookieManager.getInstance().flush()
        }
    }

    override fun onDestroy() {
        tabs.forEach { destroyWebView(it) }
        destroyPanels()
        super.onDestroy()
    }

    // ------------------------------------------------------------------ misc

    private fun saveSoon() {
        handler.removeCallbacks(saveRunnable)
        handler.postDelayed(saveRunnable, 1000)
    }

    private fun download(url: String, ua: String, disposition: String?, mime: String?, profile: String) {
        runCatching {
            val name = URLUtil.guessFileName(url, disposition, mime)
            val req = DownloadManager.Request(Uri.parse(url))
                .setMimeType(mime)
                .addRequestHeader("User-Agent", ua)
                .setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
                .setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, name)
            cookieManager(profile).getCookie(url)?.let { req.addRequestHeader("Cookie", it) }
            (getSystemService(DOWNLOAD_SERVICE) as DownloadManager).enqueue(req)
            toast("Downloading $name")
        }.onFailure { toast("Download failed") }
    }

    /** intent://, market://, mailto: and similar open in the matching app. */
    private fun openOutside(uri: Uri) {
        runCatching {
            val intent = if (uri.scheme == "intent") Intent.parseUri(uri.toString(), Intent.URI_INTENT_SCHEME)
            else Intent(Intent.ACTION_VIEW, uri)
            intent.addCategory(Intent.CATEGORY_BROWSABLE)
            intent.component = null
            intent.selector = null
            startActivity(intent)
        }
    }

    private fun toast(msg: String) = Toast.makeText(this, msg, Toast.LENGTH_SHORT).show()

    private fun dp(v: Int) = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, v.toFloat(), resources.displayMetrics).toInt()
}
