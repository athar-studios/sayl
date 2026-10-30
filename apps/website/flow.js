// Torrent background: streaks advected by a flow field, drifting right->left
// Fixed full-page background on the home page
(function () {
  'use strict';
  const c = document.getElementById('flow');
  if (!c) return;
  const x = c.getContext('2d');
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;

  const isHome = document.body.classList.contains('page-home') ||
                 window.location.pathname === '/' ||
                 window.location.pathname === '/index.html' ||
                 window.location.pathname === '/en' ||
                 window.location.pathname === '/en/' ||
                 window.location.pathname === '/en/index.html';

  if (!isHome) {
    c.style.display = 'none';
    return;
  }

  let W, H, P = [], t = 0, animId = null;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);

  function spawn(any) {
    return {
      x: any ? Math.random() * W : W + Math.random() * 80,
      y: Math.random() * H,
      s: 0.6 + Math.random() * 1.8,
      l: 0
    };
  }

  let lastW = 0;
  function size() {
    const vw = window.innerWidth;
    // Mobile browsers fire resize when the address bar shows/hides; only rebuild when width changes
    if (vw === lastW && P.length) return;
    lastW = vw;
    const vh = Math.max(window.innerHeight, document.documentElement.clientHeight, 1);
    const ch = Math.round(vh * 1.15); // cover the largest viewport (address bar hidden)

    W = c.width = Math.round(vw * dpr);
    H = c.height = Math.round(ch * dpr);

    // Ample density: 1 particle per 950px
    const n = Math.round((vw * ch) / 950);
    P = [];
    for (let i = 0; i < n; i++) {
      P.push(spawn(true));
    }

    x.fillStyle = '#070F2E';
    x.fillRect(0, 0, W, H);

    if (reduce) {
      renderStaticFrame();
    }
  }

  function field(px, py) {
    const k = 0.0016 / dpr;
    const a =
      Math.sin(py * k * 1.3 + t * 0.6) * 1.1 +
      Math.cos(px * k * 0.7 - t * 0.4) * 0.8 +
      Math.sin((px + py) * k * 0.5 + t * 0.3) * 0.6;

    // Wide, sweeping river current bending across the full width of the hero
    const band = H * (0.50 + 0.18 * Math.sin(px * k * 0.55 + t * 0.22));
    const d = Math.exp(-Math.pow((py - band) / (H * 0.35), 2));

    return {
      vx: -(1.6 + 3.2 * d),
      vy: a * 0.85 * (1 - d * 0.4),
      d: d
    };
  }

  function step() {
    t += 0.008;

    // Soft decay allows streaks to leave luminous fading trails
    x.globalCompositeOperation = 'source-over';
    x.fillStyle = 'rgba(7, 15, 46, 0.085)';
    x.fillRect(0, 0, W, H);

    // Luminous additive blending for the glowing turquoise stream
    x.globalCompositeOperation = 'lighter';

    for (let i = 0; i < P.length; i++) {
      const p = P[i];
      const f = field(p.x, p.y);
      const nx = p.x + f.vx * p.s * dpr;
      const ny = p.y + f.vy * p.s * dpr;

      // Color palette: deep turquoise -> radiant cyan -> bright foam
      const r = Math.round(f.d * 100);
      const g = Math.round(145 + f.d * 85);
      const b = Math.round(185 + f.d * 60);

      // Alpha: ~0.15 ambient background streaks up to ~0.38 in main current (~0.3-0.4)
      const alpha = 0.14 + f.d * 0.24;

      x.lineWidth = (0.75 + f.d * 1.35) * dpr;
      x.strokeStyle = `rgba(${r},${g},${b},${alpha})`;

      x.beginPath();
      x.moveTo(p.x, p.y);
      x.lineTo(nx, ny);
      x.stroke();

      p.x = nx;
      p.y = ny;

      if (p.x < -20 || p.y < -20 || p.y > H + 20) {
        Object.assign(p, spawn(false));
      }
    }
  }

  function tick() {
    step();
    animId = requestAnimationFrame(tick);
  }

  // Static artwork of the torrent when reduced motion is preferred on home
  function renderStaticFrame() {
    x.globalCompositeOperation = 'source-over';
    x.fillStyle = '#070F2E';
    x.fillRect(0, 0, W, H);

    x.globalCompositeOperation = 'lighter';
    for (let pass = 0; pass < 60; pass++) {
      step();
    }
  }

  window.addEventListener('resize', size);
  size();

  if (isHome && !reduce) {
    // Warm-up loop so streaks immediately span across the hero on initial render
    for (let i = 0; i < 20; i++) {
      step();
    }
    animId = requestAnimationFrame(tick);
  }
})();
