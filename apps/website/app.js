/**
 * SAYL Website — Client-side logic
 * No frameworks, no tracking, no external dependencies
 * Language toggle, mobile menu panel, animated metrics counters, FAQ accordion
 */

(function () {
    'use strict';

    // ==========================================================================
    // Language Data
    // ==========================================================================
    const translations = {
        ar: {
            dir: 'rtl',
            lang: 'ar',
            'hero-meta': 'الإصدار <span id="version-display">—</span> · تاريخ البناء <span id="build-date-display">—</span>',
            'footer-meta': 'إصدار <span id="footer-version">—</span> · مبني في <span id="footer-build-date">—</span> · لا تتبع · لا تحليلات · Apache-2.0',
            'faq-1-a': 'المشروع لا يزال في مرحلة التطوير والمواصفات المبكرة (ما قبل M1). النواة الأساسية (sayl-core)، ومكتبة HTTP (sayl-http)، وخادم الاختبارات (sayl-test-http)، ونظام التخزين (sayl-storage) مكتوبة وتخضع لاختبارات صارمة، بينما يجري العمل على واجهة سطح المكتب وتكامل المتصفح.',
            'faq-2-a-items': [
                'آلة حالات التنزيل بـ 11 حالة مع انتقالات محمية ومفحوصة (sayl-core، 33 اختباراً)',
                'نظام تصنيف وتدريج الأخطاء القابلة لإعادة المحاولة (sayl-core)',
                'ناقل أحداث محدود السعة مع ترقيم تسلسلي لمنع الفجوات (sayl-core)',
                'مفاتيح منع التكرار (Idempotency) مع انتهاء صلاحية زمني (sayl-core)',
                'عميل HTTP مع التحقق من النطاقات 206 وتتبع إعادة التوجيه الآمن (sayl-http، 65 اختباراً)',
                'خادم اختبارات HTTP حتمي يحاكي 27 وضع فشل للشبكة (sayl-test-http، 25 اختباراً)',
                'مخزن مهام آمن ضد التعطل مع سجل WAL ولقطات ذرية (sayl-storage، 18 اختباراً)'
            ],
            'faq-3-a-label': 'الميزات المخططة قيد العمل:',
            'faq-3-a-items': [
                'واجهة سطح المكتب عبر Tauri + Svelte (قائمة المهام، الخط الزمني، الاختصارات، السمات)',
                'جدولة التنزيلات، تحديد السرعة القصوى، وقائمة الانتظار التلقائية',
                'تكامل المتصفح عبر رسائل النظام الأصلية (Native Messaging) لكروميوم وفايرفوكس',
                'دعم تنزيل الفيديو عبر yt-dlp/FFmpeg مع دمج المسارات',
                'دعم بروتوكول التورنت عبر librqbit مع اختيار الملفات',
                'مثبتات موقعة لنظام Windows (.msi و .exe) مع نظام تحديث موثق'
            ],
            'faq-4-a': 'يرجى مراجعة ملف <a href="SECURITY.md">SECURITY.md</a> للاطلاع على إرشادات الإبلاغ المسؤول. ترسل بلاغات الثغرات بسرية إلى البريد المخصص وليس عبر Issues العامة.',
            'faq-5-a': 'نرحب بالمساهمات! راجع ملف <a href="CONTRIBUTING.md">CONTRIBUTING.md</a> للتعرف على إعداد بيئة التطوير، تشغيل الاختبارات، ومعايير قبول طلبات السحب.',
            'faq-6-a': 'المشروع منشور بموجب رخصة Apache-2.0 مفتوحة المصدر. يمكنك مراجعة نص الرخصة الكامل في ملف <a href="../LICENSE">LICENSE</a>.',
            'lang-toggle-label': 'تبديل اللغة إلى الإنجليزية',
            'lang-toggle-label-active': 'تبديل اللغة إلى العربية',
            'menu-open': 'فتح القائمة',
            'menu-close': 'إغلاق القائمة',
            'skip-link': 'تجاوز إلى المحتوى الرئيسي'
        },
        en: {
            dir: 'ltr',
            lang: 'en',
            'hero-meta': 'Version <span id="version-display-en">—</span> · Build date <span id="build-date-display-en">—</span>',
            'footer-meta': 'Version <span id="footer-version-en">—</span> · Built <span id="footer-build-date-en">—</span> · No tracking · No analytics · Apache-2.0',
            'faq-1-a': 'No. The project is currently in early specification and core development (pre-M1). Core logic (sayl-core), HTTP client (sayl-http), test fixture server (sayl-test-http), and task storage (sayl-storage) are implemented and tested, but the desktop UI and browser integration are actively being developed.',
            'faq-2-a-items': [
                '11-state download state machine with guarded transitions (sayl-core, 33 tests)',
                'Structured, classified error taxonomy with retry policies (sayl-core)',
                'Bounded event bus with sequence numbering and gap detection (sayl-core)',
                'Idempotency key management with TTL expiration (sayl-core)',
                'HTTP client with range validation, safe redirect limits, filename extraction (sayl-http, 65 tests)',
                'Deterministic HTTP fixture server covering 27 failure modes (sayl-test-http, 25 tests)',
                'Crash-resilient task store with WAL and atomic snapshots (sayl-storage, 18 tests)'
            ],
            'faq-3-a-label': 'Planned & upcoming features:',
            'faq-3-a-items': [
                'Tauri + Svelte desktop UI (task timeline, themes, keyboard shortcuts)',
                'Download scheduling, speed caps, and queue prioritization',
                'Browser integration via Native Messaging for Chromium and Firefox',
                'Media downloads via yt-dlp and FFmpeg integration',
                'BitTorrent protocol support powered by librqbit',
                'Signed Windows installers (.msi and .exe) with verified auto-updates'
            ],
            'faq-4-a': 'Please consult <a href="SECURITY.md">SECURITY.md</a> for vulnerability disclosure guidelines. Security reports must be sent privately, not via public GitHub Issues.',
            'faq-5-a': 'Contributions are welcome! Please check <a href="CONTRIBUTING.md">CONTRIBUTING.md</a> for local toolchain setup, test execution, commit style, and PR checklist.',
            'faq-6-a': 'SAYL is released under the permissive Apache-2.0 license. See the repository <a href="../LICENSE">LICENSE</a> file for details.',
            'lang-toggle-label': 'Switch to Arabic',
            'lang-toggle-label-active': 'Switch to English',
            'menu-open': 'Open menu',
            'menu-close': 'Close menu',
            'skip-link': 'Skip to main content'
        }
    };

    // ==========================================================================
    // State
    // ==========================================================================
    let currentLang = 'ar';

    // ==========================================================================
    // DOM Helpers
    // ==========================================================================
    function $(selector, context = document) {
        return context.querySelector(selector);
    }

    function $$(selector, context = document) {
        return Array.from(context.querySelectorAll(selector));
    }

    // ==========================================================================
    // Language Toggle
    // ==========================================================================
    function applyLanguage(lang) {
        const t = translations[lang];
        if (!t) return;

        currentLang = lang;
        document.documentElement.lang = t.lang;
        document.documentElement.dir = t.dir;
        document.body.dir = t.dir;

        // Update plain text elements
        $$('[data-ar][data-en]').forEach(el => {
            const key = lang === 'ar' ? 'data-ar' : 'data-en';
            const text = el.getAttribute(key);
            if (text !== null) {
                el.textContent = text;
            }
        });

        // Update aria-labels
        $$('[data-ar-label][data-en-label]').forEach(el => {
            const key = lang === 'ar' ? 'data-ar-label' : 'data-en-label';
            const label = el.getAttribute(key);
            if (label !== null) {
                el.setAttribute('aria-label', label);
            }
        });

        // Update rich content (meta info, FAQ answers)
        updateRichContent(t);

        // Update language toggle button
        const langToggle = $('#lang-toggle');
        if (langToggle) {
            langToggle.setAttribute('aria-label', t['lang-toggle-label' + (lang === 'en' ? '-active' : '')]);
            langToggle.setAttribute('aria-pressed', String(lang === 'en'));
            const currentSpan = langToggle.querySelector('.lang-current');
            if (currentSpan) {
                currentSpan.textContent = lang === 'ar' ? 'EN' : 'AR';
            }
        }

        // Update mobile menu button label
        const menuToggle = $('#menu-toggle');
        const mainNav = $('#main-nav');
        if (menuToggle && mainNav) {
            const isOpen = mainNav.classList.contains('is-open');
            menuToggle.setAttribute('aria-label', isOpen ? t['menu-close'] : t['menu-open']);
        }

        // Update skip link
        const skipLink = $('.skip-link');
        if (skipLink) {
            skipLink.textContent = t['skip-link'];
        }
    }

    function updateRichContent(t) {
        // Hero meta (version / build date)
        const heroMeta = $('#hero-meta');
        if (heroMeta) {
            heroMeta.innerHTML = t['hero-meta'];
        }

        // Footer meta
        const footerMeta = $('#footer-meta');
        if (footerMeta) {
            footerMeta.innerHTML = t['footer-meta'];
        }

        // FAQ answers
        updateFAQAnswers(t);
    }

    function updateFAQAnswers(t) {
        const faqItems = $$('.faq-item');
        const faqKeys = ['faq-1', 'faq-2', 'faq-3', 'faq-4', 'faq-5', 'faq-6'];

        faqItems.forEach((item, idx) => {
            const key = faqKeys[idx];
            if (!key) return;

            const answer = item.querySelector('.faq-answer');
            if (!answer) return;

            if (key === 'faq-2' && t['faq-2-a-items']) {
                const itemsHtml = t['faq-2-a-items'].map(it => `<li>${it}</li>`).join('');
                answer.innerHTML = `<ul>${itemsHtml}</ul>`;
            } else if (key === 'faq-3' && t['faq-3-a-items']) {
                const label = t['faq-3-a-label'] || '';
                const itemsHtml = t['faq-3-a-items'].map(it => `<li>${it}</li>`).join('');
                answer.innerHTML = `<p class="planned-label">${label}</p><ul>${itemsHtml}</ul>`;
            } else if (t[key + '-a']) {
                answer.innerHTML = `<p>${t[key + '-a']}</p>`;
            }
        });
    }

    function toggleLanguage() {
        const nextLang = currentLang === 'ar' ? 'en' : 'ar';
        applyLanguage(nextLang);
    }

    // ==========================================================================
    // Mobile Navigation Menu Toggle
    // ==========================================================================
    function initMobileMenu() {
        const menuToggle = $('#menu-toggle');
        const mainNav = $('#main-nav');
        if (!menuToggle || !mainNav) return;

        function setMenuState(open) {
            const isOpen = Boolean(open);
            mainNav.classList.toggle('is-open', isOpen);
            menuToggle.setAttribute('aria-expanded', String(isOpen));
            const t = translations[currentLang];
            const label = isOpen ? t['menu-close'] : t['menu-open'];
            menuToggle.setAttribute('aria-label', label);
        }

        menuToggle.addEventListener('click', () => {
            const willOpen = !mainNav.classList.contains('is-open');
            setMenuState(willOpen);
        });

        // Close menu when clicking any nav link
        $$('a', mainNav).forEach(link => {
            link.addEventListener('click', () => {
                setMenuState(false);
            });
        });

        // Close menu on Escape key
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape' && mainNav.classList.contains('is-open')) {
                setMenuState(false);
                menuToggle.focus();
            }
        });

        // Close menu when clicking outside
        document.addEventListener('click', (e) => {
            if (mainNav.classList.contains('is-open') &&
                !mainNav.contains(e.target) &&
                !menuToggle.contains(e.target)) {
                setMenuState(false);
            }
        });
    }

    // ==========================================================================
    // Technical Feature/Speed Counters Animation
    // ==========================================================================
    function initCounters() {
        const counters = $$('.counter');
        if (!counters.length) return;

        const prefersReducedMotion = window.matchMedia &&
            window.matchMedia('(prefers-reduced-motion: reduce)').matches;

        // If reduced motion is requested or no IntersectionObserver, show numbers immediately
        if (prefersReducedMotion || !('IntersectionObserver' in window)) {
            counters.forEach(c => {
                c.textContent = c.getAttribute('data-target') || '0';
            });
            return;
        }

        const observer = new IntersectionObserver((entries, obs) => {
            entries.forEach(entry => {
                if (entry.isIntersecting) {
                    const el = entry.target;
                    const target = parseInt(el.getAttribute('data-target') || '0', 10);
                    obs.unobserve(el);

                    if (target === 0) {
                        el.textContent = '0';
                        return;
                    }

                    const duration = 1000; // ms
                    const startTime = performance.now();

                    function updateNumber(now) {
                        const elapsed = now - startTime;
                        const progress = Math.min(1, elapsed / duration);
                        // Ease-out cubic
                        const easeOut = 1 - Math.pow(1 - progress, 3);
                        const current = Math.floor(easeOut * target);
                        el.textContent = String(current);

                        if (progress < 1) {
                            requestAnimationFrame(updateNumber);
                        } else {
                            el.textContent = String(target);
                        }
                    }

                    requestAnimationFrame(updateNumber);
                }
            });
        }, { threshold: 0.2 });

        counters.forEach(c => observer.observe(c));
    }

    // ==========================================================================
    // FAQ Accordion Keyboard Enhancement
    // ==========================================================================
    function enhanceFAQ() {
        $$('.faq-item summary').forEach(summary => {
            summary.addEventListener('keydown', (e) => {
                if (e.key === ' ' || e.key === 'Spacebar') {
                    e.preventDefault();
                    summary.click();
                }
            });
        });
    }

    // ==========================================================================
    // Build Metadata
    // ==========================================================================
    function injectBuildMetadata() {
        const version = 'v0.1.0';
        const buildDate = '2025-03-30';

        const versionEls = $$('#version-display, #version-display-en, #footer-version, #footer-version-en');
        const dateEls = $$('#build-date-display, #build-date-display-en, #footer-build-date, #footer-build-date-en');

        versionEls.forEach(el => { el.textContent = version; });
        dateEls.forEach(el => { el.textContent = buildDate; });
    }

    // ==========================================================================
    // Initialization
    // ==========================================================================
    function init() {
        applyLanguage('ar');

        const langToggle = $('#lang-toggle');
        if (langToggle) {
            langToggle.addEventListener('click', toggleLanguage);
        }

        initMobileMenu();
        initCounters();
        enhanceFAQ();
        injectBuildMetadata();
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
