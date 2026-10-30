/**
 * SAYL (سيل) Website Core JavaScript
 * Handles:
 * - First-visit language prompt & localStorage ('sayl-lang')
 * - Header language switcher (AR <-> EN)
 * - Canvas logo color-flow animation (always runs)
 * - Live download segments & progress counter
 * - Interactive mockup screenshot switcher
 * - Docs sidebar search & active section highlighting
 */

(function () {
  'use strict';

  const STORAGE_KEY = 'sayl-lang';
  const prefersReducedMotion = false; // always animate (owner request)

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
      if (modal) {
        modal.classList.add('show');
      }
    }

    // Modal button clicks
    document.querySelectorAll('[data-choose-lang]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const chosen = btn.getAttribute('data-choose-lang');
        saveLanguage(chosen);
        if (modal) modal.classList.remove('show');

        const isCurrentlyEn = window.location.pathname.includes('/en/');
        if (chosen === 'en' && !isCurrentlyEn) {
          const currentFile = window.location.pathname.split('/').pop() || 'index.html';
          window.location.href = 'en/' + currentFile;
        } else if (chosen === 'ar' && isCurrentlyEn) {
          const currentFile = window.location.pathname.split('/').pop() || 'index.html';
          window.location.href = '../' + currentFile;
        }
      });
    });

    // Header toggle button remembers language
    document.querySelectorAll('.lang-btn').forEach((btn) => {
      btn.addEventListener('click', () => {
        const next = document.documentElement.lang === 'ar' ? 'en' : 'ar';
        saveLanguage(next);
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
    const isEn = window.location.pathname.includes('/en/');
    img.src = '/icon.svg';

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
          g.addColorStop(0.42, 'rgba(0,182,189,0.6)');
          g.addColorStop(0.5, 'rgba(0,182,189,0.7)');
          g.addColorStop(0.62, 'rgba(0,182,189,0.6)');
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
    const spdEl = document.getElementById('spd');
    const badgeEl = document.querySelector('.dl .state-badge');
    if (!segs || !pctEl) return;

    if (prefersReducedMotion) {
      segs.innerHTML = '';
      for (let i = 0; i < 8; i++) {
        const d = document.createElement('div');
        d.className = 'seg';
        d.innerHTML = '<i style="width:68%;"></i>';
        segs.appendChild(d);
      }
      const isAr = document.documentElement.lang === 'ar';
      pctEl.textContent = isAr ? '٦٨٪' : '68%';
      if (spdEl) spdEl.textContent = isAr ? '٤٢.٤ م.ب/ث · المتبقي ٣د ٢٠ث' : '42.4 MB/s · 3m 20s left';
      return;
    }

    let cycleTimer = null;
    let animFrame = null;

    function startCycle() {
      if (cycleTimer) clearTimeout(cycleTimer);
      if (animFrame) cancelAnimationFrame(animFrame);

      segs.innerHTML = '';
      const bars = [];
      for (let i = 0; i < 8; i++) {
        const d = document.createElement('div');
        d.className = 'seg';
        const inner = document.createElement('i');
        inner.style.width = '0%';
        d.appendChild(inner);
        segs.appendChild(d);
        bars.push({
          el: inner,
          speed: 0.88 + Math.random() * 0.28,
          lag: Math.random() * 0.08
        });
      }

      const isAr = document.documentElement.lang === 'ar';
      if (badgeEl) {
        badgeEl.textContent = isAr ? 'قيد التنزيل' : 'Downloading';
        badgeEl.style.background = 'rgba(0, 182, 189, 0.15)';
        badgeEl.style.color = 'var(--teal)';
      }

      const cycleDuration = 8500;
      const startT = performance.now();

      function update(now) {
        const elapsed = now - startT;
        const progress = Math.min(1, elapsed / cycleDuration);
        const pctVal = Math.round(progress * 100);
        const curAr = document.documentElement.lang === 'ar';

        pctEl.textContent = curAr ? pctVal.toLocaleString('ar-EG') + '٪' : pctVal + '%';

        bars.forEach((b) => {
          const segProgress = Math.min(100, Math.max(0, (progress - b.lag) * b.speed * 100));
          b.el.style.width = segProgress.toFixed(1) + '%';
        });

        if (spdEl) {
          if (progress < 1) {
            const currentSpeed = (41.4 + Math.sin(elapsed * 0.004) * 3.2).toFixed(1);
            const remainingSecs = Math.max(0, Math.round((1 - progress) * 28));
            if (curAr) {
              spdEl.textContent = `${Number(currentSpeed).toLocaleString('ar-EG')} م.ب/ث · المتبقي ${remainingSecs.toLocaleString('ar-EG')} ث`;
            } else {
              spdEl.textContent = `${currentSpeed} MB/s · ${remainingSecs}s left`;
            }
          } else {
            if (curAr) {
              spdEl.textContent = 'اكتمل التنزيل بنجاح · ٤.٨ ج.ب';
            } else {
              spdEl.textContent = 'Completed successfully · 4.8 GB';
            }
          }
        }

        if (progress < 1) {
          animFrame = requestAnimationFrame(update);
        } else {
          if (badgeEl) {
            badgeEl.textContent = curAr ? 'مكتمل' : 'Completed';
            badgeEl.style.background = 'rgba(0, 182, 189, 0.25)';
            badgeEl.style.color = '#7fe9ec';
          }
          cycleTimer = setTimeout(startCycle, 1600);
        }
      }

      animFrame = requestAnimationFrame(update);
    }

    startCycle();

    window.addEventListener('sayl-lang-changed', () => {
      if (prefersReducedMotion) {
        const isAr = document.documentElement.lang === 'ar';
        pctEl.textContent = isAr ? '٦٨٪' : '68%';
      }
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
        const title = tab.getAttribute('data-title') || tab.getAttribute('data-ar-title') || tab.getAttribute('data-en-title');
        const desc = tab.getAttribute('data-desc') || tab.getAttribute('data-ar-desc') || tab.getAttribute('data-en-desc');

        if (src) img.src = src;
        if (capTitle && title) capTitle.textContent = title;
        if (capDesc && desc) capDesc.textContent = desc;
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
