// Torrent background: streaks advected by a flow field, drifting right->left
// Confined to the hero section on the home page, with calm static gradient on inner pages
(function () {
  'use strict';
  const c = document.getElementById('flow');
  if (!c) return;
  const x = c.getContext('2d');
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;

  const isHome = document.body.classList.contains('page-home') ||
                 !!document.querySelector('.hero #flow') ||
                 window.location.pathname === '/' ||
                 window.location.pathname === '/index.html' ||
                 window.location.pathname === '/en/' ||
                 window.location.pathname === '/en/index.html';

  let W, H, P = [], t = 0, animId = null;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);

  function spawn(any) {
    return {
      x: any ? Math.random() * W : W + Math.random() * 80,
      y: Math.random() * H,
      s: 0.6 + Math.random() * 1.5,
      l: 0
    };
  }

  function size() {
    const parent = c.parentElement;
    const clientW = parent ? parent.clientWidth : window.innerWidth;
    const clientH = parent ? parent.clientHeight : window.innerHeight;

    W = c.width = Math.round(clientW * dpr);
    H = c.height = Math.round(clientH * dpr);

    if (!isHome) {
      // Inner pages: calm static navy gradient with no streaks behind content
      renderStaticNavyGradient();
      return;
    }

    // Fewer particles: area / 1400
    const area = (clientW * clientH);
    const n = Math.max(50, Math.round(area / 1400));
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
    const k = 0.0014 / dpr;
    const a =
      Math.sin(py * k * 1.3 + t * 0.6) * 1.1 +
      Math.cos(px * k * 0.7 - t * 0.4) * 0.8 +
      Math.sin((px + py) * k * 0.5 + t * 0.3) * 0.6;

    // Channel: current across lower-middle hero
    const band = H * (0.52 + 0.16 * Math.sin(px * k * 0.6 + t * 0.2));
    const d = Math.exp(-Math.pow((py - band) / (H * 0.26), 2));

    return {
      vx: -(1.4 + 3.6 * d),
      vy: a * 0.9 * (1 - d * 0.5),
      d: d
    };
  }

  function step() {
    t += 0.0085;

    // Soft decay allows streaks to leave luminous fading trails
    x.globalCompositeOperation = 'source-over';
    x.fillStyle = 'rgba(7, 15, 46, 0.085)';
    x.fillRect(0, 0, W, H);

    // Luminous additive blending for the glowing turquoise stream
    x.globalCompositeOperation = 'lighter';

    // Home: max alpha 0.30; inner pages (if ever rendered): max alpha 0.18
    const maxAlpha = isHome ? 0.30 : 0.18;

    for (let i = 0; i < P.length; i++) {
      const p = P[i];
      const f = field(p.x, p.y);
      const nx = p.x + f.vx * p.s * dpr;
      const ny = p.y + f.vy * p.s * dpr;

      // Color gradient: deep teal -> radiant turquoise -> soft foam
      const r = Math.round(f.d * 110);
      const g = Math.round(145 + f.d * 80);
      const b = Math.round(180 + f.d * 60);
      const alpha = Math.min(maxAlpha, 0.04 + f.d * (maxAlpha - 0.04));

      // Thinner lines
      x.lineWidth = (0.55 + f.d * 0.85) * dpr;
      x.strokeStyle = `rgba(${r},${g},${b},${alpha})`;

      x.beginPath();
      x.moveTo(p.x, p.y);
      x.lineTo(nx, ny);
      x.stroke();

      p.x = nx;
      p.y = ny;

      if (p.x < -10 || p.y < -10 || p.y > H + 10) {
        Object.assign(p, spawn(false));
      }
    }
  }

  function tick() {
    step();
    animId = requestAnimationFrame(tick);
  }

  // Calm static navy gradient with zero streaks behind content
  function renderStaticNavyGradient() {
    x.globalCompositeOperation = 'source-over';
    const grad = x.createRadialGradient(W * 0.5, 0, 10, W * 0.5, H * 0.4, Math.max(W, H) * 0.8);
    grad.addColorStop(0, '#0d1e57');
    grad.addColorStop(0.6, '#070F2E');
    grad.addColorStop(1, '#050a22');
    x.fillStyle = grad;
    x.fillRect(0, 0, W, H);
  }

  // Static artwork of the torrent when reduced motion is preferred on home
  function renderStaticFrame() {
    x.globalCompositeOperation = 'source-over';
    x.fillStyle = '#070F2E';
    x.fillRect(0, 0, W, H);

    const bgGrad = x.createLinearGradient(W, H * 0.4, 0, H * 0.7);
    bgGrad.addColorStop(0, 'rgba(0, 182, 189, 0.04)');
    bgGrad.addColorStop(0.5, 'rgba(11, 26, 78, 0.22)');
    bgGrad.addColorStop(1, 'rgba(0, 182, 189, 0.03)');
    x.fillStyle = bgGrad;
    x.fillRect(0, 0, W, H);

    x.globalCompositeOperation = 'lighter';
    const maxAlpha = 0.22;

    for (let pass = 0; pass < 55; pass++) {
      t += 0.01;
      for (let i = 0; i < P.length; i++) {
        const p = P[i];
        const f = field(p.x, p.y);
        const nx = p.x + f.vx * p.s * dpr * 1.1;
        const ny = p.y + f.vy * p.s * dpr * 1.1;

        const r = Math.round(f.d * 105);
        const g = Math.round(140 + f.d * 80);
        const b = Math.round(175 + f.d * 60);
        const alpha = Math.min(maxAlpha, 0.04 + f.d * 0.16);

        x.lineWidth = (0.55 + f.d * 0.8) * dpr;
        x.strokeStyle = `rgba(${r},${g},${b},${alpha})`;

        x.beginPath();
        x.moveTo(p.x, p.y);
        x.lineTo(nx, ny);
        x.stroke();

        p.x = nx;
        p.y = ny;
        if (p.x < -10 || p.y < -10 || p.y > H + 10) {
          Object.assign(p, spawn(false));
        }
      }
    }
  }

  window.addEventListener('resize', size);
  size();

  if (isHome && !reduce) {
    for (let i = 0; i < 5; i++) {
      step();
    }
    animId = requestAnimationFrame(tick);
  }
})();
