// Respawn feature walkthrough.
// Highlights each part of the browser and explains what you can do with it.
// Runs once on first launch; replay any time with the ? button or Settings → Take the tour.
(() => {
  const K = (k) => `<kbd>${k}</kbd>`;

  // target: CSS selector to highlight (null = centered card).
  const STEPS = [
    {
      target: null,
      title: 'Welcome to Respawn, The Gamers Browser',
      body: `A lightweight browser built for gamers. This quick tour shows everything Respawn can do.
             Use ${K('→')} / ${K('←')} or the buttons to move through it, and ${K('Esc')} to skip.`,
    },
    {
      target: '#tabs',
      title: 'Tabs',
      body: `<ul>
        <li><b>Click</b> a tab to switch to it. <b>Drag</b> tabs to reorder them.</li>
        <li><b>Middle-click</b> or the ✕ closes a tab. <b>Double-click</b> empty space for a new tab.</li>
        <li>Your tabs, their order and names come back when you restart Respawn.</li>
      </ul>`,
    },
    {
      target: '#tabs',
      title: 'Right-click a tab',
      body: `<ul>
        <li><b>Rename tab…</b> — e.g. "Basketkase – Total Battle". Pick a saved account from the dropdown to load it in this tab.</li>
        <li><b>Duplicate tab</b>, <b>Reload</b>, <b>Close other tabs</b>.</li>
        <li><b>Log in with a different account</b> gives the tab a fresh, empty login.</li>
      </ul>`,
    },
    {
      target: '#tabs',
      title: 'Hibernating tabs & Keep loaded',
      body: `Tabs you're not looking at <b>hibernate</b>: they stay loaded (your game keeps running) but use
             minimal CPU and memory.<br><br>
             Click the <b>📌 pin</b> on a tab (or right-click → <b>Keep loaded</b>) so it's never unloaded, even
             if you turn on automatic unloading in Settings.`,
    },
    {
      target: '#accounts-btn',
      title: 'Separate logins per tab — Accounts ▾',
      body: `Every tab has its <b>own login</b>, so several tabs can play different accounts of the same game at once,
             and each tab always reopens signed in to its account.<br><br>
             Name a tab to <b>save it as an account</b>. This button lists your saved accounts: click one to jump
             to its tab or reopen it, even after its tab was closed.`,
    },
    {
      target: '#omnibox',
      title: 'Address bar',
      body: `<ul>
        <li>Type a web address or a search (search engine is in Settings).</li>
        <li><b>Autofill</b>: Respawn completes sites you've visited — ${K('Enter')} to go, ${K('Tab')} / ${K('→')} to accept, ${K('Esc')} to drop it.</li>
        <li>Links that open a new window open as a new tab, signed in with the same account.</li>
      </ul>`,
    },
    {
      target: '#shield',
      title: 'Ad & tracker blocker',
      body: `The green shield means ad and tracker blocking is on. It blocks known ad servers before they even
             load, so pages are lighter and faster. Add your own domains or turn it off in Settings.`,
    },
    {
      target: '#capture-btn',
      title: 'Screen capture & markup',
      body: `Captures what's on screen in the page (your game too) and opens a <b>markup editor</b>:
             pen, highlighter, lines, arrows, boxes, ellipses, <b>revision clouds</b>, text, <b>callouts</b>,
             numbered <b>count markers</b>, <b>redact</b> and <b>crop</b>, with a Markups list, undo/redo and zoom.
             Drawing <b>snaps</b> to corners, endpoints, midpoints and centres; ${K('Shift')} keeps lines horizontal or vertical.<br><br>
             ${K('Ctrl+Shift+X')} captures from anywhere in Respawn. <b>Copy</b> to paste it into Discord, or <b>Save</b>
             a PNG to Pictures\\Respawn.`,
    },
    {
      target: '#rail',
      title: 'Gaming sidebar',
      body: `One click opens <b>Chat</b> (Discord), <b>Live</b> (Twitch), <b>Video</b> (YouTube) or <b>Store</b> (Steam)
             in a side panel next to the page. Panels stay signed in and keep running when hidden. Click again to close.<br><br>
             With Discord, the panel lists your servers and channels and the conversation opens in one chat tab,
             with a built-in <b>DeepL translator</b> (add your key in Settings). <b>Right-click</b> a server, channel or any link →
             <b>Open in new tab</b> (${K('Shift')}+right-click shows the site's own menu).`,
    },
    {
      target: '#fullscreen-btn',
      title: 'Full-screen game view',
      body: `Hides every bar and fills your whole screen with the page — just the game.<br><br>
             Press ${K('F12')} (or this button) to go in and out — ${K('Esc')} also leaves full screen.
             A small badge at the top shows which account you're on; click it to switch accounts. Your tab keys still work in full screen, so you can
             flip between all your accounts. All shortcuts can be changed in Settings.`,
    },
    {
      target: '#gamemode',
      title: 'Game Mode',
      body: `Turn this on while you play a game on your PC: Respawn lowers its own CPU priority so your game
             gets the processor. ${K('Ctrl+Shift+G')} toggles it from anywhere, even in full-screen games.`,
    },
    {
      target: '#statusbar',
      title: 'Status bar',
      body: `<ul>
        <li><b>RAM</b>: how much memory Respawn is using compared with your limit.</li>
        <li><b>Tabs</b>: how many are open and how many are unloaded.</li>
        <li><b>Blocking</b>: how many ad/tracker domains are being blocked.</li>
      </ul>`,
    },
    {
      target: null,
      title: 'Keyboard shortcuts',
      body: () => {
        const k = (typeof state !== 'undefined' && state?.settings?.keybinds) || {};
        const row = (keys, what) => (keys ? `<tr><td>${K(keys)}</td><td>${what}</td></tr>` : '');
        return `<table class="tour-keys">
          ${row(k.prev_tab && k.next_tab ? `${k.prev_tab}</kbd> / <kbd>${k.next_tab}` : '', 'Previous / next tab — works inside pages and games, and in full screen')}
          ${row(k.fullscreen, 'Full-screen game view on / off')}
          ${row(k.game_mode, 'Game Mode (works in any program)')}
          ${row(k.capture, 'Screen capture & markup')}
          ${row(k.new_tab, 'New tab')}
          ${row(k.close_tab, 'Close tab')}
          ${row(k.reload, 'Reload')}
          ${row(k.address, 'Go to the address bar')}
          <tr><td>${K('Ctrl+Tab')}</td><td>Next tab (${K('Ctrl+Shift+Tab')} previous)</td></tr>
        </table>
        <p style="margin:8px 0 0">Change any of these in <b>Settings → Keyboard shortcuts</b>.</p>`;
      },
    },
    {
      target: null,
      title: 'Respawn has your back',
      body: `<ul>
        <li><b>Game crash recovery</b>: if a browser game crashes ("memory access out of bounds"), Respawn
            reloads it automatically instead of leaving you with an error box.</li>
        <li><b>Sign-in pop-ups</b> (Google, Facebook) open normally and stay with the tab's account.</li>
        <li><b>Saved passwords & autofill</b> are on, per account.</li>
      </ul>`,
    },
    {
      target: '#settings-btn',
      title: 'Settings',
      body: `Change your search engine, memory limits and automatic unloading, the ad blocker,
             per-tab logins, ${K('Q')}/${K('W')} keys, history and session restore.<br><br>
             You can replay this tour any time from the <b>?</b> button or Settings.`,
    },
    {
      target: null,
      title: "You're all set",
      body: `Have fun. Tip: right-click a game tab → <b>Rename tab…</b> to name your first account.`,
    },
  ];

  let i = 0;
  let root = null;

  function build() {
    root = document.createElement('div');
    root.id = 'tour';
    root.innerHTML = `
      <div class="tour-shade"></div>
      <div class="tour-spot"></div>
      <div class="tour-card" role="dialog" aria-live="polite">
        <div class="tour-step"></div>
        <h3 class="tour-title"></h3>
        <div class="tour-body"></div>
        <div class="tour-foot">
          <button class="tour-skip ghost">Skip tour</button>
          <span class="tour-dots"></span>
          <button class="tour-back ghost">Back</button>
          <button class="tour-next primary">Next</button>
        </div>
      </div>`;
    document.body.appendChild(root);
    root.querySelector('.tour-skip').onclick = end;
    root.querySelector('.tour-back').onclick = () => go(i - 1);
    root.querySelector('.tour-next').onclick = () => (i === STEPS.length - 1 ? end() : go(i + 1));
  }

  function place() {
    if (!root) return;
    const step = STEPS[i];
    const spot = root.querySelector('.tour-spot');
    const shade = root.querySelector('.tour-shade');
    const card = root.querySelector('.tour-card');
    const el = step.target && document.querySelector(step.target);
    const W = innerWidth, H = innerHeight, gap = 14;

    if (!el) {
      spot.style.display = 'none';
      shade.style.display = 'block';
      card.style.left = `${Math.max(12, (W - card.offsetWidth) / 2)}px`;
      card.style.top = `${Math.max(12, (H - card.offsetHeight) / 2)}px`;
      return;
    }
    shade.style.display = 'none';
    const r = el.getBoundingClientRect();
    const pad = 6;
    Object.assign(spot.style, {
      display: 'block',
      left: `${r.left - pad}px`, top: `${r.top - pad}px`,
      width: `${r.width + pad * 2}px`, height: `${r.height + pad * 2}px`,
    });
    // Card below the target if there's room, otherwise above, otherwise to the side.
    const cw = card.offsetWidth, ch = card.offsetHeight;
    let top = r.bottom + gap;
    let left = r.left + r.width / 2 - cw / 2;
    if (top + ch > H - 12) top = r.top - gap - ch;
    if (top < 12) { top = Math.min(H - ch - 12, Math.max(12, r.top)); left = r.right + gap; }
    if (left + cw > W - 12) left = W - cw - 12;
    card.style.left = `${Math.max(12, left)}px`;
    card.style.top = `${Math.max(12, top)}px`;
  }

  function go(n) {
    i = Math.max(0, Math.min(STEPS.length - 1, n));
    const step = STEPS[i];
    root.querySelector('.tour-step').textContent = `${i + 1} of ${STEPS.length}`;
    root.querySelector('.tour-title').textContent = step.title;
    root.querySelector('.tour-body').innerHTML = typeof step.body === 'function' ? step.body() : step.body;
    root.querySelector('.tour-back').disabled = i === 0;
    root.querySelector('.tour-next').textContent = i === STEPS.length - 1 ? 'Finish' : 'Next';
    root.querySelector('.tour-dots').innerHTML =
      STEPS.map((_, k) => `<i class="${k === i ? 'on' : ''}"></i>`).join('');
    requestAnimationFrame(place);
  }

  // Tour keys take priority over the browser's own shortcuts (Q/W, arrows…).
  function onKey(e) {
    if (!root) return;
    e.stopImmediatePropagation();
    if (e.key === 'ArrowRight' || e.key === 'Enter') { e.preventDefault(); i === STEPS.length - 1 ? end() : go(i + 1); }
    else if (e.key === 'ArrowLeft') { e.preventDefault(); go(i - 1); }
    else if (e.key === 'Escape') { e.preventDefault(); end(); }
  }

  function start() {
    if (root) return;
    // Hide web pages so the whole browser UI can be highlighted.
    invoke('set_overlay', { open: true });
    build();
    addEventListener('keydown', onKey, true);
    addEventListener('resize', place);
    go(0);
    root.querySelector('.tour-next').focus();
  }

  function end() {
    if (!root) return;
    root.remove();
    root = null;
    removeEventListener('keydown', onKey, true);
    removeEventListener('resize', place);
    try { localStorage.setItem('respawn.tourDone', '1'); } catch {}
    invoke('set_overlay', { open: false });
  }

  window.startTour = start;
  document.getElementById('help-btn')?.addEventListener('click', start);
  document.getElementById('tour-from-settings')?.addEventListener('click', () => {
    document.getElementById('settings-close')?.click();
    setTimeout(start, 50);
  });

  // First launch: wait for the tabs to render, then start.
  let seen = false;
  try { seen = localStorage.getItem('respawn.tourDone') === '1'; } catch {}
  if (!seen) {
    const wait = setInterval(() => {
      if (document.querySelector('#tabs .tab')) { clearInterval(wait); setTimeout(start, 400); }
    }, 200);
    setTimeout(() => clearInterval(wait), 15000);
  }
})();
