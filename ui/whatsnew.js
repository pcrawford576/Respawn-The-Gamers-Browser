// "What's new": after an update, shows the notes for every version since the
// last one you saw (from changelog.md). Also opens from Settings.
(() => {
  const $ = (s) => document.querySelector(s);
  const modal = $('#whatsnew');

  const cmp = (a, b) => {
    const pa = a.split('.').map(Number), pb = b.split('.').map(Number);
    for (let i = 0; i < 3; i++) if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) - (pb[i] || 0);
    return 0;
  };
  const esc = (s) => s.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));

  // "## 0.1.1 - 2026-11-08" sections → [{ version, date, items }]
  function parse(md) {
    const out = [];
    for (const block of md.split(/^## /m).slice(1)) {
      const [head, ...rest] = block.split('\n');
      const m = head.match(/^(\d+\.\d+\.\d+)\s*(?:-\s*(.+))?/);
      if (!m) continue; // skips "Unreleased"
      const items = rest.filter((l) => /^\s*[-*] /.test(l)).map((l) => l.replace(/^\s*[-*] /, '').trim());
      out.push({ version: m[1], date: (m[2] || '').trim(), items });
    }
    return out;
  }

  function show(sections, version) {
    $('#wn-title').textContent = sections.length === 1 ? `What's new in Respawn ${sections[0].version}` : "What's new in Respawn";
    $('#wn-version').textContent = `You're on version ${version}`;
    $('#wn-body').innerHTML = sections.map((s) => `
      <div class="wn-ver">${esc(s.version)}${s.date ? `<small>${esc(s.date)}</small>` : ''}</div>
      <ul>${s.items.map((i) => `<li>${esc(i)}</li>`).join('')}</ul>`).join('')
      || '<p class="muted">No notes for this version.</p>';
    modal.hidden = false;
    invoke('set_overlay', { open: true });
  }

  function close() {
    modal.hidden = true;
    invoke('set_overlay', { open: false });
    invoke('mark_version_seen').catch(() => {});
  }
  $('#wn-ok').onclick = close;
  $('#wn-close').onclick = close;
  modal.addEventListener('click', (e) => { if (e.target === modal) close(); });
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && !modal.hidden) close(); });

  async function load() {
    const info = await invoke('app_info');
    const md = await fetch('changelog.md').then((r) => r.text()).catch(() => '');
    return { info, all: parse(md) };
  }

  // From Settings: notes for the version you're running (and older ones below it).
  $('#whatsnew-from-settings')?.addEventListener('click', async () => {
    const { info, all } = await load();
    document.getElementById('settings-close')?.click();
    show(all.filter((s) => cmp(s.version, info.version) <= 0).slice(0, 5), info.version);
  });

  // On start: only right after an update.
  load().then(({ info, all }) => {
    const v = document.getElementById('app-version');
    if (v) v.textContent = `Respawn ${info.version}`;
    let tourDone = false;
    try { tourDone = localStorage.getItem('respawn.tourDone') === '1'; } catch {}
    if (!info.lastSeen) {
      // Brand-new install: the tour introduces everything; just remember this version.
      if (!tourDone) { invoke('mark_version_seen').catch(() => {}); return; }
    } else if (cmp(info.version, info.lastSeen) <= 0) {
      return; // nothing new
    }
    const fresh = all.filter((s) => cmp(s.version, info.version) <= 0 && (!info.lastSeen || cmp(s.version, info.lastSeen) > 0));
    if (fresh.length) setTimeout(() => show(fresh, info.version), 600);
    else invoke('mark_version_seen').catch(() => {});
  }).catch(() => {});
})();
