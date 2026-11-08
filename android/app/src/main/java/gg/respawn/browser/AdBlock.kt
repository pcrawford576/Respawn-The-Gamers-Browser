package gg.respawn.browser

import android.content.Context
import android.webkit.WebResourceResponse
import java.io.ByteArrayInputStream

/** Blocks requests to ad/tracker domains (same list as the Windows version). */
object AdBlock {
    private val blocked = HashSet<String>()

    fun init(ctx: Context) {
        if (blocked.isNotEmpty()) return
        runCatching {
            ctx.assets.open("blocklist.txt").bufferedReader().useLines { lines ->
                lines.map { it.trim().lowercase() }
                    .filter { it.isNotEmpty() && !it.startsWith("#") && it.contains('.') }
                    .forEach { blocked += it }
            }
        }
    }

    val size get() = blocked.size

    /** True if the host or any parent domain is on the list. */
    fun isBlocked(host: String?): Boolean {
        var h = host?.lowercase()?.trimEnd('.') ?: return false
        while (true) {
            if (h in blocked) return true
            val i = h.indexOf('.')
            if (i < 0) return false
            h = h.substring(i + 1)
        }
    }

    fun emptyResponse() = WebResourceResponse(
        "text/plain", "utf-8", 403, "Blocked", emptyMap(), ByteArrayInputStream(ByteArray(0))
    )
}
