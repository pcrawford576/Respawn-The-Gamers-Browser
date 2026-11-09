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

/**
 * Runs before the game's own scripts, on Total Battle pages only (Balanced and Battery saver):
 *  - lays the page out 1280 px wide instead of a wide desktop width, so the game fits
 *    the phone screen without zooming and draws a smaller picture;
 *  - fits the game's drawing inside __LONG__ x __SHORT__ pixels (e.g. 1280 x 720, either way up);
 *    0 = the phone's own screen pixels
 *    (it would otherwise draw the desktop-sized page times the phone's 2.5-3.5x density);
 *  - caps the frame rate at __FPS__ by spacing out requestAnimationFrame.
 */
const val PERF_JS = """
(function () {
  var game = /(^|\.)totalbattle\.com$/i.test(location.hostname);
  if (!game && window.top !== window) { try { game = /totalbattle\.com/i.test(document.referrer); } catch (e) {} }
  if (!game || window.__respawnPerf) return;
  window.__respawnPerf = true;
  var maxLong = __LONG__, maxShort = __SHORT__, fps = __FPS__, width = __WIDTH__;

  if (width > 0 && window.top === window) {
    var addMeta = function () {
      var h = document.head || document.documentElement;
      if (!h) return false;
      var m = document.createElement('meta');
      m.name = 'viewport';
      m.content = 'width=' + width;
      h.appendChild(m);
      return true;
    };
    if (!addMeta()) new MutationObserver(function (_, o) { if (addMeta()) o.disconnect(); })
      .observe(document, { childList: true, subtree: true });
  }

  {
    var real = window.devicePixelRatio || 1;
    var screenL = Math.max(screen.width, screen.height) * real, screenS = Math.min(screen.width, screen.height) * real;   // real pixels
    try {
      Object.defineProperty(window, 'devicePixelRatio', { configurable: true, get: function () {
        var de = document.documentElement;
        var w = Math.max(de ? de.clientWidth : 0, window.innerWidth || 0, 1);
        var h = Math.max(de ? de.clientHeight : 0, window.innerHeight || 0, 1);
        var L = maxLong > 0 ? maxLong : screenL, S = maxShort > 0 ? maxShort : screenS;   // 0 = the phone's own pixels
        var d = Math.min(L / Math.max(w, h), S / Math.min(w, h));         // fits inside L x S, either way up
        return Math.max(0.25, Math.min(real, d));
      } });
    } catch (e) {}
  }

  if (fps > 0) {
    var raf = window.requestAnimationFrame.bind(window), caf = window.cancelAnimationFrame.bind(window);
    var gap = 1000 / fps - 2, last = -1e9, seq = 0, pending = {};
    window.requestAnimationFrame = function (cb) {
      var id = ++seq;
      var tick = function (t) {
        if (!(id in pending)) return;
        if (t === last || t - last >= gap) { last = t; delete pending[id]; cb(t); }
        else pending[id] = raf(tick);
      };
      pending[id] = raf(tick);
      return id;
    };
    window.cancelAnimationFrame = function (id) { if (id in pending) { caf(pending[id]); delete pending[id]; } };
  }
})();
"""

/**
 * Runs before the game's own scripts on Total Battle pages (every performance level):
 * measures the game's memory for the Game memory report, turns off anti-aliasing
 * (__LITEGL__), with game sound off (__MUTE__) skips unpacking sound clips, and with
 * tighter memory on (__TRIM__) makes the game engine grow in small steps, and with
 * lower-quality graphics on (__LOWTEX__) stores large textures at half size, and with
 * touch as mouse on (__TOUCHMOUSE__) turns a finger into the mouse and a pinch into the wheel.
 */
const val GAME_JS = """
(function () {
  var game = /(^|\.)totalbattle\.com$/i.test(location.hostname);
  if (!game && window.top !== window) { try { game = /totalbattle\.com/i.test(document.referrer); } catch (e) {} }
  if (!game || window.__respawnGame) return;
  window.__respawnGame = true;
  var LITE = __LITEGL__, MUTE = __MUTE__, TRIM = __TRIM__, LOWTEX = __LOWTEX__, TOUCHMOUSE = __TOUCHMOUSE__;
  var st = { mems: [], ask: '', audio: 0, clips: 0, canvas: null, aa: null, formats: '', grows: 0, trimmed: 0, shrunk: 0, savedBytes: 0, rtShrunk: 0,
    skip: { small: 0, format: 0, target: 0 }, skipFmt: {} };
  var texSizes = new Map();   // texture -> { 'target:level': [bytes, compressed] }, minus deleted textures
  var MB = function (b) { return Math.round(b / 1048576); };

  // Game engine memory (WebAssembly): what it asks for, and what it really holds.
  var M = window.WebAssembly && WebAssembly.Memory;
  if (M) {
    var keep = function (m) { if (m instanceof M && st.mems.indexOf(m) < 0) st.mems.push(m); };
    var W = function (d) {
      var m = new M(d);
      try { st.ask = 'starts at ' + Math.round(d.initial / 16) + ' MB, max ' + (d.maximum ? Math.round(d.maximum / 16) + ' MB' : 'not set'); } catch (e) {}
      keep(m);
      return m;
    };
    W.prototype = M.prototype;
    WebAssembly.Memory = W;
    var grab = function (r) {
      try { var ex = (r && r.instance ? r.instance : r).exports; for (var k in ex) keep(ex[k]); } catch (e) {}
      return r;
    };
    ['instantiate', 'instantiateStreaming'].forEach(function (n) {
      var f = WebAssembly[n];
      if (f) WebAssembly[n] = function () { return f.apply(WebAssembly, arguments).then(grab); };
    });
    // The game engine asks to grow by ~20% extra, then 10%, then 5% if refused.
    // Experimental: refuse the first two big asks so it grows only by what it needs (+5%).
    var G = M.prototype.grow, tries = 0, pendingReset = false;
    M.prototype.grow = function (delta) {
      if (!pendingReset) { pendingReset = true; Promise.resolve().then(function () { tries = 0; pendingReset = false; }); }
      tries++;
      var cur = this.buffer.byteLength / 65536;
      if (TRIM && tries < 3 && delta > cur * 0.06) { st.trimmed++; throw new RangeError('Respawn: grow in a smaller step'); }
      st.grows++;
      return G.call(this, delta);
    };
  }

  // Sound: browsers unpack every clip into raw audio. With game sound off, clips become silent 1-sample buffers.
  var B = window.BaseAudioContext || window.AudioContext || window.webkitAudioContext;
  var bp = B && B.prototype, dec = bp && bp.decodeAudioData;
  if (dec) bp.decodeAudioData = function (data, ok, fail) {
    var ctx = this;
    if (MUTE) {
      var s = ctx.createBuffer(1, 1, ctx.sampleRate);
      st.clips++;
      if (ok) setTimeout(function () { ok(s); }, 0);
      return Promise.resolve(s);
    }
    var p = dec.call(ctx, data).then(function (b) {
      st.audio += b.length * b.numberOfChannels * 4;
      st.clips++;
      if (ok) ok(b);
      return b;
    }, function (e) { if (fail) fail(e); throw e; });
    if (ok || fail) p.catch(function () {});
    return p;
  };

  // Drawing: no anti-aliasing (saves the extra multisample buffers), low-power GPU mode; count texture memory.
  var isC = function (f) {
    return (f >= 0x83F0 && f <= 0x83F3) || (f >= 0x8C4C && f <= 0x8C4F) || (f >= 0x9270 && f <= 0x9279) ||
      (f >= 0x93B0 && f <= 0x93DD) || f === 0x8D64;
  };
  // Lower-quality graphics: large textures (512 px and up) are stored at half width and
  // half height (a quarter of the memory). Uploads that later change part of a shrunk
  // texture are shrunk to match. Textures made without pixels (drawing targets) are untouched.
  var lowTextures = function (gl, texOf, put) {
    var MIN_SIDE = 512, BYTE = 0x1401;
    var unpack = { 3317: 4, 3314: 0, 3315: 0, 3316: 0 };   // ALIGNMENT, ROW_LENGTH, SKIP_ROWS, SKIP_PIXELS
    var small = new Map();                                 // texture -> { levels: number of levels it has (immutable) or 0 }
    var ps = gl.pixelStorei;
    gl.pixelStorei = function (p, v) { if (p in unpack) unpack[p] = v; return ps.apply(gl, arguments); };
    var plain = function () { return !unpack[3314] && !unpack[3315] && !unpack[3316]; };
    var chans = function (f) {
      switch (f) {
        case 0x1908: return 4;                             // RGBA
        case 0x1907: return 3;                             // RGB
        case 0x190A: case 0x8227: return 2;                // LUMINANCE_ALPHA, RG
        case 0x1906: case 0x1909: case 0x1903: return 1;   // ALPHA, LUMINANCE, RED
      }
      return 0;
    };
    var bytesOf = function (px) { return new Uint8Array(px.buffer, px.byteOffset, px.byteLength); };
    // 16-bit packed pixels: [shift, mask] per channel.
    var PACKED = {
      0x8033: [[12, 15], [8, 15], [4, 15], [0, 15]],       // UNSIGNED_SHORT_4_4_4_4
      0x8034: [[11, 31], [6, 31], [1, 31], [0, 1]],        // UNSIGNED_SHORT_5_5_5_1
      0x8363: [[11, 31], [5, 63], [0, 31]]                 // UNSIGNED_SHORT_5_6_5
    };
    var canShrink = function (format, type) { return (type === BYTE && chans(format) > 0) || (PACKED[type] && (format === 0x1908 || format === 0x1907)); };
    var bppOf = function (format, type) { return type === BYTE ? chans(format) : 2; };
    // Sized formats whose pixels arrive as bytes or 16-bit packed (RGBA8, RGB8, R8, RG8, sRGB, RGBA4, RGB565, RGB5_A1).
    var SIZED = { 0x8058: 1, 0x8051: 1, 0x8229: 1, 0x822B: 1, 0x8C43: 1, 0x8C41: 1, 0x8056: 1, 0x8D62: 1, 0x8057: 1 };
    var skip = function (why, bytes, type) {
      st.skip[why] += bytes;
      if (why === 'format') { var k = '0x' + (type || 0).toString(16); st.skipFmt[k] = (st.skipFmt[k] || 0) + bytes; }
    };
    var half16 = function (px, w, h, f) {
      var al = unpack[3317] || 4, fl = PACKED[f];
      var nw = Math.max(1, w >> 1), nh = Math.max(1, h >> 1);
      var ss = Math.ceil(w * 2 / al) * al / 2, ds = Math.ceil(nw * 2 / al) * al / 2;
      var src = new Uint16Array(px.buffer, px.byteOffset, px.byteLength >> 1), out = new Uint16Array(ds * nh);
      for (var y = 0; y < nh; y++) {
        var r0 = Math.min(h - 1, 2 * y) * ss, r1 = Math.min(h - 1, 2 * y + 1) * ss;
        for (var x = 0; x < nw; x++) {
          var c0 = Math.min(w - 1, 2 * x), c1 = Math.min(w - 1, 2 * x + 1);
          var p0 = src[r0 + c0], p1 = src[r0 + c1], p2 = src[r1 + c0], p3 = src[r1 + c1], v = 0;
          for (var i = 0; i < fl.length; i++) {
            var sh = fl[i][0], m = fl[i][1];
            v |= ((((p0 >> sh) & m) + ((p1 >> sh) & m) + ((p2 >> sh) & m) + ((p3 >> sh) & m) + 2) >> 2) << sh;
          }
          out[y * ds + x] = v;
        }
      }
      return { data: out, w: nw, h: nh };
    };
    var shrinkPixels = function (px, w, h, format, type) {
      return PACKED[type] ? half16(px, w, h, type) : half(bytesOf(px), w, h, chans(format));
    };
    var half = function (src, w, h, c) {                   // 2x2 average, rows padded to UNPACK_ALIGNMENT
      var al = unpack[3317] || 4;
      var nw = Math.max(1, w >> 1), nh = Math.max(1, h >> 1);
      var ss = Math.ceil(w * c / al) * al, ds = Math.ceil(nw * c / al) * al;
      var out = new Uint8Array(ds * nh);
      for (var y = 0; y < nh; y++) {
        var r0 = Math.min(h - 1, 2 * y) * ss, r1 = Math.min(h - 1, 2 * y + 1) * ss, o = y * ds;
        for (var x = 0; x < nw; x++) {
          var c0 = Math.min(w - 1, 2 * x) * c, c1 = Math.min(w - 1, 2 * x + 1) * c, d = o + x * c;
          for (var k = 0; k < c; k++) out[d + k] = (src[r0 + c0 + k] + src[r0 + c1 + k] + src[r1 + c0 + k] + src[r1 + c1 + k] + 2) >> 2;
        }
      }
      return { data: out, w: nw, h: nh };
    };
    var srcOf = function (a, i) {                          // pixel data argument (with WebGL2 srcOffset), or null
      var px = a[i];
      if (!px || typeof px === 'number' || !ArrayBuffer.isView(px)) return null;
      if (a.length > i + 1 && typeof a[i + 1] === 'number' && px.subarray) px = px.subarray(a[i + 1]);
      return px;
    };

    var ti = gl.texImage2D;
    gl.texImage2D = function () {
      var a = arguments;
      try {
        if (a.length >= 9) {
          var t = texOf(a[0]), lvl = a[1], w = a[3], h = a[4], px = srcOf(a, 8), ok = canShrink(a[6], a[7]);
          if (t && lvl === 0) { small.delete(t); pending.delete(t); }
          var shrink = ok && plain() && (lvl === 0 ? px && Math.max(w, h) >= MIN_SIDE : t && small.has(t));
          if (lvl === 0 && px && !shrink) {
            var bytes = w * h * 4;
            if (Math.max(w, h) < MIN_SIDE) skip('small', bytes); else if (!ok) skip('format', bytes, a[7]); else skip('target', bytes);
          }
          if (shrink && px) {
            var r = shrinkPixels(px, w, h, a[6], a[7]);
            if (lvl === 0) { small.set(t, { levels: 0 }); st.shrunk++; }
            st.savedBytes += (w * h - r.w * r.h) * bppOf(a[6], a[7]);
            put(a[0], lvl, r.w * r.h * 4, false);
            return ti.call(gl, a[0], lvl, a[2], r.w, r.h, 0, a[6], a[7], r.data);
          }
          if (shrink && !px) return ti.call(gl, a[0], lvl, a[2], Math.max(1, w >> 1), Math.max(1, h >> 1), 0, a[6], a[7], null);
        }
      } catch (e) {}
      return ti.apply(gl, a);
    };

    var ts2 = gl.texStorage2D, pending = new Map();
    // Single-level storage could be a picture or a drawing target, so wait to reserve it
    // until the game either fills it with pixels (store it halved) or uses it any other way (full size).
    var allocate = function (t, shrinkIt) {
      var p = pending.get(t);
      if (!p) return;
      pending.delete(t);
      var prev = texOf(p.target), w = p.w, h = p.h;
      if (shrinkIt) {
        w = Math.max(1, w >> 1); h = Math.max(1, h >> 1);
        small.set(t, { levels: 1 });
        st.shrunk++;
        st.savedBytes += (p.w * p.h - w * h) * 4;
      } else skip('target', p.w * p.h * 4);
      if (prev !== t) gl.bindTexture(p.target, t);
      ts2.call(gl, p.target, 1, p.fmt, w, h);
      if (prev !== t) gl.bindTexture(p.target, prev || null);
    };
    var flushAll = function () { pending.forEach(function (_, t) { allocate(t, false); }); };
    if (ts2) gl.texStorage2D = function (target, levels, fmt, w, h) {
      try {
        var t = texOf(target);
        if (t) { small.delete(t); pending.delete(t); }
        if (t && levels === 1 && target === 0x0DE1 && SIZED[fmt] && Math.max(w, h) >= MIN_SIDE) {
          pending.set(t, { target: target, fmt: fmt, w: w, h: h });
          return;
        }
        if (Math.max(w, h) < MIN_SIDE) skip('small', w * h * 4);
        else if (levels === 1 || !SIZED[fmt]) skip(SIZED[fmt] ? 'target' : 'format', w * h * 4, fmt);
        // Mipmapped storage: drawing targets are almost never mipmapped.
        if (t && levels > 1 && SIZED[fmt] && Math.max(w, h) >= MIN_SIDE) {
          var nw = Math.max(1, w >> 1), nh = Math.max(1, h >> 1);
          var nl = Math.min(levels, Math.floor(Math.log2(Math.max(nw, nh))) + 1);
          small.set(t, { levels: nl });
          st.shrunk++;
          st.savedBytes += (w * h - nw * nh) * 4 * 1.33;
          put(target, 's', nw * nh * 4 * 1.33, false);
          return ts2.call(gl, target, nl, fmt, nw, nh);
        }
      } catch (e) {}
      return ts2.apply(gl, arguments);
    };

    var tsi = gl.texSubImage2D;
    gl.texSubImage2D = function () {
      var a = arguments;
      try {
        var t = a.length >= 9 && texOf(a[0]);
        if (t && pending.has(t)) {
          var p0 = srcOf(a, 8);
          allocate(t, !!(p0 && canShrink(a[6], a[7]) && plain()));
        }
        var info = t && small.get(t);
        if (info) {
          if (info.levels && a[1] >= info.levels) return;  // that level no longer exists
          var px = srcOf(a, 8);
          if (canShrink(a[6], a[7]) && px && plain()) {
            var r = shrinkPixels(px, a[4], a[5], a[6], a[7]);
            return tsi.call(gl, a[0], a[1], a[2] >> 1, a[3] >> 1, r.w, r.h, a[6], a[7], r.data);
          }
        }
      } catch (e) {}
      return tsi.apply(gl, arguments);
    };

    var fbt = gl.framebufferTexture2D;
    gl.framebufferTexture2D = function () {
      try { allocate(arguments[3], false); if (small.has(arguments[3])) st.rtShrunk++; } catch (e) {}
      return fbt.apply(gl, arguments);
    };
    // Anything else that uses a waiting texture gets it at full size first.
    var before = function (name, fn) {
      var f = gl[name];
      if (f) gl[name] = function () { try { fn(arguments); } catch (e) {} return f.apply(gl, arguments); };
    };
    before('copyTexSubImage2D', function (a) { allocate(texOf(a[0]), false); });
    before('generateMipmap', function (a) { allocate(texOf(a[0]), false); });
    before('deleteTexture', function (a) { pending.delete(a[0]); small.delete(a[0]); });
    ['drawArrays', 'drawElements', 'drawArraysInstanced', 'drawElementsInstanced', 'drawRangeElements',
     'clear', 'readPixels'].forEach(function (n) { before(n, function () { if (pending.size) flushAll(); }); });
  };

  var hook = function (gl) {
    var unit = 0, bound = {};
    var wrap = function (name, count) {
      var f = gl[name];
      if (!f) return;
      gl[name] = function () { try { count(arguments); } catch (e) {} return f.apply(gl, arguments); };
    };
    // Which texture an upload goes to: the one bound to that target on the active unit.
    var texOf = function (target) { return bound[unit + ':' + ((target >= 0x8515 && target <= 0x851A) ? 0x8513 : target)]; };
    var put = function (target, level, bytes, compressed) {
      var t = texOf(target);
      if (!t || !bytes) return;
      var rec = texSizes.get(t) || {};
      rec[target + ':' + level] = [bytes, compressed];
      texSizes.set(t, rec);
    };
    wrap('activeTexture', function (a) { unit = a[0]; });
    wrap('bindTexture', function (a) { bound[unit + ':' + a[0]] = a[1]; });
    wrap('deleteTexture', function (a) { texSizes.delete(a[0]); });
    wrap('texImage2D', function (a) {
      var w, h;
      if (a.length >= 9) { w = a[3]; h = a[4]; }
      else { var s = a[5]; w = s && (s.videoWidth || s.naturalWidth || s.width); h = s && (s.videoHeight || s.naturalHeight || s.height); }
      if (w && h) put(a[0], a[1], w * h * 4, false);
    });
    wrap('texStorage2D', function (a) { if (!isC(a[2])) put(a[0], 's', a[3] * a[4] * 4 * (a[1] > 1 ? 1.33 : 1), false); });
    if (LOWTEX) lowTextures(gl, texOf, put);
    wrap('compressedTexImage2D', function (a) { var d = a[6]; if (d && d.byteLength) put(a[0], a[1], d.byteLength, true); });
    wrap('compressedTexSubImage2D', function (a) { var d = a[7]; if (d && d.byteLength) put(a[0], 'c' + a[1], d.byteLength, true); });
    try {
      var ex = gl.getSupportedExtensions() || [], f = [];
      if (ex.some(function (x) { return /s3tc/i.test(x); })) f.push('DXT (desktop)');
      if (ex.some(function (x) { return /compressed_texture_etc$/i.test(x); })) f.push('ETC2');
      if (ex.some(function (x) { return /astc/i.test(x); })) f.push('ASTC');
      st.formats = f.join(', ') || 'none';
    } catch (e) {}
  };
  var gc = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function (type, attrs) {
    if (!/webgl/i.test(String(type))) return gc.apply(this, arguments);
    if (LITE) attrs = Object.assign({}, attrs || {}, { antialias: false, powerPreference: 'low-power', preserveDrawingBuffer: false });
    var gl = gc.call(this, type, attrs);
    if (gl && !gl.__respawn) {
      gl.__respawn = true;
      st.canvas = this;
      if (TOUCHMOUSE) try { this.style.touchAction = 'none'; } catch (e) {}
      try { st.aa = !!gl.getContextAttributes().antialias; } catch (e) {}
      hook(gl);
    }
    return gl;
  };

  // Touch as mouse: the game follows the mouse, which a finger drag doesn't move, so after
  // panning the map jumped back to the last tap. Here one finger becomes the mouse and a
  // two-finger pinch becomes the mouse wheel (zoom). The page itself no longer scrolls or zooms.
  if (TOUCHMOUSE) {
    var pts = new Map(), mode = null, el = null, lx = 0, ly = 0, sx = 0, sy = 0, pinchD = 0, pressed = false, timer = 0;
    // The press waits a moment (or until the finger moves) so a second finger can turn it into a pinch instead of a click.
    var press = function () { clearTimeout(timer); if (mode === 'drag' && !pressed) { pressed = true; send('down', sx, sy, 1); if (lx !== sx || ly !== sy) send('move', lx, ly, 1); } };
    var spread = function () {
      var a = Array.from(pts.values());
      return Math.hypot(a[0].x - a[1].x, a[0].y - a[1].y);
    };
    var middle = function () {
      var a = Array.from(pts.values());
      return { x: (a[0].x + a[1].x) / 2, y: (a[0].y + a[1].y) / 2 };
    };
    var send = function (kind, x, y, buttons) {
      var o = { bubbles: true, cancelable: true, composed: true, view: window, clientX: x, clientY: y,
                screenX: x, screenY: y, button: 0, buttons: buttons };
      try {
        el.dispatchEvent(new PointerEvent('pointer' + kind, Object.assign({ pointerId: 1, pointerType: 'mouse',
          isPrimary: true, width: 1, height: 1, pressure: buttons ? 0.5 : 0 }, o)));
      } catch (e) {}
      el.dispatchEvent(new MouseEvent('mouse' + kind, o));
    };
    var isCanvas = function (t) { return t && t.tagName === 'CANVAS'; };
    var stop = function (e) { e.stopImmediatePropagation(); if (e.cancelable) e.preventDefault(); };
    addEventListener('pointerdown', function (e) {
      if (e.pointerType !== 'touch' || !isCanvas(e.target)) return;
      stop(e);
      pts.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (pts.size === 1) {
        mode = 'drag'; el = e.target; lx = sx = e.clientX; ly = sy = e.clientY; pressed = false;
        send('move', lx, ly, 0);
        timer = setTimeout(press, 120);
      } else if (pts.size === 2) {
        clearTimeout(timer);
        if (mode === 'drag' && pressed) send('up', lx, ly, 0);   // already dragging: let go, then pinch
        mode = 'pinch';
        pinchD = spread();
      }
    }, true);
    addEventListener('pointermove', function (e) {
      if (e.pointerType !== 'touch' || !pts.has(e.pointerId)) return;
      stop(e);
      pts.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (mode === 'drag') {
        lx = e.clientX; ly = e.clientY;
        if (!pressed && Math.hypot(lx - sx, ly - sy) > 6) press();
        if (pressed) send('move', lx, ly, 1);
      } else if (mode === 'pinch' && pts.size >= 2) {
        var d = spread();
        if (Math.abs(d - pinchD) > 4) {
          var c = middle();
          el.dispatchEvent(new WheelEvent('wheel', { bubbles: true, cancelable: true, clientX: c.x, clientY: c.y,
            deltaY: (pinchD - d) * 3, deltaMode: 0 }));
          pinchD = d;
        }
      }
    }, true);
    var lift = function (e) {
      if (e.pointerType !== 'touch' || !pts.has(e.pointerId)) return;
      stop(e);
      pts.delete(e.pointerId);
      if (pts.size === 0) {
        if (mode === 'drag') { press(); send('up', lx, ly, 0); }   // a quick tap still clicks
        mode = null;
      }
    };
    addEventListener('pointerup', lift, true);
    addEventListener('pointercancel', lift, true);
    // Keep the raw touches away from the game and stop the page scrolling or zooming.
    ['touchstart', 'touchmove', 'touchend', 'touchcancel'].forEach(function (n) {
      addEventListener(n, function (e) { if (isCanvas(e.target)) stop(e); }, { capture: true, passive: false });
    });
  }

  // Outside sites this page loaded from (analytics, chat widgets... candidates for the blocker).
  var hosts = function () {
    try {
      var n = {};
      performance.getEntriesByType('resource').forEach(function (e) {
        var h = new URL(e.name).hostname;
        if (!/(^|\.)totalbattle\.com$/i.test(h)) n[h] = (n[h] || 0) + 1;
      });
      return Object.keys(n).sort(function (x, y) { return n[y] - n[x]; }).slice(0, 10)
        .map(function (h) { return h + ' (' + n[h] + ')'; }).join(', ');
    } catch (e) { return ''; }
  };

  // Report (read by the app every few seconds; frames send theirs to the top page).
  var rep = function () {
    var w = 0;
    st.mems.forEach(function (m) { try { w += m.buffer.byteLength; } catch (e) {} });
    var c = st.canvas, tex = 0, texC = 0, n = 0;
    texSizes.forEach(function (rec) {
      n++;
      for (var k in rec) { if (rec[k][1]) texC += rec[k][0]; else tex += rec[k][0]; }
    });
    return {
      wasmMB: MB(w), wasmAsk: st.ask, texMB: MB(tex), texCompressedMB: MB(texC), textures: n,
      grows: st.grows, trimmed: st.trimmed, trim: TRIM,
      lowtex: LOWTEX, shrunk: st.shrunk, savedMB: MB(st.savedBytes), rtShrunk: st.rtShrunk,
      skipSmallMB: MB(st.skip.small), skipFormatMB: MB(st.skip.format), skipTargetMB: MB(st.skip.target),
      skipFormats: Object.keys(st.skipFmt).map(function (k) { return k + ' ' + MB(st.skipFmt[k]) + ' MB'; }).join(', '),
      hosts: hosts(),
      audioMB: MB(st.audio), audioClips: st.clips, muted: MUTE,
      jsMB: (window.performance && performance.memory) ? MB(performance.memory.usedJSHeapSize) : -1,
      canvas: c ? c.width + 'x' + c.height : '', aa: st.aa, formats: st.formats
    };
  };
  if (window.top === window) {
    var frames = {};
    addEventListener('message', function (e) { if (e.data && e.data.respawnMem) frames[e.origin] = e.data.respawnMem; });
    window.__respawnMemAll = function () {
      var best = rep(), score = function (r) { return r.wasmMB + r.texMB + r.texCompressedMB; };
      for (var k in frames) if (score(frames[k]) > score(best)) best = frames[k];
      return best;
    };
  } else {
    setInterval(function () { try { window.top.postMessage({ respawnMem: rep() }, '*'); } catch (e) {} }, 2000);
  }
})();
"""

/** Reads the Game memory report from the page (empty when it's not a game page). */
const val MEM_READ_JS =
    "(function(){try{return window.__respawnMemAll?JSON.stringify(window.__respawnMemAll()):''}catch(e){return ''}})()"

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
    /** Game performance: 0 = Sharp, 1 = Balanced, 2 = Battery saver. */
    var performance: Int
        get() = prefs.getInt("performance", 1)
        set(v) = prefs.edit().putInt("performance", v).apply()
    /** Game sound; off skips unpacking sound clips, which can take 100+ MB. */
    var gameSound: Boolean
        get() = prefs.getBoolean("game_sound", true)
        set(v) = prefs.edit().putBoolean("game_sound", v).apply()
    /** Turn the phone sideways for game tabs (games are built for wide screens). */
    var gameLandscape: Boolean
        get() = prefs.getBoolean("game_landscape", false)
        set(v) = prefs.edit().putBoolean("game_landscape", v).apply()
    /** Experimental: make the game engine grow its memory in small steps. */
    var trimHeap: Boolean
        get() = prefs.getBoolean("trim_heap", false)
        set(v) = prefs.edit().putBoolean("trim_heap", v).apply()
    /** Store the game's large textures at half size (a quarter of the memory). */
    var lowTextures: Boolean
        get() = prefs.getBoolean("low_textures", false)
        set(v) = prefs.edit().putBoolean("low_textures", v).apply()
    /** Game drawing size: 0 = 720p (720 x 1280), 1 = 1080p, 2 = the phone's screen. */
    var gameResolution: Int
        get() = prefs.getInt("game_resolution", 0)
        set(v) = prefs.edit().putInt("game_resolution", v).apply()
    /** Finger works as the mouse in games (one finger drags, two fingers zoom). */
    var touchMouse: Boolean
        get() = prefs.getBoolean("touch_mouse", true)
        set(v) = prefs.edit().putBoolean("touch_mouse", v).apply()
    /** Pause games (all page timers) while Respawn is in the background. */
    var pauseInBackground: Boolean
        get() = prefs.getBoolean("pause_in_background", true)
        set(v) = prefs.edit().putBoolean("pause_in_background", v).apply()

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
