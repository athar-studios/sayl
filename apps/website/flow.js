// Torrent background: thousands of streaks advected by a flow field, drifting right->left
// Enhanced brightness, line thickness, and turquoise-to-foam luminous gradient
(function () {
  'use strict';
  const c = document.getElementById('flow');
  if (!c) return;
  const x = c.getContext('2d');
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;

  let W, H, P = [], t = 0, animId = null;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);

  function spawn(any) {
    return {
      x: any ? Math.random() * W : W + Math.random() * 80,
      y: Math.random() * H,
      s: 0.7 + Math.random() * 1.8,
      l: 0
    };
  }

  function size() {
    W = c.width = Math.round(window.innerWidth * dpr);
    H = c.height = Math.round(window.innerHeight * dpr);

    const n = Math.round((window.innerWidth * window.innerHeight) / 600);
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
    const k = 0.0015 / dpr;
    const a =
      Math.sin(py * k * 1.3 + t * 0.6) * 1.1 +
      Math.cos(px * k * 0.7 - t * 0.4) * 0.8 +
      Math.sin((px + py) * k * 0.5 + t * 0.3) * 0.6;

    // Channel: powerful current along a sweeping band across the lower-middle page
    const band = H * (0.54 + 0.18 * Math.sin(px * k * 0.6 + t * 0.2));
    const d = Math.exp(-Math.pow((py - band) / (H * 0.24), 2));

    return {
      vx: -(1.5 + 4.2 * d),
      vy: a * 0.95 * (1 - d * 0.5),
      d: d
    };
  }

  function step() {
    t += 0.0085;

    // Soft decay allows streaks to leave luminous fading trails
    x.globalCompositeOperation = 'source-over';
    x.fillStyle = 'rgba(7, 15, 46, 0.075)';
    x.fillRect(0, 0, W, H);

    // Luminous additive blending for the glowing turquoise stream
    x.globalCompositeOperation = 'lighter';

    for (let i = 0; i < P.length; i++) {
      const p = P[i];
      const f = field(p.x, p.y);
      const nx = p.x + f.vx * p.s * dpr;
      const ny = p.y + f.vy * p.s * dpr;

      // Color gradient: deep teal -> radiant turquoise -> bright foam white
      const r = Math.round(f.d * 125);
      const g = Math.round(155 + f.d * 90);
      const b = Math.round(190 + f.d * 65);
      const alpha = Math.min(0.35, 0.09 + f.d * 0.26);

      x.lineWidth = (1.0 + f.d * 1.8) * dpr;
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

  // Render a crisp, beautiful static artwork of the torrent when reduced motion is preferred
  function renderStaticFrame() {
    x.globalCompositeOperation = 'source-over';
    x.fillStyle = '#070F2E';
    x.fillRect(0, 0, W, H);

    // Draw a subtle background ambient gradient for depth
    const bgGrad = x.createLinearGradient(W, H * 0.4, 0, H * 0.7);
    bgGrad.addColorStop(0, 'rgba(0, 182, 189, 0.06)');
    bgGrad.addColorStop(0.5, 'rgba(11, 26, 78, 0.25)');
    bgGrad.addColorStop(1, 'rgba(0, 182, 189, 0.04)');
    x.fillStyle = bgGrad;
    x.fillRect(0, 0, W, H);

    x.globalCompositeOperation = 'lighter';

    // Accumulate multi-pass streak field
    for (let pass = 0; pass < 70; pass++) {
      t += 0.01;
      for (let i = 0; i < P.length; i++) {
        const p = P[i];
        const f = field(p.x, p.y);
        const nx = p.x + f.vx * p.s * dpr * 1.2;
        const ny = p.y + f.vy * p.s * dpr * 1.2;

        const r = Math.round(f.d * 120);
        const g = Math.round(150 + f.d * 90);
        const b = Math.round(185 + f.d * 70);
        const alpha = Math.min(0.28, 0.06 + f.d * 0.20);

        x.lineWidth = (0.9 + f.d * 1.6) * dpr;
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

  if (!reduce) {
    // Warm up the canvas briefly so streaks are immediately visible without blocking main thread
    for (let i = 0; i < 6; i++) {
      step();
    }
    animId = requestAnimationFrame(tick);
  }
})();
