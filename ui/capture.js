// Respawn screen capture + markup editor (Bluebeam-style).
// Camera button or Ctrl+Shift+X captures the page area, then opens the editor.
(() => {
  const ICON = {
    select: 'M7 2l12 11.2-5.8.5 3.3 7.3-2.2 1-3.2-7.4L7 19z',
    pen: 'M3 17.25V21h3.75L17.8 9.94l-3.75-3.75zM20.7 7.04a1 1 0 0 0 0-1.41l-2.34-2.34a1 1 0 0 0-1.41 0l-1.83 1.83 3.75 3.75z',
    highlight: 'M6 14l3 3v5h6v-5l3-3V9H6zm5-12h2v3h-2zM3.5 5.88l1.41-1.42 2.13 2.12-1.42 1.42zm13.46.71 2.12-2.12 1.42 1.41-2.13 2.13z',
    line: 'M4 19 19 4l1 1L5 20z',
    arrow: 'M5 19 16.6 7.4V13h2V4h-9v2h5.6L4 17.6z',
    rect: 'M3 5v14h18V5zm16 12H5V7h14z',
    ellipse: 'M12 5C6.5 5 2 8.1 2 12s4.5 7 10 7 10-3.1 10-7-4.5-7-10-7zm0 12c-4.4 0-8-2.2-8-5s3.6-5 8-5 8 2.2 8 5-3.6 5-8 5z',
    cloud: 'M19.35 10.04A7.49 7.49 0 0 0 12 4C9.11 4 6.6 5.64 5.35 8.04A5.99 5.99 0 0 0 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM19 18H6c-2.21 0-4-1.79-4-4s1.79-4 4-4h.71C7.37 7.69 9.48 6 12 6a5.5 5.5 0 0 1 5.5 5.5v.5H19a3 3 0 0 1 0 6z',
    text: 'M5 4v3h5.5v12h3V7H19V4z',
    callout: 'M20 2H4a2 2 0 0 0-2 2v18l4-4h14a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2zm0 14H5.17L4 17.17V4h16z',
    count: 'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 15h-2V9H9V7h4z',
    blur: 'M3 3h4v4H3zm7 0h4v4h-4zm7 0h4v4h-4zM3 10h4v4H3zm7 0h4v4h-4zm7 0h4v4h-4zM3 17h4v4H3zm7 0h4v4h-4zm7 0h4v4h-4z',
    crop: 'M17 15h2V7a2 2 0 0 0-2-2H9v2h8zM7 17V1H5v4H1v2h4v10a2 2 0 0 0 2 2h10v4h2v-4h4v-2z',
  };
  const TOOLS = [
    ['select', 'Select & move', 'V'], ['pen', 'Pen', 'P'], ['highlight', 'Highlighter', 'H'],
    ['line', 'Line', 'L'], ['arrow', 'Arrow', 'A'], ['rect', 'Rectangle', 'R'], ['ellipse', 'Ellipse', 'E'],
    ['cloud', 'Revision cloud', 'C'], ['text', 'Text box', 'T'], ['callout', 'Callout', 'K'],
    ['count', 'Count marker', 'N'], ['blur', 'Redact (pixelate)', 'B'], ['crop', 'Crop', 'X'],
  ];
  const NAMES = Object.fromEntries(TOOLS.map(([k, n]) => [k, n]));
  const COLORS = ['#ff3b30', '#ff9500', '#ffd60a', '#34c759', '#3df5a7', '#0a84ff', '#a46bff', '#ffffff', '#111111'];
  const svg = (k) => `<svg viewBox="0 0 24 24"><path d="${ICON[k]}"/></svg>`;

  let ed = null;

  // ------------------------------------------------------------ capture

  async function startCapture() {
    if (ed) return;
    if (document.querySelector('.modal:not([hidden])') || document.getElementById('tour')) return;
    let buf;
    try {
      buf = await invoke('capture_page');
    } catch (e) {
      flash(`Capture failed: ${e}`);
      return;
    }
    const bytes = new Uint8Array(buf);
    const dv = new DataView(bytes.buffer, bytes.byteOffset, 8);
    const w = dv.getUint32(0, true), h = dv.getUint32(4, true);
    const img = new ImageData(new Uint8ClampedArray(bytes.buffer, bytes.byteOffset + 8, w * h * 4), w, h);
    open(img);
  }

  function flash(msg) {
    const t = document.getElementById('toast');
    if (!t) return;
    t.textContent = msg;
    t.hidden = false;
    clearTimeout(flash.t);
    flash.t = setTimeout(() => (t.hidden = true), 2800);
  }

  // ------------------------------------------------------------- editor

  function open(imageData) {
    const src = document.createElement('canvas');
    src.width = imageData.width;
    src.height = imageData.height;
    src.getContext('2d').putImageData(imageData, 0, 0);

    const root = document.createElement('div');
    root.id = 'capture';
    root.innerHTML = `
      <div class="cap-bar">
        <div class="cap-group cap-tools">
          ${TOOLS.map(([k, n, key]) => `<button class="cap-btn" data-tool="${k}" title="${n} (${key})">${svg(k)}</button>`).join('')}
        </div>
        <div class="cap-sep"></div>
        <div class="cap-group cap-colors">
          ${COLORS.map((c) => `<button class="cap-swatch" data-color="${c}" style="--c:${c}" title="${c}"></button>`).join('')}
        </div>
        <div class="cap-sep"></div>
        <label class="cap-prop" title="Line width">
          <svg viewBox="0 0 24 24"><path d="M3 17h18v-2H3zm0 3h18v-1H3zm0-7h18v-3H3zm0-9v4h18V4z"/></svg>
          <select data-prop="width">${[1, 2, 3, 4, 6, 8, 12, 16].map((v) => `<option value="${v}">${v}px</option>`).join('')}</select>
        </label>
        <label class="cap-prop" title="Text size">
          <svg viewBox="0 0 24 24"><path d="M9 4v3h5v12h3V7h5V4zm-6 8h3v7h3v-7h3V9H3z"/></svg>
          <select data-prop="fontSize">${[14, 18, 24, 32, 40, 56].map((v) => `<option value="${v}">${v}</option>`).join('')}</select>
        </label>
        <button class="cap-btn cap-toggle" data-prop="fill" title="Fill shapes">
          <svg viewBox="0 0 24 24"><path d="M16.56 8.94 7.62 0 6.21 1.41l2.38 2.38-5.15 5.15a1.49 1.49 0 0 0 0 2.12l5.5 5.5c.29.29.68.44 1.06.44s.77-.15 1.06-.44l5.5-5.5c.59-.58.59-1.53 0-2.12zM5.21 10 10 5.21 14.79 10zM19 11.5s-2 2.17-2 3.5a2 2 0 0 0 4 0c0-1.33-2-3.5-2-3.5z"/></svg>
        </button>
        <div class="cap-sep"></div>
        <button class="cap-btn on" data-act="snap" title="Snap to endpoints, corners, midpoints &amp; centres (S). Hold Alt to place freely. Shift locks lines horizontal / vertical.">
          <svg viewBox="0 0 24 24"><path d="M3 7v6a9 9 0 0 0 18 0V7h-5v6a4 4 0 0 1-8 0V7zm2 2h1v2H5zm13 0h1v2h-1z"/></svg>
        </button>
        <div class="cap-sep"></div>
        <button class="cap-btn" data-act="undo" title="Undo (Ctrl+Z)"><svg viewBox="0 0 24 24"><path d="M12.5 8c-2.65 0-5.05.99-6.9 2.6L2 7v9h9l-3.62-3.62A7.95 7.95 0 0 1 20.46 16l2.37-.78A10.5 10.5 0 0 0 12.5 8z"/></svg></button>
        <button class="cap-btn" data-act="redo" title="Redo (Ctrl+Y)"><svg viewBox="0 0 24 24"><path d="M18.4 10.6A10.46 10.46 0 0 0 11.5 8C6.85 8 2.92 11.03 1.54 15.22L3.9 16a7.98 7.98 0 0 1 12.72-3.62L13 16h9V7z"/></svg></button>
        <button class="cap-btn" data-act="zoomout" title="Zoom out (Ctrl −)"><svg viewBox="0 0 24 24"><path d="M19 13H5v-2h14z"/></svg></button>
        <button class="cap-btn cap-fit" data-act="fit" title="Fit to window (Ctrl 0)">Fit</button>
        <button class="cap-btn" data-act="zoomin" title="Zoom in (Ctrl +)"><svg viewBox="0 0 24 24"><path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6z"/></svg></button>
        <div class="cap-spacer"></div>
        <span class="cap-status"></span>
        <button class="cap-action" data-act="copy" title="Copy image (Ctrl+C)">Copy</button>
        <button class="cap-action primary" data-act="save" title="Save PNG to Pictures\\Respawn (Ctrl+S)">Save</button>
        <button class="cap-btn" data-act="close" title="Close (Esc)"><svg viewBox="0 0 24 24"><path d="M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg></button>
      </div>
      <div class="cap-main">
        <div class="cap-stage"><canvas></canvas></div>
        <aside class="cap-list">
          <div class="cap-list-head">Markups <span class="cap-count">0</span></div>
          <div class="cap-items"></div>
          <div class="cap-hint">Click a markup to select it. <kbd>Del</kbd> deletes. Double-click text to edit.<br>
            <kbd>Shift</kbd> locks lines and moves horizontal / vertical. <kbd>S</kbd> snapping on/off, hold <kbd>Alt</kbd> to place freely.</div>
        </aside>
      </div>`;
    document.body.appendChild(root);
    invoke('set_overlay', { open: true });

    ed = {
      root, src, W: src.width, H: src.height,
      canvas: root.querySelector('canvas'), stage: root.querySelector('.cap-stage'),
      shapes: [], crop: null, sel: null, nextId: 1, countN: 1,
      tool: 'arrow', color: '#ff3b30', hlColor: '#ffd60a', width: 4, fontSize: 24, fill: false,
      undo: [], redo: [], zoom: 1, view: { s: 1, ox: 0, oy: 0 },
      drag: null, editing: null, dirty: false, closeArmed: 0, snap: true, hint: null,
    };

    root.querySelectorAll('[data-tool]').forEach((b) => (b.onclick = () => setTool(b.dataset.tool)));
    root.querySelectorAll('[data-color]').forEach((b) => (b.onclick = () => setProp('color', b.dataset.color)));
    root.querySelectorAll('select[data-prop]').forEach((s) => (s.onchange = () => setProp(s.dataset.prop, Number(s.value))));
    root.querySelector('[data-prop="fill"]').onclick = () => setProp('fill', !ed.fill);
    root.querySelectorAll('[data-act]').forEach((b) => (b.onclick = () => action(b.dataset.act)));

    const c = ed.canvas;
    c.addEventListener('pointerdown', onDown);
    // Keep keyboard focus where it is (e.g. a text box just opened by a click).
    c.addEventListener('mousedown', (e) => e.preventDefault());
    c.addEventListener('pointermove', onMove);
    c.addEventListener('pointerup', onUp);
    c.addEventListener('dblclick', onDblClick);
    c.addEventListener('wheel', onWheel, { passive: false });
    addEventListener('keydown', onKey, true);
    addEventListener('resize', layout);

    setTool('arrow');
    syncProps();
    layout();
    status(`${ed.W} × ${ed.H}`);
  }

  function close() {
    if (!ed) return;
    commitText();
    removeEventListener('keydown', onKey, true);
    removeEventListener('resize', layout);
    ed.root.remove();
    ed = null;
    invoke('set_overlay', { open: false });
  }

  // -------------------------------------------------------------- view

  const area = () => ed.crop || { x: 0, y: 0, w: ed.W, h: ed.H };

  function layout() {
    if (!ed) return;
    const r = ed.stage.getBoundingClientRect();
    const dpr = devicePixelRatio || 1;
    ed.canvas.width = Math.round(r.width * dpr);
    ed.canvas.height = Math.round(r.height * dpr);
    ed.canvas.style.width = `${r.width}px`;
    ed.canvas.style.height = `${r.height}px`;
    const a = area();
    const fit = Math.min((r.width - 40) / a.w, (r.height - 40) / a.h, 1);
    const s = fit * ed.zoom;
    ed.view = { s, ox: (r.width - a.w * s) / 2 - a.x * s, oy: (r.height - a.h * s) / 2 - a.y * s };
    render();
  }

  function toImg(e) {
    const r = ed.canvas.getBoundingClientRect();
    return { x: (e.clientX - r.left - ed.view.ox) / ed.view.s, y: (e.clientY - r.top - ed.view.oy) / ed.view.s };
  }

  function toScreen(x, y) {
    const r = ed.canvas.getBoundingClientRect();
    return { x: r.left + ed.view.ox + x * ed.view.s, y: r.top + ed.view.oy + y * ed.view.s };
  }

  // ------------------------------------------------------------ drawing

  function render() {
    const { canvas, view } = ed;
    const ctx = canvas.getContext('2d');
    const dpr = devicePixelRatio || 1;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.setTransform(dpr * view.s, 0, 0, dpr * view.s, dpr * view.ox, dpr * view.oy);

    const a = area();
    ctx.save();
    ctx.beginPath();
    ctx.rect(a.x, a.y, a.w, a.h);
    ctx.clip();
    ctx.drawImage(ed.src, 0, 0);
    for (const sh of ed.shapes) if (!sh.hidden) drawShape(ctx, sh);
    if (ed.drag?.preview) drawShape(ctx, ed.drag.preview);
    ctx.restore();

    // Page edge
    ctx.strokeStyle = 'rgba(255,255,255,.18)';
    ctx.lineWidth = 1 / view.s;
    ctx.strokeRect(a.x, a.y, a.w, a.h);

    // Crop rectangle being dragged
    if (ed.drag?.crop) {
      const r = norm(ed.drag.crop);
      ctx.save();
      ctx.fillStyle = 'rgba(0,0,0,.45)';
      ctx.beginPath();
      ctx.rect(a.x, a.y, a.w, a.h);
      ctx.rect(r.x, r.y, r.w, r.h);
      ctx.fill('evenodd');
      ctx.setLineDash([6 / view.s, 4 / view.s]);
      ctx.strokeStyle = '#3df5a7';
      ctx.lineWidth = 1.5 / view.s;
      ctx.strokeRect(r.x, r.y, r.w, r.h);
      ctx.restore();
    }

    drawHint(ctx);

    // Selection outline
    const sel = ed.shapes.find((s) => s.id === ed.sel);
    if (sel) {
      const b = bbox(sel);
      const p = 6 / view.s;
      ctx.save();
      ctx.setLineDash([5 / view.s, 4 / view.s]);
      ctx.strokeStyle = '#3df5a7';
      ctx.lineWidth = 1.5 / view.s;
      ctx.strokeRect(b.x - p, b.y - p, b.w + p * 2, b.h + p * 2);
      ctx.restore();
    }
    renderList();
  }

  const norm = (r) => ({
    x: Math.min(r.x1, r.x2), y: Math.min(r.y1, r.y2), w: Math.abs(r.x2 - r.x1), h: Math.abs(r.y2 - r.y1),
  });

  function rgba(hex, a) {
    const n = parseInt(hex.slice(1), 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
  }

  function arrowHead(ctx, x1, y1, x2, y2, size) {
    const ang = Math.atan2(y2 - y1, x2 - x1);
    ctx.beginPath();
    ctx.moveTo(x2, y2);
    ctx.lineTo(x2 - size * Math.cos(ang - 0.45), y2 - size * Math.sin(ang - 0.45));
    ctx.lineTo(x2 - size * Math.cos(ang + 0.45), y2 - size * Math.sin(ang + 0.45));
    ctx.closePath();
    ctx.fill();
  }

  function textLines(sh) { return (sh.text || '').split('\n'); }

  function textBox(ctx, sh) {
    ctx.font = `600 ${sh.fontSize}px "Segoe UI", system-ui, sans-serif`;
    const lines = textLines(sh);
    const w = Math.max(sh.fontSize * 2, ...lines.map((l) => ctx.measureText(l || ' ').width));
    const lh = sh.fontSize * 1.25;
    const pad = sh.fontSize * 0.4;
    return { w: w + pad * 2, h: lines.length * lh + pad * 2, lh, pad, lines };
  }

  function calloutBox(ctx, sh) {
    const tb = textBox(ctx, sh);
    return { x: sh.x2, y: sh.y2, ...tb };
  }

  function drawShape(ctx, sh) {
    ctx.save();
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.strokeStyle = sh.color;
    ctx.fillStyle = sh.color;
    ctx.lineWidth = sh.width;
    switch (sh.type) {
      case 'pen':
      case 'highlight': {
        if (sh.type === 'highlight') {
          ctx.globalAlpha = 0.38;
          ctx.globalCompositeOperation = 'multiply';
          ctx.lineCap = 'square';
        }
        ctx.beginPath();
        sh.pts.forEach((p, i) => (i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y)));
        if (sh.pts.length === 1) ctx.lineTo(sh.pts[0].x + 0.1, sh.pts[0].y);
        ctx.stroke();
        break;
      }
      case 'line':
      case 'arrow':
        ctx.beginPath();
        ctx.moveTo(sh.x1, sh.y1);
        ctx.lineTo(sh.x2, sh.y2);
        ctx.stroke();
        if (sh.type === 'arrow') arrowHead(ctx, sh.x1, sh.y1, sh.x2, sh.y2, Math.max(12, sh.width * 4));
        break;
      case 'rect': {
        const r = norm(sh);
        if (sh.fill) { ctx.fillStyle = rgba(sh.color, 0.22); ctx.fillRect(r.x, r.y, r.w, r.h); }
        ctx.strokeRect(r.x, r.y, r.w, r.h);
        break;
      }
      case 'ellipse': {
        const r = norm(sh);
        ctx.beginPath();
        ctx.ellipse(r.x + r.w / 2, r.y + r.h / 2, r.w / 2, r.h / 2, 0, 0, Math.PI * 2);
        if (sh.fill) { ctx.fillStyle = rgba(sh.color, 0.22); ctx.fill(); }
        ctx.stroke();
        break;
      }
      case 'cloud': {
        // Revision cloud: scallops all the way round the rectangle.
        const r = norm(sh);
        const bump = Math.max(10, sh.width * 4);
        const corners = [[r.x, r.y], [r.x + r.w, r.y], [r.x + r.w, r.y + r.h], [r.x, r.y + r.h]];
        const out = [[0, -1], [1, 0], [0, 1], [-1, 0]];
        ctx.beginPath();
        ctx.moveTo(r.x, r.y);
        for (let e = 0; e < 4; e++) {
          const [ax, ay] = corners[e];
          const [bx, by] = corners[(e + 1) % 4];
          const len = Math.hypot(bx - ax, by - ay);
          const n = Math.max(1, Math.round(len / (bump * 1.6)));
          for (let k = 0; k < n; k++) {
            const t0 = k / n, t1 = (k + 1) / n;
            const px = ax + (bx - ax) * t1, py = ay + (by - ay) * t1;
            const mx = ax + (bx - ax) * (t0 + t1) / 2 + out[e][0] * bump;
            const my = ay + (by - ay) * (t0 + t1) / 2 + out[e][1] * bump;
            ctx.quadraticCurveTo(mx, my, px, py);
          }
        }
        ctx.closePath();
        if (sh.fill) { ctx.fillStyle = rgba(sh.color, 0.18); ctx.fill(); }
        ctx.stroke();
        break;
      }
      case 'text': {
        const tb = textBox(ctx, sh);
        if (sh.fill) {
          ctx.fillStyle = 'rgba(255,255,255,.92)';
          ctx.fillRect(sh.x, sh.y, tb.w, tb.h);
          ctx.lineWidth = Math.max(1, sh.width / 2);
          ctx.strokeRect(sh.x, sh.y, tb.w, tb.h);
        }
        ctx.fillStyle = sh.color;
        ctx.textBaseline = 'top';
        tb.lines.forEach((l, i) => ctx.fillText(l, sh.x + tb.pad, sh.y + tb.pad + i * tb.lh));
        break;
      }
      case 'callout': {
        const b = calloutBox(ctx, sh);
        // Leader from the tip to the nearest side of the box.
        const cx = Math.max(b.x, Math.min(sh.x1, b.x + b.w));
        const cy = Math.max(b.y, Math.min(sh.y1, b.y + b.h));
        const inside = sh.x1 >= b.x && sh.x1 <= b.x + b.w && sh.y1 >= b.y && sh.y1 <= b.y + b.h;
        if (!inside) {
          ctx.beginPath();
          ctx.moveTo(cx, cy);
          ctx.lineTo(sh.x1, sh.y1);
          ctx.stroke();
          arrowHead(ctx, cx, cy, sh.x1, sh.y1, Math.max(12, sh.width * 4));
        }
        ctx.fillStyle = 'rgba(255,255,255,.95)';
        ctx.fillRect(b.x, b.y, b.w, b.h);
        ctx.lineWidth = Math.max(1.5, sh.width / 2);
        ctx.strokeRect(b.x, b.y, b.w, b.h);
        ctx.fillStyle = '#111';
        ctx.textBaseline = 'top';
        b.lines.forEach((l, i) => ctx.fillText(l, b.x + b.pad, b.y + b.pad + i * b.lh));
        break;
      }
      case 'count': {
        const rad = Math.max(12, sh.fontSize * 0.62);
        ctx.beginPath();
        ctx.arc(sh.x, sh.y, rad, 0, Math.PI * 2);
        ctx.fill();
        ctx.lineWidth = 2;
        ctx.strokeStyle = 'rgba(255,255,255,.9)';
        ctx.stroke();
        ctx.fillStyle = isLight(sh.color) ? '#111' : '#fff';
        ctx.font = `700 ${Math.round(rad * 1.1)}px "Segoe UI", system-ui, sans-serif`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(String(sh.n), sh.x, sh.y + 1);
        break;
      }
      case 'blur': {
        // Pixelate what's underneath (from the original capture).
        const r = norm(sh);
        if (r.w >= 1 && r.h >= 1) {
          const block = Math.max(14, Math.round(Math.min(r.w, r.h) / 4)); // big blocks: text can't be read
          const tw = Math.max(1, Math.round(r.w / block)), th = Math.max(1, Math.round(r.h / block));
          const tmp = document.createElement('canvas');
          tmp.width = tw; tmp.height = th;
          tmp.getContext('2d').drawImage(ed.src, r.x, r.y, r.w, r.h, 0, 0, tw, th);
          ctx.imageSmoothingEnabled = false;
          ctx.drawImage(tmp, 0, 0, tw, th, r.x, r.y, r.w, r.h);
        }
        break;
      }
    }
    ctx.restore();
  }

  function isLight(hex) {
    const n = parseInt(hex.slice(1), 16);
    return ((n >> 16) & 255) * 0.3 + ((n >> 8) & 255) * 0.59 + (n & 255) * 0.11 > 170;
  }

  // ------------------------------------------------------- hit testing

  const measure = document.createElement('canvas').getContext('2d');

  function bbox(sh) {
    switch (sh.type) {
      case 'pen': case 'highlight': {
        const xs = sh.pts.map((p) => p.x), ys = sh.pts.map((p) => p.y);
        const h = sh.width / 2;
        return { x: Math.min(...xs) - h, y: Math.min(...ys) - h, w: Math.max(...xs) - Math.min(...xs) + sh.width, h: Math.max(...ys) - Math.min(...ys) + sh.width };
      }
      case 'text': { const t = textBox(measure, sh); return { x: sh.x, y: sh.y, w: t.w, h: t.h }; }
      case 'callout': {
        const b = calloutBox(measure, sh);
        const x = Math.min(b.x, sh.x1), y = Math.min(b.y, sh.y1);
        return { x, y, w: Math.max(b.x + b.w, sh.x1) - x, h: Math.max(b.y + b.h, sh.y1) - y };
      }
      case 'count': { const r = Math.max(12, sh.fontSize * 0.62); return { x: sh.x - r, y: sh.y - r, w: r * 2, h: r * 2 }; }
      default: return norm(sh);
    }
  }

  function distSeg(p, a, b) {
    const dx = b.x - a.x, dy = b.y - a.y;
    const l = dx * dx + dy * dy;
    const t = l ? Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / l)) : 0;
    return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
  }

  function hit(p) {
    const tol = 8 / ed.view.s;
    for (let i = ed.shapes.length - 1; i >= 0; i--) {
      const sh = ed.shapes[i];
      if (sh.type === 'line' || sh.type === 'arrow') {
        if (distSeg(p, { x: sh.x1, y: sh.y1 }, { x: sh.x2, y: sh.y2 }) <= tol + sh.width / 2) return sh;
      } else if (sh.type === 'pen' || sh.type === 'highlight') {
        for (let k = 1; k < sh.pts.length; k++) {
          if (distSeg(p, sh.pts[k - 1], sh.pts[k]) <= tol + sh.width / 2) return sh;
        }
        if (sh.pts.length === 1 && Math.hypot(p.x - sh.pts[0].x, p.y - sh.pts[0].y) <= tol + sh.width) return sh;
      } else {
        const b = bbox(sh);
        if (p.x >= b.x - tol && p.x <= b.x + b.w + tol && p.y >= b.y - tol && p.y <= b.y + b.h + tol) return sh;
      }
    }
    return null;
  }

  function moveShape(sh, dx, dy) {
    if (sh.pts) sh.pts.forEach((p) => { p.x += dx; p.y += dy; });
    if ('x1' in sh) { sh.x1 += dx; sh.y1 += dy; sh.x2 += dx; sh.y2 += dy; }
    if ('x' in sh) { sh.x += dx; sh.y += dy; }
  }

  // --------------------------------------------------------- undo/redo

  const snapshot = () => JSON.stringify({ shapes: ed.shapes, crop: ed.crop, countN: ed.countN });

  function pushUndo() {
    ed.undo.push(snapshot());
    if (ed.undo.length > 200) ed.undo.shift();
    ed.redo = [];
    ed.dirty = true;
  }

  function restore(json) {
    const s = JSON.parse(json);
    ed.shapes = s.shapes;
    ed.crop = s.crop;
    ed.countN = s.countN;
    if (!ed.shapes.some((x) => x.id === ed.sel)) ed.sel = null;
    layout();
  }

  function undo() { if (ed.undo.length) { ed.redo.push(snapshot()); restore(ed.undo.pop()); } }
  function redo() { if (ed.redo.length) { ed.undo.push(snapshot()); restore(ed.redo.pop()); } }

  // ---------------------------------------------------- tools & props

  function setTool(t) {
    commitText();
    ed.tool = t;
    if (t !== 'select') ed.sel = null;
    ed.root.querySelectorAll('[data-tool]').forEach((b) => b.classList.toggle('on', b.dataset.tool === t));
    ed.canvas.style.cursor = t === 'select' ? 'default' : t === 'text' ? 'text' : 'crosshair';
    syncProps();
    status(`${NAMES[t]}`);
    render();
  }

  function setProp(k, v) {
    if (k === 'color' && ed.tool === 'highlight' && !ed.sel) ed.hlColor = v;
    else ed[k] = v;
    const sel = ed.shapes.find((s) => s.id === ed.sel);
    if (sel && k in sel) {
      pushUndo();
      sel[k] = v;
    }
    syncProps();
    render();
  }

  function syncProps() {
    const r = ed.root;
    const shown = ed.tool === 'highlight' && !ed.sel ? ed.hlColor : ed.color;
    r.querySelectorAll('[data-color]').forEach((b) => b.classList.toggle('on', b.dataset.color === shown));
    r.querySelector('select[data-prop="width"]').value = String(ed.width);
    r.querySelector('select[data-prop="fontSize"]').value = String(ed.fontSize);
    r.querySelector('[data-prop="fill"]').classList.toggle('on', ed.fill);
  }

  function select(id) {
    ed.sel = id;
    const sh = ed.shapes.find((s) => s.id === id);
    if (sh) {
      // Show the selected markup's settings in the toolbar.
      ['color', 'width', 'fontSize', 'fill'].forEach((k) => { if (k in sh) ed[k] = sh[k]; });
      syncProps();
    }
    render();
  }

  function base(type) {
    return {
      id: ed.nextId++, type, color: type === 'highlight' ? ed.hlColor : ed.color,
      width: type === 'highlight' ? Math.max(14, ed.width * 4) : ed.width,
      fontSize: ed.fontSize, fill: ed.fill,
    };
  }

  // ---------------------------------------------------------- snapping
  // CAD-style object snaps: endpoints, corners, midpoints, centres, quadrants
  // and nearest-on-edge, from your markups and the capture's edges.
  // Hold Alt to place freely; S turns snapping on/off.

  function snapTargets(exclude) {
    const pts = [], segs = [];
    const add = (x, y, kind) => pts.push({ x, y, kind });
    const box = (r, kind, mids) => {
      const cs = [[r.x, r.y], [r.x + r.w, r.y], [r.x + r.w, r.y + r.h], [r.x, r.y + r.h]];
      cs.forEach(([x, y]) => add(x, y, kind));
      for (let i = 0; i < 4; i++) {
        const [ax, ay] = cs[i], [bx, by] = cs[(i + 1) % 4];
        if (mids) add((ax + bx) / 2, (ay + by) / 2, 'Midpoint');
        segs.push([{ x: ax, y: ay }, { x: bx, y: by }]);
      }
    };
    const a = area();
    box(a, 'Corner', true);
    add(a.x + a.w / 2, a.y + a.h / 2, 'Center');
    for (const sh of ed.shapes) {
      if (sh === exclude || sh.hidden) continue;
      switch (sh.type) {
        case 'line': case 'arrow':
          add(sh.x1, sh.y1, 'Endpoint');
          add(sh.x2, sh.y2, 'Endpoint');
          add((sh.x1 + sh.x2) / 2, (sh.y1 + sh.y2) / 2, 'Midpoint');
          segs.push([{ x: sh.x1, y: sh.y1 }, { x: sh.x2, y: sh.y2 }]);
          break;
        case 'pen': case 'highlight': {
          const f = sh.pts[0], l = sh.pts[sh.pts.length - 1];
          add(f.x, f.y, 'Endpoint');
          add(l.x, l.y, 'Endpoint');
          break;
        }
        case 'rect': case 'cloud': case 'blur': {
          const r = norm(sh);
          box(r, 'Corner', true);
          add(r.x + r.w / 2, r.y + r.h / 2, 'Center');
          break;
        }
        case 'ellipse': {
          const r = norm(sh);
          add(r.x + r.w / 2, r.y, 'Quadrant'); add(r.x + r.w, r.y + r.h / 2, 'Quadrant');
          add(r.x + r.w / 2, r.y + r.h, 'Quadrant'); add(r.x, r.y + r.h / 2, 'Quadrant');
          add(r.x + r.w / 2, r.y + r.h / 2, 'Center');
          break;
        }
        case 'count': add(sh.x, sh.y, 'Center'); break;
        case 'text': box(bbox(sh), 'Corner', false); break;
        case 'callout': {
          add(sh.x1, sh.y1, 'Endpoint');
          const b = calloutBox(measure, sh);
          box({ x: b.x, y: b.y, w: b.w, h: b.h }, 'Corner', true);
          break;
        }
      }
    }
    return { pts, segs };
  }

  /** Nearest snap within reach of p, or null. */
  function findSnap(p, t) {
    const tol = 11 / ed.view.s;
    let best = null, bd = tol;
    for (const q of t.pts) {
      const d = Math.hypot(q.x - p.x, q.y - p.y);
      if (d <= bd) { bd = d; best = { x: q.x, y: q.y, kind: q.kind }; }
    }
    if (best) return best;
    // Nothing exact nearby: slide onto the closest edge.
    for (const [a, b] of t.segs) {
      const dx = b.x - a.x, dy = b.y - a.y, l = dx * dx + dy * dy;
      const k = l ? Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / l)) : 0;
      const q = { x: a.x + k * dx, y: a.y + k * dy };
      const d = Math.hypot(q.x - p.x, q.y - p.y);
      if (d <= bd) { bd = d; best = { ...q, kind: 'Nearest' }; }
    }
    return best;
  }

  /**
   * Shift held: lock to horizontal or vertical from `from`, then line up with
   * a snap point along that axis (dotted guide shows what it lined up with).
   */
  function orthoSnap(p, from, t, useSnap) {
    const horiz = Math.abs(p.x - from.x) >= Math.abs(p.y - from.y);
    const q = horiz ? { x: p.x, y: from.y } : { x: from.x, y: p.y };
    if (!useSnap) return { p: q, hint: null };
    const tol = 11 / ed.view.s;
    let best = null, bd = tol;
    for (const s of t.pts) {
      const d = horiz ? Math.abs(s.x - q.x) : Math.abs(s.y - q.y);
      if (d <= bd) { bd = d; best = s; }
    }
    if (!best) return { p: q, hint: null };
    const r = horiz ? { x: best.x, y: from.y } : { x: from.x, y: best.y };
    return { p: r, hint: { x: r.x, y: r.y, kind: horiz ? 'Vertical alignment' : 'Horizontal alignment', from: best } };
  }

  const snapOn = (e) => ed.snap && !e.altKey;

  /** Where the pointer lands, with snapping applied (also sets the on-screen hint). */
  function snapped(e, exclude) {
    const p = toImg(e);
    if (!snapOn(e)) { ed.hint = null; return p; }
    const hit = findSnap(p, snapTargets(exclude));
    ed.hint = hit;
    return hit ? { x: hit.x, y: hit.y } : p;
  }

  function drawHint(ctx) {
    const h = ed.hint;
    if (!h) return;
    const s = 1 / ed.view.s;
    ctx.save();
    if (h.from) {
      // Alignment guide from the point it lined up with.
      ctx.setLineDash([4 * s, 4 * s]);
      ctx.strokeStyle = 'rgba(61,245,167,.8)';
      ctx.lineWidth = 1 * s;
      ctx.beginPath();
      ctx.moveTo(h.from.x, h.from.y);
      ctx.lineTo(h.x, h.y);
      ctx.stroke();
      ctx.setLineDash([]);
    }
    ctx.lineWidth = 2 * s;
    ctx.strokeStyle = '#3df5a7';
    ctx.fillStyle = 'rgba(10,11,16,.55)';
    const r = 7 * s;
    ctx.beginPath();
    switch (h.kind) {
      case 'Midpoint':
        ctx.moveTo(h.x, h.y - r); ctx.lineTo(h.x + r, h.y + r * 0.8); ctx.lineTo(h.x - r, h.y + r * 0.8); ctx.closePath();
        break;
      case 'Center':
        ctx.arc(h.x, h.y, r, 0, Math.PI * 2);
        break;
      case 'Quadrant':
        ctx.moveTo(h.x, h.y - r); ctx.lineTo(h.x + r, h.y); ctx.lineTo(h.x, h.y + r); ctx.lineTo(h.x - r, h.y); ctx.closePath();
        break;
      case 'Nearest':
        ctx.moveTo(h.x - r, h.y - r); ctx.lineTo(h.x + r, h.y - r); ctx.lineTo(h.x - r, h.y + r); ctx.lineTo(h.x + r, h.y + r); ctx.closePath();
        break;
      default: // Endpoint / Corner / alignment
        ctx.rect(h.x - r, h.y - r, r * 2, r * 2);
    }
    ctx.fill();
    ctx.stroke();
    // Label
    ctx.font = `600 ${11 * s}px "Segoe UI", system-ui, sans-serif`;
    const tw = ctx.measureText(h.kind).width;
    ctx.fillStyle = 'rgba(10,11,16,.85)';
    ctx.fillRect(h.x + r + 4 * s, h.y + r + 2 * s, tw + 10 * s, 17 * s);
    ctx.fillStyle = '#3df5a7';
    ctx.textBaseline = 'top';
    ctx.fillText(h.kind, h.x + r + 9 * s, h.y + r + 5 * s);
    ctx.restore();
  }

  function toggleSnap() {
    ed.snap = !ed.snap;
    ed.hint = null;
    ed.root.querySelector('[data-act="snap"]').classList.toggle('on', ed.snap);
    status(ed.snap ? 'Snapping on — hold Alt to place freely' : 'Snapping off');
    render();
  }

  // ------------------------------------------------------------ mouse

  function onDown(e) {
    if (e.button !== 0) return;
    commitText();
    ed.canvas.setPointerCapture(e.pointerId);
    const t = ed.tool;
    if (t === 'select') {
      const p = toImg(e);
      const sh = hit(p);
      select(sh ? sh.id : null);
      if (sh) ed.drag = { mode: 'move', start: p, applied: { x: 0, y: 0 }, moved: false, sh };
      return;
    }
    // Start point snaps (not for freehand pen strokes).
    const p = t === 'pen' || t === 'highlight' ? toImg(e) : snapped(e);
    if (t === 'pen' || t === 'highlight') {
      ed.drag = { preview: { ...base(t), pts: [p] } };
    } else if (t === 'crop') {
      ed.drag = { crop: { x1: p.x, y1: p.y, x2: p.x, y2: p.y } };
    } else if (t === 'text') {
      pushUndo();
      const sh = { ...base('text'), x: p.x, y: p.y, text: '' };
      ed.shapes.push(sh);
      select(sh.id);
      editText(sh, true);
      return;
    } else if (t === 'count') {
      pushUndo();
      const sh = { ...base('count'), x: p.x, y: p.y, n: ed.countN++ };
      ed.shapes.push(sh);
      render();
      return;
    } else {
      ed.drag = { preview: { ...base(t), x1: p.x, y1: p.y, x2: p.x, y2: p.y } };
    }
    render();
  }

  function onMove(e) {
    if (!ed) return;
    const d = ed.drag;
    if (!d) {
      // Not drawing: preview the snap under the cursor for drawing tools.
      const drawTool = !['select', 'pen', 'highlight'].includes(ed.tool);
      const before = ed.hint;
      if (drawTool) snapped(e); else ed.hint = null;
      if (before || ed.hint) render();
      return;
    }
    if (d.mode === 'move') {
      // Shift: move only straight across or straight up/down.
      const p = toImg(e);
      let want = { x: p.x - d.start.x, y: p.y - d.start.y };
      if (e.shiftKey) want = Math.abs(want.x) >= Math.abs(want.y) ? { x: want.x, y: 0 } : { x: 0, y: want.y };
      if (!d.moved) { pushUndo(); d.moved = true; }
      moveShape(d.sh, want.x - d.applied.x, want.y - d.applied.y);
      d.applied = want;
    } else if (d.crop) {
      const p = snapped(e);
      d.crop.x2 = Math.max(0, Math.min(ed.W, p.x));
      d.crop.y2 = Math.max(0, Math.min(ed.H, p.y));
    } else if (d.preview.pts) {
      ed.hint = null;
      const p = toImg(e);
      if (e.shiftKey && d.preview.pts.length) {
        // Shift with the pen: straight horizontal / vertical stroke from where you started.
        const s = d.preview.pts[0];
        d.preview.pts = [s, Math.abs(p.x - s.x) >= Math.abs(p.y - s.y) ? { x: p.x, y: s.y } : { x: s.x, y: p.y }];
      } else {
        d.preview.pts.push(p);
      }
    } else {
      const from = { x: d.preview.x1, y: d.preview.y1 };
      let p;
      if (e.shiftKey && ['line', 'arrow', 'callout'].includes(d.preview.type)) {
        // Shift: lines lock to horizontal or vertical.
        const r = orthoSnap(toImg(e), from, snapTargets(d.preview), snapOn(e));
        p = r.p;
        ed.hint = r.hint;
      } else {
        p = snapped(e, d.preview);
        if (e.shiftKey) {
          // Shift: boxes and ellipses become squares and circles.
          const dx = p.x - from.x, dy = p.y - from.y;
          const m = Math.max(Math.abs(dx), Math.abs(dy));
          p = { x: from.x + Math.sign(dx || 1) * m, y: from.y + Math.sign(dy || 1) * m };
          ed.hint = null;
        }
      }
      d.preview.x2 = p.x;
      d.preview.y2 = p.y;
    }
    render();
  }

  function onUp() {
    const d = ed?.drag;
    if (!d) return;
    ed.drag = null;
    ed.hint = null;
    if (d.mode === 'move') { render(); return; }
    if (d.crop) {
      const r = norm(d.crop);
      if (r.w > 8 && r.h > 8) {
        pushUndo();
        ed.crop = { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.w), h: Math.round(r.h) };
        ed.zoom = 1;
        status(`Cropped to ${ed.crop.w} × ${ed.crop.h} — Undo to restore`);
        setTool('select');
        layout();
      } else render();
      return;
    }
    const sh = d.preview;
    const tiny = sh.pts ? false : Math.hypot(sh.x2 - sh.x1, sh.y2 - sh.y1) < 4;
    if (tiny && sh.type !== 'callout') { render(); return; }
    pushUndo();
    if (sh.type === 'callout') {
      // Text box goes where you released; arrow points where you pressed.
      if (tiny) { sh.x2 = sh.x1 + 60; sh.y2 = sh.y1 - 60; }
      sh.text = '';
      ed.shapes.push(sh);
      select(sh.id);
      editText(sh, true);
      return;
    }
    ed.shapes.push(sh);
    render();
  }

  function onDblClick(e) {
    const sh = hit(toImg(e));
    if (sh && (sh.type === 'text' || sh.type === 'callout')) {
      select(sh.id);
      pushUndo();
      editText(sh, false);
    }
  }

  function onWheel(e) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    ed.zoom = Math.max(0.2, Math.min(8, ed.zoom * (e.deltaY < 0 ? 1.15 : 1 / 1.15)));
    layout();
  }

  // ------------------------------------------------------ text editing

  function editText(sh, isNew) {
    const ta = document.createElement('textarea');
    ta.className = 'cap-text';
    ta.value = sh.text || '';
    const x = sh.type === 'callout' ? sh.x2 : sh.x, y = sh.type === 'callout' ? sh.y2 : sh.y;
    const p = toScreen(x, y);
    const fs = sh.fontSize * ed.view.s;
    Object.assign(ta.style, {
      left: `${p.x}px`, top: `${p.y}px`, fontSize: `${fs}px`, padding: `${fs * 0.4}px`,
      color: sh.type === 'callout' ? '#111' : sh.color, lineHeight: '1.25',
    });
    ed.root.appendChild(ta);
    ed.editing = { sh, ta, isNew };
    const grow = () => {
      ta.style.height = 'auto';
      ta.style.width = 'auto';
      ta.style.height = `${ta.scrollHeight + 2}px`;
      ta.style.width = `${Math.max(fs * 4, ta.scrollWidth + 4)}px`;
    };
    ta.addEventListener('input', () => { sh.text = ta.value; grow(); render(); });
    ta.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' || (e.key === 'Enter' && e.ctrlKey)) { e.preventDefault(); commitText(); }
    });
    // Only finish THIS text box (a late blur must not close the next one).
    ta.addEventListener('blur', () => setTimeout(() => { if (ed?.editing?.ta === ta) commitText(); }, 0));
    sh.hidden = true;
    grow();
    ta.focus(); // right away, so no keystrokes are lost
    render();
  }

  function commitText() {
    const e = ed?.editing;
    if (!e) return;
    ed.editing = null;
    e.sh.text = e.ta.value.replace(/\s+$/, '');
    delete e.sh.hidden;
    e.ta.remove();
    if (!e.sh.text) {
      ed.shapes = ed.shapes.filter((s) => s !== e.sh);
      if (e.isNew) ed.undo.pop(); // nothing was added
      ed.sel = null;
    }
    render();
  }

  // ------------------------------------------------------- markup list

  function renderList() {
    const items = ed.root.querySelector('.cap-items');
    ed.root.querySelector('.cap-count').textContent = ed.shapes.length;
    items.innerHTML = '';
    [...ed.shapes].reverse().forEach((sh) => {
      const row = document.createElement('div');
      row.className = 'cap-item' + (sh.id === ed.sel ? ' on' : '');
      const label = sh.type === 'count' ? `${NAMES.count} ${sh.n}`
        : sh.text ? `${NAMES[sh.type]}: ${sh.text.split('\n')[0]}` : NAMES[sh.type];
      row.innerHTML = `<i style="background:${sh.color}"></i><span></span><button title="Delete">✕</button>`;
      row.querySelector('span').textContent = label;
      row.onclick = (ev) => {
        if (ev.target.tagName === 'BUTTON') { deleteShape(sh.id); return; }
        setTool('select');
        select(sh.id);
      };
      items.appendChild(row);
    });
    if (!ed.shapes.length) items.innerHTML = '<div class="cap-empty">No markups yet. Pick a tool above and draw on the capture.</div>';
  }

  function deleteShape(id) {
    pushUndo();
    ed.shapes = ed.shapes.filter((s) => s.id !== id);
    if (ed.sel === id) ed.sel = null;
    render();
  }

  // ----------------------------------------------------------- export

  function exportCanvas() {
    const a = area();
    const c = document.createElement('canvas');
    c.width = a.w;
    c.height = a.h;
    const ctx = c.getContext('2d');
    ctx.translate(-a.x, -a.y);
    ctx.drawImage(ed.src, 0, 0);
    for (const sh of ed.shapes) drawShape(ctx, sh);
    return c;
  }

  const toBlob = (c) => new Promise((res) => c.toBlob(res, 'image/png'));

  async function copy() {
    commitText();
    try {
      const blob = await toBlob(exportCanvas());
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
      status('Copied — paste it into Discord, a document, anywhere');
    } catch (e) {
      status(`Couldn't copy: ${e.message || e}`);
    }
  }

  async function save() {
    commitText();
    const d = new Date();
    const pad = (n) => String(n).padStart(2, '0');
    const name = `Respawn ${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}.png`;
    try {
      const blob = await toBlob(exportCanvas());
      const bytes = new Uint8Array(await blob.arrayBuffer());
      const path = await invoke('save_capture', bytes, { headers: { 'x-file-name': name } });
      ed.dirty = false;
      status(`Saved: ${path}`, true);
    } catch (e) {
      status(`Couldn't save: ${e}`);
    }
  }

  function status(msg, withFolder) {
    if (!ed) return;
    const s = ed.root.querySelector('.cap-status');
    s.textContent = msg;
    s.title = msg;
    if (withFolder) {
      const b = document.createElement('button');
      b.className = 'cap-link';
      b.textContent = 'Open folder';
      b.onclick = () => invoke('open_captures_folder');
      s.appendChild(document.createTextNode(' '));
      s.appendChild(b);
    }
  }

  function action(a) {
    switch (a) {
      case 'snap': return toggleSnap();
      case 'undo': return undo();
      case 'redo': return redo();
      case 'zoomin': ed.zoom = Math.min(8, ed.zoom * 1.25); return layout();
      case 'zoomout': ed.zoom = Math.max(0.2, ed.zoom / 1.25); return layout();
      case 'fit': ed.zoom = 1; return layout();
      case 'copy': return copy();
      case 'save': return save();
      case 'close': return tryClose();
    }
  }

  function tryClose() {
    if (ed.dirty && Date.now() - ed.closeArmed > 2500) {
      ed.closeArmed = Date.now();
      status('Unsaved markups — press Esc or ✕ again to discard');
      return;
    }
    close();
  }

  // --------------------------------------------------------- keyboard

  function onKey(e) {
    if (!ed) return;
    // Typing in a markup's text box: leave the keys to the text box.
    if (ed.editing) return;
    // Otherwise the editor owns the keyboard (no tab switching etc.).
    e.stopImmediatePropagation();
    const k = e.key.toLowerCase();
    if (e.ctrlKey) {
      if (k === 'z') { e.preventDefault(); e.shiftKey ? redo() : undo(); }
      else if (k === 'y') { e.preventDefault(); redo(); }
      else if (k === 'c') { e.preventDefault(); copy(); }
      else if (k === 's') { e.preventDefault(); save(); }
      else if (k === '0') { e.preventDefault(); action('fit'); }
      else if (k === '=' || k === '+') { e.preventDefault(); action('zoomin'); }
      else if (k === '-') { e.preventDefault(); action('zoomout'); }
      return;
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      if (ed.sel) { ed.sel = null; render(); } else tryClose();
      return;
    }
    if ((e.key === 'Delete' || e.key === 'Backspace') && ed.sel) { e.preventDefault(); deleteShape(ed.sel); return; }
    if (e.key.startsWith('Arrow') && ed.sel) {
      e.preventDefault();
      const sh = ed.shapes.find((s) => s.id === ed.sel);
      const step = (e.shiftKey ? 10 : 1) / Math.max(ed.view.s, 0.01);
      pushUndo();
      moveShape(sh, e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0,
        e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0);
      render();
      return;
    }
    if (k === 's' && !e.altKey) { e.preventDefault(); toggleSnap(); return; }
    const tool = TOOLS.find(([, , key]) => key.toLowerCase() === k);
    if (tool && !e.altKey) { e.preventDefault(); setTool(tool[0]); }
  }

  // ---------------------------------------------------------- wiring

  window.startCapture = startCapture;
  // For troubleshooting: the current markups as data.
  window.__respawnMarkups = () => (ed ? JSON.parse(snapshot()).shapes : null);
  document.getElementById('capture-btn')?.addEventListener('click', startCapture);
  listen('capture-request', () => startCapture());
})();
