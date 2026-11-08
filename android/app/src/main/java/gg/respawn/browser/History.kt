package gg.respawn.browser

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** Visited sites, for address-bar suggestions. */
class History(ctx: Context) {
    private class Entry(val url: String, var visits: Int, var last: Long)

    private val file = File(ctx.filesDir, "history.json")
    private val entries = mutableListOf<Entry>()
    private var dirty = false

    init {
        runCatching {
            val arr = JSONArray(file.readText())
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                entries += Entry(o.getString("url"), o.optInt("visits", 1), o.optLong("last"))
            }
        }
    }

    fun record(url: String) {
        if (!(url.startsWith("http://") || url.startsWith("https://")) || url.contains(BRIDGE_HOST) || url.length > 2048) return
        val now = System.currentTimeMillis()
        val e = entries.firstOrNull { it.url == url }
        if (e != null) {
            if (now - e.last > 30_000) e.visits++
            e.last = now
        } else {
            entries += Entry(url, 1, now)
            if (entries.size > 5000) entries.minByOrNull { it.last }?.let { entries.remove(it) }
        }
        dirty = true
    }

    fun clear() {
        entries.clear()
        dirty = true
        flush()
    }

    fun flush() {
        if (!dirty) return
        dirty = false
        val arr = JSONArray()
        entries.forEach { arr.put(JSONObject().put("url", it.url).put("visits", it.visits).put("last", it.last)) }
        runCatching { file.writeText(arr.toString()) }
    }

    /**
     * Suggestions, best first: whole sites ("twitch.tv") rank by all their
     * visits; individual pages ("twitch.tv/directory") follow.
     */
    fun suggestions(limit: Int = 300): List<String> {
        val now = System.currentTimeMillis()
        val score = HashMap<String, Long>()
        for (e in entries) {
            val age = now - e.last
            val s = e.visits * 10L + when {
                age < 86_400_000L -> 30
                age < 7 * 86_400_000L -> 15
                age < 30 * 86_400_000L -> 5
                else -> 0
            }
            val rest = e.url.substringAfter("://").trimEnd('/')
            val host = rest.substringBefore('/').substringBefore('?').removePrefix("www.")
            score.merge(host, s) { a, b -> a + b }
            val page = rest.removePrefix("www.")
            if (page != host) score.merge(page, s) { a, b -> a + b }
        }
        return score.entries.sortedWith(compareByDescending<Map.Entry<String, Long>> { it.value }.thenBy { it.key.length })
            .take(limit).map { it.key }
    }
}
