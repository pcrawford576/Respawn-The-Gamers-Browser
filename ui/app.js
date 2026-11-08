// Respawn browser chrome. Talks to Rust via Tauri IPC.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (s) => document.querySelector(s);
const tabsEl = $('#tabs');
const address = $('#address');
let state = null;

const ICON_PIN = '<svg viewBox="0 0 24 24"><path d="M16 3v2l-1 1v5l3 3v2h-5v5l-1 1-1-1v-5H6v-2l3-3V6L8 5V3z"/></svg>';
const ICON_CLOSE = '<svg viewBox="0 0 24 24"><path d="M18.3 5.7 12 12l6.3 6.3-1.4 1.4L12 13.4l-6.3 6.3-1.4-1.4L10.6 12 4.3 5.7l1.4-1.4 6.3 6.3 6.3-6.3z"/></svg>';

function hostOf(url) {
  try { return new URL(url).hostname.replace(/^www\./, ''); } catch { return ''; }
}

function render() {
  if (!state) return;
  document.body.classList.toggle('game', state.game_mode);
  $('#shield').classList.toggle('on', state.adblock_active);
  $('#shield').title = state.adblock_active ? 'Ad & tracker blocking: on' : 'Ad & tracker blocking: off';

  renderTabs();

  // Address bar (don't clobber what the user is typing)
  const active = state.tabs.find((t) => t.id === state.active);
  if (document.activeElement !== address) address.value = active ? active.url : '';

  renderAccountsLabel();
  renderKeyHints();
  document.body.classList.toggle('fullscreen', !!state.fullscreen);

  // Sidebar
  document.querySelectorAll('.rail-btn').forEach((b) =>
    b.classList.toggle('active', b.dataset.panel === state.sidebar));
}

function renderTabs() {
  // Don't rebuild the strip under the mouse while a tab is being dragged.
  if (drag) { drag.pendingRender = true; return; }
  tabsEl.innerHTML = '';
  for (const t of state.tabs) {
    const el = document.createElement('div');
    el.dataset.id = t.id;
    const isActive = t.id === state.active;
    el.className = 'tab' + (isActive ? ' active' : '') + (t.pinned ? ' pinned' : '') +
      (t.discarded ? ' sleeping' : '') + (t.loading && !t.discarded ? ' loading' : '');
    const status = t.discarded ? 'Unloaded — click to reload'
      : !isActive ? 'Hibernating — still loaded, using minimal resources' : '';
    // A tab's own name prefixes its title: "Basketkase – Total Battle".
    const tabName = t.name;
    el.title = (tabName ? tabName + '\n' : '') + (t.title || t.url) + (status ? '\n' + status : '') +
      (t.pinned ? '\nKept loaded (never unloaded)' : '') + '\nRight-click to rename';
    const baseTitle = t.title || hostOf(t.url) || 'New Tab';
    // With a name, keep just the site part ("Total Battle: Tactical War Game" → "Total Battle").
    const label = tabName
      ? `${tabName} – ${baseTitle.split(/:| \| | - | — /)[0].trim() || baseTitle}`
      : baseTitle;
    const letter = (hostOf(t.url) || label || '?')[0].toUpperCase();
    el.innerHTML = `
      <span class="mono-icon">${t.url ? escapeHtml(letter) : '+'}</span>
      <span class="title"></span>
      ${t.discarded ? '<span class="zz">zz</span>' : ''}
      ${t.url ? `<button class="pin" title="${t.pinned ? 'Kept loaded — click to allow unloading' : 'Keep this tab loaded (for games)'}">${ICON_PIN}</button>` : ''}
      <button class="close" title="Close tab">${ICON_CLOSE}</button>`;
    el.querySelector('.title').textContent = label;
    el.addEventListener('mousedown', (e) => {
      if (e.button === 1) { e.preventDefault(); invoke('close_tab', { id: t.id }); }
    });
    el.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      invoke('tab_menu', { id: t.id });
    });
    el.addEventListener('click', (e) => {
      if (suppressClick) { suppressClick = false; return; }
      if (e.target.closest('.close')) invoke('close_tab', { id: t.id });
      else if (e.target.closest('.pin')) invoke('toggle_pin', { id: t.id });
      else if (t.id !== state.active || t.discarded) invoke('activate_tab', { id: t.id });
    });
    tabsEl.appendChild(el);
  }
}

// ---------- drag tabs to reorder ----------
let drag = null;          // { el, id, startX, offsetX, active }
let suppressClick = false;

tabsEl.addEventListener('pointerdown', (e) => {
  const el = e.target.closest('.tab');
  if (!el || e.button !== 0 || e.target.closest('.close, .pin')) return;
  drag = { el, id: Number(el.dataset.id), startX: e.clientX, active: false, pendingRender: false };
  el.setPointerCapture(e.pointerId);
});

tabsEl.addEventListener('pointermove', (e) => {
  if (!drag) return;
  const dx = e.clientX - drag.startX;
  if (!drag.active) {
    if (Math.abs(dx) < 6) return; // small wiggles still count as a click
    drag.active = true;
    drag.el.classList.add('dragging');
    tabsEl.classList.add('reordering');
  }
  // Move the tab's slot when the pointer crosses a neighbour's midpoint.
  const prev = drag.el.previousElementSibling;
  const next = drag.el.nextElementSibling;
  if (prev && e.clientX < prev.getBoundingClientRect().left + prev.offsetWidth / 2) {
    tabsEl.insertBefore(drag.el, prev);
  } else if (next && e.clientX > next.getBoundingClientRect().left + next.offsetWidth / 2) {
    tabsEl.insertBefore(next, drag.el);
  }
  // Keep the dragged tab under the pointer.
  const slot = drag.el.getBoundingClientRect().left - (parseFloat(drag.el.style.translate) || 0);
  const half = drag.el.offsetWidth / 2;
  const min = tabsEl.getBoundingClientRect().left - slot;
  const max = tabsEl.getBoundingClientRect().right - slot - drag.el.offsetWidth;
  const x = Math.max(min, Math.min(max, e.clientX - slot - half));
  drag.el.style.translate = `${x}px 0`;
});

function endDrag() {
  if (!drag) return;
  const d = drag;
  drag = null;
  d.el.classList.remove('dragging');
  d.el.style.translate = '';
  tabsEl.classList.remove('reordering');
  if (d.active) {
    suppressClick = true;
    setTimeout(() => (suppressClick = false), 0);
    const index = [...tabsEl.children].indexOf(d.el);
    const from = state.tabs.findIndex((t) => t.id === d.id);
    if (index !== from) {
      // Update locally right away so the strip doesn't jump back, then tell Rust.
      const [t] = state.tabs.splice(from, 1);
      state.tabs.splice(index, 0, t);
      invoke('move_tab', { id: d.id, index });
    }
  }
  if (d.pendingRender || d.active) renderTabs();
}
tabsEl.addEventListener('pointerup', endDrag);
tabsEl.addEventListener('pointercancel', endDrag);

// ---------- switch tabs with the keyboard ----------
function cycleTab(step) {
  if (!state || state.tabs.length < 2) return;
  const i = state.tabs.findIndex((t) => t.id === state.active);
  const next = state.tabs[(i + step + state.tabs.length) % state.tabs.length];
  invoke('activate_tab', { id: next.id });
}

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

// ---------- events from Rust ----------
listen('state', (e) => { state = e.payload; render(); });

listen('stats', (e) => {
  const s = e.payload;
  const pct = s.cap_mb ? Math.min(100, (s.mem_mb / s.cap_mb) * 100) : 0;
  const bar = $('#mem-bar');
  bar.style.width = pct + '%';
  bar.classList.toggle('warn', pct > 70 && pct <= 90);
  bar.classList.toggle('hot', pct > 90);
  $('#mem-text').textContent = `${s.mem_mb} / ${s.cap_mb || '∞'} MB`;
  $('#tab-text').textContent = s.sleeping ? `${s.tabs} (${s.sleeping} asleep)` : `${s.tabs}`;
  $('#block-text').textContent = s.blocked_domains ? `${s.blocked_domains} domains` : 'off';
});

let toastTimer;
listen('toast', (e) => {
  const t = $('#toast');
  t.textContent = e.payload;
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (t.hidden = true), 2800);
});

invoke('get_state').then((s) => { state = s; render(); });

// ---------- controls ----------
$('#newtab').addEventListener('click', () => invoke('new_tab', {}));
$('#tabstrip').addEventListener('dblclick', (e) => {
  if (e.target === e.currentTarget || e.target.id === 'tabs') invoke('new_tab', {});
});

document.querySelectorAll('[data-nav]').forEach((b) =>
  b.addEventListener('click', () => invoke('nav_action', { action: b.dataset.nav })));

// ---------- address bar + inline autofill ----------
// As you type, the rest of the best-matching site from your history is filled
// in and selected: keep typing to refine, Backspace to drop it, Tab/→ to accept,
// Enter to go.
let completion = null; // { text, url } currently shown in the bar
let completeSeq = 0;

address.addEventListener('input', async (e) => {
  completion = null;
  const typed = address.value;
  // Only autofill when adding characters at the end, never on delete/paste-edit.
  if (!e.inputType || !e.inputType.startsWith('insert')) return;
  if (address.selectionStart !== typed.length || !typed.trim()) return;
  const seq = ++completeSeq;
  let c = null;
  try { c = await invoke('complete', { input: typed }); } catch { return; }
  // Drop stale answers if the user kept typing.
  if (seq !== completeSeq || address.value !== typed || !c) return;
  if (!c.text.toLowerCase().startsWith(typed.trimStart().toLowerCase())) return;
  const lead = typed.length - typed.trimStart().length;
  const full = typed + c.text.slice(typed.length - lead);
  address.value = full;
  address.setSelectionRange(typed.length, full.length);
  completion = { text: full, url: c.url };
});

$('#omnibox').addEventListener('submit', (e) => {
  e.preventDefault();
  const value = address.value.trim();
  if (!value) return;
  const input = completion && address.value === completion.text ? completion.url : value;
  completion = null;
  invoke('navigate', { input });
  address.blur();
});

address.addEventListener('focus', () => setTimeout(() => address.select(), 0));
address.addEventListener('blur', () => { completion = null; });
address.addEventListener('keydown', (e) => {
  const hasFill = address.selectionStart !== address.selectionEnd &&
    address.selectionEnd === address.value.length && completion;
  if ((e.key === 'Tab' || e.key === 'ArrowRight' || e.key === 'End') && hasFill) {
    e.preventDefault();
    address.setSelectionRange(address.value.length, address.value.length);
  } else if (e.key === 'Escape') {
    if (hasFill) {
      address.value = address.value.slice(0, address.selectionStart);
      completion = null;
    } else {
      address.blur();
      render();
    }
  }
});

document.querySelectorAll('.rail-btn').forEach((b) =>
  b.addEventListener('click', () => invoke('toggle_sidebar', { panel: b.dataset.panel })));

$('#gamemode').addEventListener('click', () => invoke('toggle_game_mode'));

// ---------- keyboard shortcuts (set in Settings → Keyboard shortcuts) ----------
const KEY_ACTIONS = [
  ['prev_tab', 'Previous tab'],
  ['next_tab', 'Next tab'],
  ['fullscreen', 'Full-screen game view'],
  ['game_mode', 'Game Mode', 'works in any program'],
  ['capture', 'Screen capture & markup'],
  ['new_tab', 'New tab'],
  ['close_tab', 'Close tab'],
  ['reload', 'Reload page'],
  ['address', 'Go to the address bar'],
];
const keybinds = () => state?.settings?.keybinds || {};

// Same key names as the in-page script (Rust side): "Ctrl+Shift+G", "Q", "F12"…
function keyName(e) {
  const c = e.code || '';
  if (/^Key[A-Z]$/.test(c)) return c.slice(3);
  if (/^Digit\d$/.test(c)) return c.slice(5);
  if (/^F\d{1,2}$/.test(c)) return c;
  const named = { Tab: 'Tab', Space: 'Space', Enter: 'Enter', Backquote: '`', Minus: '-', Equal: '=',
    BracketLeft: '[', BracketRight: ']', Semicolon: ';', Quote: "'", Comma: ',', Period: '.', Slash: '/',
    Backslash: '\\', PageUp: 'PageUp', PageDown: 'PageDown', Home: 'Home', End: 'End', Insert: 'Insert',
    Delete: 'Delete', ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down' };
  if (named[c]) return named[c];
  if (/^Numpad\d$/.test(c)) return 'Num' + c.slice(6);
  return null;
}
const comboOf = (e) => {
  const k = keyName(e);
  return k ? (e.ctrlKey ? 'Ctrl+' : '') + (e.altKey ? 'Alt+' : '') + (e.shiftKey ? 'Shift+' : '') + k : null;
};

document.addEventListener('keydown', (e) => {
  if (!$('#settings').hidden || !$('#acct-modal').hidden || e.repeat) return;
  const typing = e.target.closest && e.target.closest('input, textarea, select');
  // Esc leaves full screen.
  if (e.key === 'Escape' && state?.fullscreen && !typing) {
    e.preventDefault();
    invoke('run_action', { action: 'fullscreen' });
    return;
  }
  // ← / → also switch tabs while the browser bar has focus.
  if (!typing && !e.ctrlKey && !e.altKey && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
    e.preventDefault();
    cycleTab(e.key === 'ArrowLeft' ? -1 : 1);
    return;
  }
  if (e.ctrlKey && e.key === 'Tab') {
    e.preventDefault();
    cycleTab(e.shiftKey ? -1 : 1);
    return;
  }
  const combo = comboOf(e);
  if (!combo) return;
  const action = Object.keys(keybinds()).find((a) => keybinds()[a] === combo);
  if (!action || action === 'game_mode') return; // Game Mode is handled system-wide
  const plain = !e.ctrlKey && !e.altKey && !/^F\d/.test(combo);
  if (plain && typing) return;
  e.preventDefault();
  if (action === 'address') address.focus();
  else if (action === 'capture') window.startCapture && window.startCapture();
  else invoke('run_action', { action });
});

listen('focus-address', () => { address.focus(); address.select(); });

$('#fullscreen-btn').addEventListener('click', () => invoke('run_action', { action: 'fullscreen' }));

function renderKeyHints() {
  const k = keybinds();
  const parts = [];
  if (k.prev_tab || k.next_tab) parts.push(`${[k.prev_tab, k.next_tab].filter(Boolean).join('/')} switch tabs`);
  if (k.fullscreen) parts.push(`${k.fullscreen} full screen`);
  if (k.game_mode) parts.push(`${k.game_mode} game mode`);
  if (k.capture) parts.push(`${k.capture} capture`);
  $('#hints').textContent = parts.join(' · ');
  document.querySelectorAll('[data-key-for]').forEach((el) => {
    el.textContent = k[el.dataset.keyFor] || '';
    el.hidden = !k[el.dataset.keyFor];
  });
}

// Settings: shortcut editor
let kbDraft = {};
let kbListening = null;
function renderKeybinds() {
  const box = $('#keybinds');
  box.innerHTML = '';
  for (const [action, label, note] of KEY_ACTIONS) {
    const row = document.createElement('div');
    row.className = 'kb-row';
    const cur = kbDraft[action] || '';
    row.innerHTML = `<span>${label}${note ? `<span class="kb-note">(${note})</span>` : ''}</span>
      <button type="button" class="kb-key${cur ? '' : ' empty'}${kbListening === action ? ' listening' : ''}">${
        kbListening === action ? 'Press keys…' : cur || 'Not set'}</button>`;
    row.querySelector('button').onclick = () => { kbListening = action; $('#kb-msg').textContent = ''; renderKeybinds(); };
    box.appendChild(row);
  }
}
// Captures the next key press while a shortcut button is listening.
document.addEventListener('keydown', (e) => {
  if (!kbListening) return;
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;
  e.preventDefault();
  e.stopImmediatePropagation();
  const action = kbListening;
  if (e.key === 'Escape') { kbListening = null; renderKeybinds(); return; }
  if (e.key === 'Backspace' && !e.ctrlKey && !e.altKey) { kbDraft[action] = ''; kbListening = null; renderKeybinds(); return; }
  const combo = comboOf(e);
  if (!combo) { $('#kb-msg').textContent = "That key can't be used — try another."; return; }
  if (action === 'game_mode' && !e.ctrlKey && !e.altKey && !/^F\d/.test(combo)) {
    $('#kb-msg').textContent = 'Game Mode works in every program, so it needs Ctrl or Alt (or an F key).';
    return;
  }
  const clash = Object.keys(kbDraft).find((a) => a !== action && kbDraft[a] === combo);
  if (clash) {
    kbDraft[clash] = '';
    const name = KEY_ACTIONS.find(([a]) => a === clash)[1];
    $('#kb-msg').textContent = `${combo} was used for "${name}" — that one is now not set.`;
  }
  kbDraft[action] = combo;
  kbListening = null;
  renderKeybinds();
}, true);
$('#kb-reset').addEventListener('click', () => {
  kbDraft = { prev_tab: 'Q', next_tab: 'W', fullscreen: 'F12', game_mode: 'Ctrl+Shift+G', capture: 'Ctrl+Shift+X',
    new_tab: 'Ctrl+T', close_tab: 'Ctrl+W', reload: 'Ctrl+R', address: 'Ctrl+L' };
  kbListening = null;
  $('#kb-msg').textContent = 'Defaults restored — click Save to keep them.';
  renderKeybinds();
});

// No browser right-click menu on the chrome itself (except in text fields).
document.addEventListener('contextmenu', (e) => {
  if (!e.target.closest('input, textarea')) e.preventDefault();
});

// ---------- rename a tab / pick its account ----------
const acctModal = $('#acct-modal');
const acctPick = $('#acct-pick');
const acctName = $('#acct-name');
let acctEditing = 0;
let acctOwnSlot = 0;

const savedAccounts = () => state?.settings?.saved_accounts || [];

listen('rename-tab', (e) => {
  acctEditing = e.payload.id;
  acctOwnSlot = e.payload.slot ?? 0;
  // Dropdown: keep this tab's own login, or switch it to a saved account.
  acctPick.innerHTML = '';
  const own = savedAccounts().find((a) => a.slot === acctOwnSlot);
  acctPick.add(new Option(own ? `${own.name} (this tab's login)` : "This tab's current login", ''));
  for (const a of savedAccounts()) {
    if (a.slot === acctOwnSlot) continue;
    const openElsewhere = state.tabs.some((t) => t.account === a.slot);
    acctPick.add(new Option(a.name + (openElsewhere ? '  — open in another tab' : ''), String(a.slot)));
  }
  acctPick.value = '';
  acctName.value = e.payload.name || own?.name || '';
  acctModal.hidden = false;
  invoke('set_overlay', { open: true });
  setTimeout(() => acctName.focus(), 0);
});

acctPick.addEventListener('change', () => {
  const a = savedAccounts().find((x) => String(x.slot) === acctPick.value);
  const own = savedAccounts().find((x) => x.slot === acctOwnSlot);
  acctName.value = a ? a.name : (own?.name || acctName.value);
});

// Typing the name of a saved account selects it.
acctName.addEventListener('input', () => {
  const typed = acctName.value.trim().toLowerCase();
  const a = savedAccounts().find((x) => x.slot !== acctOwnSlot && x.name.toLowerCase() === typed);
  acctPick.value = a ? String(a.slot) : '';
});
function closeAcctModal() {
  acctModal.hidden = true;
  invoke('set_overlay', { open: false });
}
$('#acct-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  const slot = acctPick.value === '' ? null : Number(acctPick.value);
  await invoke('rename_tab', { id: acctEditing, name: acctName.value, slot });
  closeAcctModal();
});
$('#acct-cancel').addEventListener('click', closeAcctModal);
acctModal.addEventListener('click', (e) => { if (e.target === acctModal) closeAcctModal(); });
acctModal.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeAcctModal(); });

// ---------- Accounts ▾ ----------
$('#accounts-btn').addEventListener('click', () => invoke('accounts_menu'));

function renderAccountsLabel() {
  const t = state?.tabs.find((x) => x.id === state.active);
  const acc = savedAccounts().find((a) => t && a.slot === t.account);
  $('#accounts-label').textContent = t?.name || acc?.name || 'Accounts';
}

// ---------- settings ----------
const modal = $('#settings');

function openSettings() {
  const s = state.settings;
  $('#s-engine').value = s.search_engine;
  $('#s-mem').value = s.mem_cap_mb;
  $('#s-gmem').value = s.game_mem_cap_mb;
  $('#s-idle').value = s.discard_idle_min;
  $('#s-renderers').value = s.renderer_limit;
  $('#s-adblock').checked = s.adblock;
  $('#s-extra').value = (s.extra_blocked || []).join('\n');
  $('#s-history').checked = s.save_history !== false;
  $('#s-restore').checked = s.restore_session !== false;
  $('#s-autounload').checked = !!s.auto_unload;
  kbDraft = { ...(s.keybinds || {}) };
  kbListening = null;
  $('#kb-msg').textContent = '';
  renderKeybinds();
  $('#s-isolate').checked = s.isolate_tabs !== false;
  $('#s-dsplit').checked = s.discord_split !== false;
  $('#s-deepl').value = s.deepl_key || '';
  $('#s-trto').value = s.translate_to || 'EN-US';
  $('#s-trauto').checked = s.auto_translate !== false;
  modal.hidden = false;
  invoke('set_overlay', { open: true });
}

function closeSettings() {
  modal.hidden = true;
  invoke('set_overlay', { open: false });
}

$('#settings-btn').addEventListener('click', openSettings);
$('#settings-close').addEventListener('click', closeSettings);
modal.addEventListener('click', (e) => { if (e.target === modal) closeSettings(); });
document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && !modal.hidden) closeSettings(); });

$('#clear-history').addEventListener('click', () => invoke('clear_history'));

$('#settings-save').addEventListener('click', async () => {
  const num = (sel) => Math.max(0, parseInt($(sel).value, 10) || 0);
  const settings = {
    ...state.settings, // keep values that aren't on this form
    search_engine: $('#s-engine').value,
    mem_cap_mb: num('#s-mem'),
    game_mem_cap_mb: num('#s-gmem'),
    discard_idle_min: num('#s-idle'),
    renderer_limit: Math.min(32, num('#s-renderers')),
    adblock: $('#s-adblock').checked,
    extra_blocked: $('#s-extra').value.split('\n').map((l) => l.trim()).filter(Boolean),
    save_history: $('#s-history').checked,
    restore_session: $('#s-restore').checked,
    auto_unload: $('#s-autounload').checked,
    keybinds: { ...kbDraft },
    isolate_tabs: $('#s-isolate').checked,
    discord_split: $('#s-dsplit').checked,
    deepl_key: $('#s-deepl').value.trim(),
    translate_to: $('#s-trto').value,
    auto_translate: $('#s-trauto').checked,
  };
  await invoke('save_settings', { settings });
  closeSettings();
});
