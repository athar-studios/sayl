/**
 * SAYL (سيل) Website Core JavaScript
 * Handles:
 * - First-visit language prompt & localStorage ('sayl-lang')
 * - Header language switcher (AR <-> EN)
 * - Canvas logo color-flow animation (respects prefers-reduced-motion)
 * - Live download segments & progress counter
 * - Interactive mockup screenshot switcher
 * - Docs sidebar search & active section highlighting
 */

(function () {
  'use strict';

  const STORAGE_KEY = 'sayl-lang';
  const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // ---------------------------------------------------------------------------
  // 1. Language Management
  // ---------------------------------------------------------------------------
  function getSavedLanguage() {
    try {
      return localStorage.getItem(STORAGE_KEY);
    } catch (e) {
      return null;
    }
  }

  function saveLanguage(lang) {
    try {
      localStorage.setItem(STORAGE_KEY, lang);
    } catch (e) {}
  }

  function applyLanguage(lang) {
    const isAr = lang === 'ar';
    document.documentElement.lang = lang;
    document.documentElement.dir = isAr ? 'rtl' : 'ltr';

    // Update simple text elements with data-ar / data-en
    document.querySelectorAll('[data-ar]').forEach((el) => {
      const text = isAr ? el.getAttribute('data-ar') : el.getAttribute('data-en');
      if (text !== null) {
        // If element has no child tags or only text, swap textContent, else swap innerHTML
        if (el.children.length === 0) {
          el.textContent = text;
        } else {
          el.innerHTML = text;
        }
      }
    });

    // Update input placeholders
    document.querySelectorAll('[data-ar-placeholder]').forEach((el) => {
      const ph = isAr ? el.getAttribute('data-ar-placeholder') : el.getAttribute('data-en-placeholder');
      if (ph !== null) el.placeholder = ph;
    });

    // Toggle language-specific container blocks
    document.querySelectorAll('[data-lang-block]').forEach((el) => {
      const blockLang = el.getAttribute('data-lang-block');
      if (blockLang === lang) {
        el.style.display = '';
      } else {
        el.style.display = 'none';
      }
    });

    // Update language toggle buttons in header
    document.querySelectorAll('.lang-btn').forEach((btn) => {
      btn.textContent = isAr ? 'English' : 'العربية';
      btn.setAttribute('aria-label', isAr ? 'Switch to English' : 'التبديل إلى العربية');
    });

    // Notify any active counters
    window.dispatchEvent(new CustomEvent('sayl-lang-changed', { detail: { lang } }));
  }

  function initLanguage() {
    const saved = getSavedLanguage();
    const modal = document.getElementById('lang-modal');

    if (!saved) {
      // First visit: show selection modal
      if (modal) {
        modal.classList.add('show');
      }
      // Default to Arabic initially
      applyLanguage('ar');
    } else {
      applyLanguage(saved);
    }

    // Modal button clicks
    document.querySelectorAll('[data-choose-lang]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const chosen = btn.getAttribute('data-choose-lang');
        saveLanguage(chosen);
        applyLanguage(chosen);
        if (modal) modal.classList.remove('show');
      });
    });

    // Header toggle button
    document.querySelectorAll('.lang-btn').forEach((btn) => {
      btn.addEventListener('click', () => {
        const current = document.documentElement.lang || 'ar';
        const next = current === 'ar' ? 'en' : 'ar';
        saveLanguage(next);
        applyLanguage(next);
      });
    });
  }

  // ---------------------------------------------------------------------------
  // 2. Logo Internal Color-Flow Animation (still logo, colors flow inside)
  // ---------------------------------------------------------------------------
  function initLogoAnimation() {
    const canvas = document.getElementById('logo');
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    const img = new Image();
    img.src = 'icon.svg';

    img.onload = () => {
      const W = 1024;
      const H = Math.round((W * img.naturalHeight) / img.naturalWidth);
      canvas.width = W;
      canvas.height = H;

      const t0 = performance.now();

      function render(now) {
        const p = ((now - t0) / 3600) % 1;
        ctx.globalCompositeOperation = 'source-over';
        ctx.clearRect(0, 0, W, H);
        ctx.drawImage(img, 0, 0, W, H);

        ctx.globalCompositeOperation = 'source-atop';
        const L = W * 1.25;
        const o = p * L;

        for (let k = -1; k < 2; k++) {
          const b = W - o + k * L;
          const g = ctx.createLinearGradient(b - L, 0, b, 0);
          g.addColorStop(0, 'rgba(11,26,78,0)');
          g.addColorStop(0.24, 'rgba(11,26,78,0.78)');
          g.addColorStop(0.42, 'rgba(0,182,189,0.65)');
          g.addColorStop(0.5, 'rgba(127,240,242,0.95)');
          g.addColorStop(0.54, 'rgba(235,255,255,0.92)');
          g.addColorStop(0.62, 'rgba(0,182,189,0.65)');
          g.addColorStop(0.8, 'rgba(11,26,78,0.72)');
          g.addColorStop(1, 'rgba(11,26,78,0)');

          ctx.fillStyle = g;
          ctx.fillRect(b - L, 0, L, H);
        }

        if (!prefersReducedMotion) {
          requestAnimationFrame(render);
        }
      }

      if (prefersReducedMotion) {
        render(t0 + 1800);
      } else {
        requestAnimationFrame(render);
      }
    };
  }

  // ---------------------------------------------------------------------------
  // 3. Live Download Card Simulation (Hero)
  // ---------------------------------------------------------------------------
  function initDownloadDemo() {
    const segs = document.querySelector('.segs');
    const pctEl = document.getElementById('pct');
    if (!segs || !pctEl) return;

    segs.innerHTML = '';
    for (let i = 0; i < 8; i++) {
      const d = document.createElement('div');
      d.className = 'seg';
      const dur = 4.5 + Math.random() * 4.5;
      const delay = Math.random() * 0.9;
      d.innerHTML = `<i style="animation-duration:${dur}s;animation-delay:${delay}s"></i>`;
      segs.appendChild(d);
    }

    const t0 = performance.now();
    function updateCounter() {
      const elapsed = performance.now() - t0;
      const pct = Math.min(100, Math.round(elapsed / 80));
      const isAr = document.documentElement.lang === 'ar';

      pctEl.textContent = isAr ? pct.toLocaleString('ar-EG') + '٪' : pct + '%';

      if (pct < 100 && !prefersReducedMotion) {
        requestAnimationFrame(updateCounter);
      }
    }

    if (!prefersReducedMotion) {
      requestAnimationFrame(updateCounter);
    } else {
      const isAr = document.documentElement.lang === 'ar';
      pctEl.textContent = isAr ? '٦٨٪' : '68%';
    }

    window.addEventListener('sayl-lang-changed', (e) => {
      const isAr = e.detail.lang === 'ar';
      const current = parseInt(pctEl.textContent.replace(/[^\d]/g, ''), 10) || 68;
      pctEl.textContent = isAr ? current.toLocaleString('ar-EG') + '٪' : current + '%';
    });
  }

  // ---------------------------------------------------------------------------
  // 4. Mockups Screenshot Showcase Tab Switcher
  // ---------------------------------------------------------------------------
  function initMockupsShowcase() {
    const tabs = document.querySelectorAll('.mockup-tab-btn');
    const img = document.getElementById('mockup-img');
    const capTitle = document.getElementById('mockup-caption-title');
    const capDesc = document.getElementById('mockup-caption-desc');
    if (!tabs.length || !img) return;

    tabs.forEach((tab) => {
      tab.addEventListener('click', () => {
        tabs.forEach((t) => t.classList.remove('active'));
        tab.classList.add('active');

        const src = tab.getAttribute('data-img');
        const arTitle = tab.getAttribute('data-ar-title');
        const enTitle = tab.getAttribute('data-en-title');
        const arDesc = tab.getAttribute('data-ar-desc');
        const enDesc = tab.getAttribute('data-en-desc');

        if (src) img.src = src;

        if (capTitle) {
          capTitle.setAttribute('data-ar', arTitle || '');
          capTitle.setAttribute('data-en', enTitle || '');
          const isAr = document.documentElement.lang === 'ar';
          capTitle.textContent = isAr ? arTitle : enTitle;
        }

        if (capDesc) {
          capDesc.setAttribute('data-ar', arDesc || '');
          capDesc.setAttribute('data-en', enDesc || '');
          const isAr = document.documentElement.lang === 'ar';
          capDesc.textContent = isAr ? arDesc : enDesc;
        }
      });
    });
  }

  // ---------------------------------------------------------------------------
  // 5. Docs Sidebar: Filter Search & Active Link Highlight
  // ---------------------------------------------------------------------------
  function initDocsSidebar() {
    const search = document.querySelector('.docs-search');
    const links = document.querySelectorAll('.docs aside a');
    const mobileToggle = document.querySelector('.docs-mobile-toggle');
    const navLinksContainer = document.querySelector('.docs-nav-links');

    if (search && links.length) {
      search.addEventListener('input', (e) => {
        const query = e.target.value.toLowerCase().trim();
        links.forEach((a) => {
          const text = (a.textContent || '').toLowerCase();
          const href = (a.getAttribute('href') || '').toLowerCase();
          if (!query || text.includes(query) || href.includes(query)) {
            a.style.display = 'block';
          } else {
            a.style.display = 'none';
          }
        });
      });
    }

    if (mobileToggle && navLinksContainer) {
      mobileToggle.addEventListener('click', () => {
        navLinksContainer.classList.toggle('mobile-open');
      });
      links.forEach((a) => {
        a.addEventListener('click', () => {
          navLinksContainer.classList.remove('mobile-open');
        });
      });
    }

    // Active link highlighting via IntersectionObserver
    if ('IntersectionObserver' in window && links.length) {
      const headings = document.querySelectorAll('.docs article h2[id]');
      const linkMap = {};
      links.forEach((a) => {
        const id = a.getAttribute('href');
        if (id && id.startsWith('#')) {
          linkMap[id.substring(1)] = a;
        }
      });

      const observer = new IntersectionObserver(
        (entries) => {
          entries.forEach((entry) => {
            if (entry.isIntersecting) {
              const id = entry.target.id;
              links.forEach((a) => a.classList.remove('on'));
              if (linkMap[id]) {
                linkMap[id].classList.add('on');
              }
            }
          });
        },
        { rootMargin: '-10% 0px -75% 0px' }
      );

      headings.forEach((h) => observer.observe(h));
    }
  }

  // ---------------------------------------------------------------------------
  // Boot
  // ---------------------------------------------------------------------------
  document.addEventListener('DOMContentLoaded', () => {
    initLanguage();
    initLogoAnimation();
    initDownloadDemo();
    initMockupsShowcase();
    initDocsSidebar();
  });
})();
