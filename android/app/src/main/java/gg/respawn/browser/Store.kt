package gg.respawn.browser

import android.content.Context
import android.net.Uri
import android.webkit.WebView
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

const val DEFAULT_PROFILE = "Default"
const val NEWTAB_URL = "file:///android_asset/newtab.html"
const val BRIDGE_HOST = "respawn.invalid"

fun isNewtab(url: String?) = url == null || url.isEmpty() || url.startsWith("file:///android_asset/newtab")

fun isTotalBattle(host: String?) = host != null && (host == "totalbattle.com" || host.endsWith(".totalbattle.com"))

class Tab(
    val id: Long,
    var url: String,
    var title: String = "",
    /** Name you gave the tab, e.g. the in-game account name. */
    var name: String? = null,
    /** WebView profile = this tab's own cookies/logins. */
    var profile: String = DEFAULT_PROFILE,
    /** "Keep loaded": never unloaded when Android runs low on memory. */
    var pinned: Boolean = false,
    /** Desktop site (Total Battle needs it). */
    var desktop: Boolean = false,
) {
    var webView: WebView? = null
    var lastActive: Long = System.currentTimeMillis()
    var progress: Int = 100
    /** Times this tab auto-reloaded after a game crash (rate limit). */
    val crashReloads = ArrayDeque<Long>()

    val host: String? get() = runCatching { Uri.parse(url).host }.getOrNull()

    /** "Basketkase – Total Battle", or the page title. */
    fun label(): String {
        if (isNewtab(url)) return name ?: "New Tab"
        val base = title.ifBlank { host?.removePrefix("www.") ?: url }
        val n = name ?: return base
        val site = base.split(Regex(":| \\| | - | — ")).first().trim().ifBlank { base }
        return "$n – $site"
    }
}

/** A named login you can reopen even after its tab was closed. */
data class SavedAccount(val profile: String, var name: String, var url: String)

/** Settings + saved tabs. */
class Store(private val ctx: Context) {
    private val prefs = ctx.getSharedPreferences("settings", Context.MODE_PRIVATE)
    private val sessionFile = File(ctx.filesDir, "session.json")

    var searchEngine: String
        get() = prefs.getString("search_engine", "duckduckgo")!!
        set(v) = prefs.edit().putString("search_engine", v).apply()
    var adblock: Boolean
        get() = prefs.getBoolean("adblock", true)
        set(v) = prefs.edit().putBoolean("adblock", v).apply()
    var keepScreenOn: Boolean
        get() = prefs.getBoolean("keep_screen_on", true)
        set(v) = prefs.edit().putBoolean("keep_screen_on", v).apply()
    var isolateTabs: Boolean
        get() = prefs.getBoolean("isolate_tabs", true)
        set(v) = prefs.edit().putBoolean("isolate_tabs", v).apply()
    /** Phones: only the tab you're on stays loaded (plus "Keep loaded" tabs). */
    var oneAtATime: Boolean
        get() = prefs.getBoolean("one_at_a_time", true)
        set(v) = prefs.edit().putBoolean("one_at_a_time", v).apply()
    var saveHistory: Boolean
        get() = prefs.getBoolean("save_history", true)
        set(v) = prefs.edit().putBoolean("save_history", v).apply()

    /** Named accounts, kept even when their tab is closed. */
    fun loadAccounts(): MutableList<SavedAccount> = runCatching {
        val arr = JSONArray(prefs.getString("accounts", "[]"))
        (0 until arr.length()).map { i ->
            val o = arr.getJSONObject(i)
            SavedAccount(o.getString("profile"), o.getString("name"), o.optString("url"))
        }.toMutableList()
    }.getOrDefault(mutableListOf())

    fun saveAccounts(list: List<SavedAccount>) {
        val arr = JSONArray()
        list.forEach { arr.put(JSONObject().put("profile", it.profile).put("name", it.name).put("url", it.url)) }
        prefs.edit().putString("accounts", arr.toString()).apply()
    }

    fun nextProfileName(): String {
        val n = prefs.getInt("profile_counter", 1) + 1
        prefs.edit().putInt("profile_counter", n).apply()
        return "tab-$n"
    }

    fun nextTabId(): Long {
        val n = prefs.getLong("tab_counter", 0) + 1
        prefs.edit().putLong("tab_counter", n).apply()
        return n
    }

    fun saveSession(tabs: List<Tab>, active: Tab?) {
        val arr = JSONArray()
        var activeIndex = 0
        tabs.filter { !isNewtab(it.url) || it.name != null }.forEachIndexed { i, t ->
            if (t === active) activeIndex = i
            arr.put(JSONObject().apply {
                put("id", t.id)
                put("url", t.url)
                put("title", t.title)
                put("name", t.name ?: JSONObject.NULL)
                put("profile", t.profile)
                put("pinned", t.pinned)
                put("desktop", t.desktop)
            })
        }
        val json = JSONObject().put("tabs", arr).put("active", activeIndex).toString()
        runCatching {
            val tmp = File(sessionFile.path + ".tmp")
            tmp.writeText(json)
            tmp.renameTo(sessionFile)
        }
    }

    /** Returns saved tabs and the index of the active one. */
    fun loadSession(): Pair<List<Tab>, Int> = runCatching {
        val o = JSONObject(sessionFile.readText())
        val arr = o.getJSONArray("tabs")
        val tabs = (0 until arr.length()).map { i ->
            val t = arr.getJSONObject(i)
            Tab(
                id = t.getLong("id"),
                url = t.getString("url"),
                title = t.optString("title"),
                name = if (t.isNull("name")) null else t.optString("name").ifBlank { null },
                profile = t.optString("profile", DEFAULT_PROFILE),
                pinned = t.optBoolean("pinned"),
                desktop = t.optBoolean("desktop"),
            )
        }
        tabs to o.optInt("active", 0)
    }.getOrDefault(emptyList<Tab>() to 0)
}
