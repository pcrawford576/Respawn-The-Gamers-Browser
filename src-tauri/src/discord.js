// Injected into Discord pages by Respawn. __MODE__ is "panel" (sidebar) or "tab".
// - Split view: the sidebar shows only servers + channels, the tab only the chat.
// - Clicking a channel in the sidebar shows it in one chat tab.
// - DeepL translator: auto-translate incoming messages, Alt+T translates your message.
(() => {
  if (!/(^|\.)discord\.com$/.test(location.hostname)) return;
  const MODE = '__MODE__';
  const ipc = () => window.__TAURI_INTERNALS__;
  const call = (cmd, args) => (ipc() ? ipc().invoke(cmd, args || {}) : Promise.reject('Respawn bridge unavailable'));

  let prefs = { auto: true, to: 'EN-US', reply: 'DE', hasKey: false, split: true, langs: [] };
  const loadPrefs = () => call('discord_prefs').then((p) => { prefs = p; tick(); buildBar(); }).catch(() => {});

  // ------------------------------------------------------------------ styles
  const style = document.createElement('style');
  style.textContent = `
    [data-respawn-hide] { display: none !important; }
    .respawn-tr { margin: 2px 0 2px; padding: 1px 0 1px 8px; border-left: 2px solid #3df5a7; color: #b5bac1;
      font-style: italic; white-space: pre-wrap; }
    .respawn-tr small { font-style: normal; opacity: .65; margin-left: 6px; font-size: .78em; letter-spacing: .04em; }
    #respawn-trbar { position: fixed; right: 18px; bottom: 86px; z-index: 9999; display: flex; gap: 6px; align-items: center;
      background: #10121a; border: 1px solid #2a2f45; border-radius: 10px; padding: 5px 8px; color: #e7e9f3;
      font: 12px "Segoe UI", system-ui, sans-serif; box-shadow: 0 6px 18px rgba(0,0,0,.45); }
    #respawn-trbar button, #respawn-trbar select { background: #0a0b10; color: #e7e9f3; border: 1px solid #2a2f45;
      border-radius: 6px; padding: 3px 7px; font: 12px "Segoe UI", system-ui, sans-serif; cursor: pointer; }
    #respawn-trbar button:hover { border-color: #3df5a7; }
    #respawn-trbar .on { border-color: #3df5a7; color: #3df5a7; }
    #respawn-trbar .muted { color: #8b90a8; }
    #respawn-trbar .msg { color: #ffb347; max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  `;
  // Added once the page exists (this script runs before the page is built).
  const addStyle = () => { if (!style.isConnected) (document.head || document.documentElement).appendChild(style); };

  // -------------------------------------------------------------- split view
  const lca = (a, b) => {
    const seen = new Set();
    for (let n = a; n; n = n.parentElement) seen.add(n);
    for (let n = b; n; n = n.parentElement) if (seen.has(n)) return n;
    return null;
  };
  const childOn = (anc, node) => {
    let n = node;
    while (n && n.parentElement !== anc) n = n.parentElement;
    return n;
  };

  // Works from Discord's stable list markers rather than its (changing) class names.
  function split() {
    const marked = [...document.querySelectorAll('[data-respawn-hide]')];
    const want = new Set();
    if (prefs.split) {
      const chat = document.querySelector('[data-list-id="chat-messages"]');
      const guilds = document.querySelector('[data-list-id="guildsnav"]');
      const channels = document.getElementById('channels');
      if (chat) {
        if (MODE === 'panel') {
          // Sidebar: keep servers + channels, hide the conversation.
          const nav = channels || guilds;
          const a = nav && lca(nav, chat);
          const c = a && childOn(a, chat);
          if (c && !c.contains(nav)) want.add(c);
        } else {
          // Tab: hide servers + channels, keep the conversation.
          for (const nav of [guilds, channels]) {
            if (!nav) continue;
            const a = lca(nav, chat);
            const c = a && childOn(a, nav);
            if (c && !c.contains(chat)) want.add(c);
          }
        }
      }
    }
    marked.forEach((e) => { if (!want.has(e)) e.removeAttribute('data-respawn-hide'); });
    want.forEach((e) => e.setAttribute('data-respawn-hide', ''));
  }

  // ------------------------------------- sidebar: channel clicks -> chat tab
  if (MODE === 'panel') {
    let lastInput = 0;
    addEventListener('pointerdown', () => (lastInput = Date.now()), true);
    addEventListener('keydown', () => (lastInput = Date.now()), true);
    let last = location.pathname;
    const changed = () => {
      if (location.pathname === last) return;
      last = location.pathname;
      // Only when YOU picked a channel (not Discord's own redirects).
      if (prefs.split && /^\/channels\//.test(last) && Date.now() - lastInput < 3000) {
        call('discord_navigated', { url: location.href }).catch(() => {});
      }
    };
    const push = history.pushState;
    history.pushState = function () {
      const r = push.apply(this, arguments);
      setTimeout(changed, 0);
      return r;
    };
    addEventListener('popstate', () => setTimeout(changed, 0));
  }

  // ------------------------------- chat tab: switch channel without reload
  if (MODE === 'tab') {
    window.__respawnGo = (url) => {
      const u = new URL(url);
      if (u.origin !== location.origin || u.pathname === location.pathname) return;
      const title = document.title;
      history.pushState({}, '', u.pathname + u.search);
      dispatchEvent(new PopStateEvent('popstate', { state: {} }));
      // If Discord didn't pick up the change, load the page normally.
      setTimeout(() => { if (document.title === title) location.href = url; }, 2000);
    };
  }

  // -------------------------------------------------------------- translator
  const results = new Map(); // message element id -> {text, src} | null (no translation needed)
  const queue = [];
  let busy = false;
  let lastError = '';

  const COMMON_EN = /\b(the|and|you|your|is|are|to|of|it|that|for|i|we|in|on|my|me|what|this|with|have|was|not|but|lol|ok|yes|no|can|will|just|do|so|if|at|be)\b/i;
  const isTargetAlready = (t) => prefs.to.startsWith('EN') && /^[\x00-\x7F\s]*$/.test(t) && COMMON_EN.test(t);
  const meaningful = (t) =>
    t.replace(/https?:\/\/\S+/g, '').replace(/[@#:][\w-]+/g, '').replace(/[\p{Extended_Pictographic}\p{P}\p{S}\d\s]/gu, '').length >= 2;

  function show(el, res) {
    const next = el.nextElementSibling;
    const has = next && next.classList && next.classList.contains('respawn-tr');
    if (!res) { if (has) next.remove(); return; }
    if (has && next.dataset.for === el.id) return;
    if (has) next.remove();
    const d = document.createElement('div');
    d.className = 'respawn-tr';
    d.dataset.for = el.id;
    d.textContent = res.text;
    const tag = document.createElement('small');
    tag.textContent = `🌐 ${res.src}`;
    d.appendChild(tag);
    el.insertAdjacentElement('afterend', d);
  }

  function enqueue(el, force) {
    if (results.has(el.id) || queue.some((q) => q.id === el.id)) return;
    const text = (el.innerText || '').trim();
    if (!text || !meaningful(text) || (!force && isTargetAlready(text))) { results.set(el.id, null); return; }
    queue.push({ id: el.id, text: text.slice(0, 4000) });
    pump();
  }

  async function pump() {
    if (busy || !queue.length) return;
    busy = true;
    const batch = queue.splice(0, 20);
    try {
      const out = await call('discord_translate', { texts: batch.map((b) => b.text), target: prefs.to });
      const base = prefs.to.split('-')[0];
      batch.forEach((b, i) => {
        const r = out[i];
        const same = !r || (r.src || '').toUpperCase().split('-')[0] === base || r.text.trim() === b.text.trim();
        results.set(b.id, same ? null : r);
        const el = document.getElementById(b.id);
        if (el) show(el, results.get(b.id));
      });
      lastError = '';
    } catch (e) {
      lastError = String(e);
      batch.forEach((b) => results.delete(b.id)); // try again later
      setTimeout(pump, 15000);
    }
    busy = false;
    updateBar();
    if (queue.length) setTimeout(pump, 300);
  }

  function scan() {
    if (MODE !== 'tab' || !prefs.hasKey) return;
    document.querySelectorAll('[id^="message-content-"]').forEach((el) => {
      if (results.has(el.id)) show(el, results.get(el.id));
      else if (prefs.auto) enqueue(el, false);
    });
  }

  // Alt+click a message: translate just that one (works with auto off too).
  addEventListener('click', (e) => {
    if (MODE !== 'tab' || !e.altKey || !prefs.hasKey) return;
    const el = e.target.closest && e.target.closest('[id^="message-content-"]');
    if (!el) return;
    e.preventDefault();
    results.delete(el.id);
    enqueue(el, true);
  }, true);

  // Alt+T in the message box: translate what you typed into the reply language.
  addEventListener('keydown', async (e) => {
    if (MODE !== 'tab' || !e.altKey || e.ctrlKey || (e.key || '').toLowerCase() !== 't') return;
    const box = document.querySelector('[role="textbox"][data-slate-editor="true"]');
    if (!box || !prefs.hasKey) return;
    e.preventDefault();
    e.stopPropagation();
    const text = box.innerText.trim();
    if (!text) return;
    setMsg('Translating…');
    try {
      const [r] = await call('discord_translate', { texts: [text], target: prefs.reply });
      box.focus();
      document.execCommand('selectAll');
      document.execCommand('insertText', false, r.text);
      setMsg('');
    } catch (err) {
      setMsg(String(err));
    }
  }, true);

  // ----------------------------------------------------------- translator bar
  let bar = null;
  function setMsg(m) {
    const s = bar && bar.querySelector('.msg');
    if (s) { s.textContent = m; s.title = m; }
  }
  function updateBar() {
    if (!bar) return;
    const auto = bar.querySelector('[data-k="auto"]');
    if (auto) {
      auto.classList.toggle('on', prefs.auto);
      auto.textContent = `🌐 Auto-translate: ${prefs.auto ? 'On' : 'Off'}`;
    }
    setMsg(lastError);
  }
  function buildBar() {
    if (MODE !== 'tab') return;
    if (bar) bar.remove();
    bar = document.createElement('div');
    bar.id = 'respawn-trbar';
    if (!prefs.hasKey) {
      bar.innerHTML = '<span class="muted">🌐 Translator: add your DeepL key in Respawn ⚙ Settings</span>';
    } else {
      const opts = prefs.langs.map(([c, n]) => `<option value="${c}">${n}</option>`).join('');
      bar.innerHTML = `
        <button data-k="auto" title="Translate new messages into ${prefs.to} automatically. Alt+click a message to translate just that one."></button>
        <span class="muted">Reply in</span>
        <select data-k="reply" title="Language for Alt+T">${opts}</select>
        <button data-k="mine" title="Translate the message you're typing (Alt+T)">Translate mine (Alt+T)</button>
        <span class="msg"></span>`;
      bar.querySelector('[data-k="reply"]').value = prefs.reply;
      bar.querySelector('[data-k="auto"]').onclick = () => {
        prefs.auto = !prefs.auto;
        call('discord_set_prefs', { auto: prefs.auto }).catch(() => {});
        if (!prefs.auto) document.querySelectorAll('.respawn-tr').forEach((d) => d.remove());
        results.clear();
        updateBar();
        scan();
      };
      bar.querySelector('[data-k="reply"]').onchange = (ev) => {
        prefs.reply = ev.target.value;
        call('discord_set_prefs', { reply: prefs.reply }).catch(() => {});
      };
      bar.querySelector('[data-k="mine"]').onclick = () => {
        const box = document.querySelector('[role="textbox"][data-slate-editor="true"]');
        if (box) {
          box.focus();
          box.dispatchEvent(new KeyboardEvent('keydown', { key: 't', altKey: true, bubbles: true }));
        }
      };
    }
    updateBar();
  }

  // -------------------------------------------------------------------- loop
  let pending = false;
  function tick() {
    pending = false;
    split();
    scan();
    if (bar) {
      // Only show the translator bar where there's a chat to type in.
      const hasChat = !!document.querySelector('[role="textbox"][data-slate-editor="true"]');
      if (hasChat && !bar.isConnected) document.body.appendChild(bar);
      if (!hasChat && bar.isConnected) bar.remove();
    }
  }
  const schedule = () => { if (!pending) { pending = true; setTimeout(tick, 250); } };
  const start = () => {
    addStyle();
    new MutationObserver(schedule).observe(document.body, { childList: true, subtree: true });
    setInterval(tick, 1500);
    loadPrefs();
    tick();
  };
  if (document.body) start();
  else addEventListener('DOMContentLoaded', start, { once: true });
  window.__respawnReloadPrefs = loadPrefs;
})();
