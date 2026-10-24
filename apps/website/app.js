/**
 * SAYL Website — Client-side logic
 * No frameworks, no tracking, no external dependencies
 * Language toggle, artifact availability probing, FAQ accordion
 */

(function () {
    'use strict';

    // ==========================================================================
    // Configuration
    // ==========================================================================
    const ARTIFACT_BASE = '';
    const CHECK_INTERVAL_MS = 30000; // Re-check artifact availability every 30s

    // ==========================================================================
    // Language Data
    // ==========================================================================
    const translations = {
        ar: {
            dir: 'rtl',
            lang: 'ar',
            'hero-title': 'سيل (SAYL)',
            'hero-subtitle': '— مدير تحميل مستقل لـ Windows',
            'hero-description': 'مدير تحميل مفتوح المصدر، يحترم خصوصيتك، يدعم الاستئناف والتنزيل المتوازي، بواجهة عربية أولاً.',
            'hero-cta-primary': 'تنزيل الآن',
            'hero-cta-secondary': 'عرض على GitHub',
            'hero-meta': 'الإصدار <span id="version-display">—</span> · تاريخ البناء <span id="build-date-display">—</span>',
            'nav-features': 'المميزات',
            'nav-download': 'التنزيل',
            'nav-requirements': 'متطلبات النظام',
            'nav-privacy': 'الخصوصية',
            'nav-faq': 'الأسئلة الشائعة',
            'features-title': 'المميزات',
            'feature-1-title': 'استئناف التنزيل',
            'feature-1-desc': 'يدعم الاستئناف الآمن بعد انقطاع الاتصال أو إغلاق التطبيق، مع التحقق من سلامة الملف عبر التحقق من النطاقات (Range validation).',
            'feature-2-title': 'تنزيل متوازي موثوق',
            'feature-2-desc': 'تقسيم ديناميكي للنطاقات مع التحقق من دعم الخادم (206 Partial Content) قبل البدء، وسقف افتراضي 8 اتصالات للمهمة.',
            'feature-3-title': 'مخزن مهام آمن من التعطل',
            'feature-3-desc': 'سجل كتابة مسبق (WAL) مع لقطات ذرية (atomic snapshots)، استعادة تلقائية بعد انقطاع الطاقة أو تعطل العملية.',
            'feature-4-title': 'خصوصية مطلقة',
            'feature-4-desc': 'لا تحليلات، لا تتبع، لا خطوط خارجية، لا رفع تقارير تعطل تلقائي. بياناتك لا تغادر جهازك.',
            'feature-5-title': 'دعم RTL و i18n',
            'feature-5-desc': 'واجهة عربية أولاً مع دعم كامل للاتجاه RTL، مفاتيح ترجمة لجميع النصوص، وخيار يدوي للتبديل بين العربية والإنجليزية.',
            'feature-6-title': 'أسماء ملفات آمنة',
            'feature-6-desc': 'تعقيم أسماء الملفات من Content-Disposition و URL: منع اجتياز المسارات، أسماء الأجهزة المحجوزة في Windows، أحرف التحكم، مع الحفاظ على Unicode العربي.',
            'download-title': 'تنزيل سيل',
            'download-note': 'اختر المثبت المناسب لنظامك. الملفات موقعة وتتحقق عبر SHA256.',
            'platform-windows': 'Windows x64',
            'platform-source': 'كود المصدر',
            'download-installer-title': 'مثبت NSIS (موصى به)',
            'download-installer-desc': 'تثبيت لكل المستخدمين أو المستخدم الحالي، اختصارات قائمة البداية، إلغاء تثبيت نظيف.',
            'download-msi-title': 'مثبت MSI',
            'download-msi-desc': 'مخصص للنشر المؤسسي عبر Group Policy، تثبيت صامت، صيانة موثوقة.',
            'download-source-title': 'أرشيف المصدر',
            'download-source-desc': 'كود المصدر الكامل للإصدار، للتحقق والبناء من المصدر.',
            'btn-download-exe': 'تنزيل .exe',
            'btn-download-msi': 'تنزيل .msi',
            'btn-download-source': 'تنزيل مصدر .zip',
            'checksums-title': 'التحقق من السلامة',
            'checksums-desc': 'قارن تجزئة SHA256 للملف الذي نزّلته مع القيم الرسمية:',
            'btn-checksums': 'عرض SHA256SUMS.txt',
            'checksum-help-summary': 'كيف أتحقق؟',
            'checksum-help-text': 'على Windows PowerShell:',
            'checksum-help-compare': 'قارن النتيجة مع السطر المقابل في SHA256SUMS.txt.',
            'requirements-title': 'متطلبات النظام',
            'req-os': 'Windows 10 (1903+) أو Windows 11، بنية x64',
            'req-cpu': 'x64 مع دعم SSE2',
            'req-ram': '4 GB حد أدنى، 8 GB موصى به للتنزيلات المتوازية الكبيرة',
            'req-disk': '100 MB للتثبيت + مساحة للتنزيلات',
            'req-net': 'اتصال إنترنت للتنزيل (لا يتطلب اتصالًا للتشغيل المحلي)',
            'req-note': 'بناء GNU (rust-lld) يعمل للاختبارات؛ البناء الإنتاجي MSVC/TAURI يحتاج Windows SDK (مرفوع الامتيازات).',
            'privacy-title': 'الخصوصية',
            'privacy-core': '«لا نرسل سجل تنزيلاتك أو روابطك أو ملفاتك إلى خوادمنا. لا يحتوي التطبيق على تحليلات استخدام أو إعلانات أو تتبع. بيانات التنزيل تعالج محليًا، والاتصالات الخارجية اللازمة للتنزيل أو الميزات الاختيارية موضحة.»',
            'privacy-practical': 'ماذا يعني هذا عمليًا؟',
            'privacy-1': 'لا معرّفات جهاز، لا بصمة متصفح، لا تسجيل جلسات.',
            'privacy-2': 'لا رفع تقارير تعطل تلقائي — تقارير الأخطاء يدوية بالكامل.',
            'privacy-3': 'لا خطوط خارجية، لا مكتبات تحليلات، لا أدوات دردشة مدمجة.',
            'privacy-4': 'السجلات محلية، قابلة للحذف والتصدير بعد تنقية البيانات الحساسة.',
            'privacy-5': 'التحديثات وفهرس الإضافات/الثيمات اختيارية (opt-in) افتراضيًا.',
            'privacy-6': 'خادم الملف يرى اتصال تنزيل عادي؛ مستضيف الموقع/GitHub قد يعالجون بيانات وصفية وفق سياساتهم.',
            'faq-title': 'الأسئلة الشائعة',
            'faq-1-q': 'هل سيل (SAYL) جاهز للاستخدام اليومي؟',
            'faq-1-a': 'لا. المشروع في مرحلة المواصفات والتطوير المبكر (ما قبل M1). النواة (sayl-core)، HTTP (sayl-http)، خادم الاختبارات (sayl-test-http)، والتخزين (sayl-storage) موجودة وتخضع للاختبار، لكن واجهة سطح المكتب وتكامل المتصفح والميزات المتقدمة قيد التطوير.',
            'faq-2-q': 'ما هي الميزات المطبقة فعليًا اليوم؟',
            'faq-2-a-items': [
                'آلة حالات تنزيل بـ 11 حالة مع انتقالات محمية (sayl-core، 33 اختبار)',
                'نظام أخطاء مصنف وقابل لإعادة المحاولة (sayl-core)',
                'ناقل أحداث محدود السعة مع ترقيم تسلسلي وكشف فجوات (sayl-core)',
                'مفاتيح عدم تكرار (Idempotency) مع TTL (sayl-core)',
                'عميل HTTP مع تحقق النطاقات، تتبع إعادة التوجيه الآمن، استخراج أسماء ملفات آمنة (sayl-http، 65 اختبار)',
                'خادم اختبارات HTTP حتمي يغطي 27 وضع فشل (sayl-test-http، 25 اختبار)',
                'مخزن مهام آمن من التعطل مع WAL ولقطات ذرية (sayl-storage، 18 اختبار)'
            ],
            'faq-3-q': 'ما الميزات المخططة وغير المنجزة بعد؟',
            'faq-3-a-label': '<strong>قيد التطوير / Planned:</strong>',
            'faq-3-a-items': [
                'واجهة سطح مكتب Tauri + Svelte (Timeline، قائمة المهام، الثيمات، الاختصارات)',
                'جدولة التنزيلات، حد السرعة، الطابور، منطقة السحب',
                'تكامل المتصفح (Native Messaging، ملحقات Chromium/Firefox)',
                'دعم الفيديو عبر yt-dlp/FFmpeg (قوائم تشغيل، تحويل، دمج)',
                'دعم التورنت عبر librqbit (اختيار ملفات، بث/Seek)',
                'نظام إضافات WASM وكاتالوج إضافات/ثيمات',
                'معاينة Android HTTP + SAF + المهام الخلفية',
                'مثبتات MSI/NSIS موقعة، تحديثات تلقائية موقعة'
            ],
            'faq-4-q': 'كيف أبلغ عن خطأ أمني؟',
            'faq-4-a': 'راجع <a href="SECURITY.md">SECURITY.md</a> للتعليمات. تقارير الثغرات تُرسل خاصًا، لا عبر Issues العامة.',
            'faq-5-q': 'هل يمكنني المساهمة؟',
            'faq-5-a': 'نعم. راجع <a href="CONTRIBUTING.md">CONTRIBUTING.md</a> لإعداد بيئة GNU+rust-lld، أوامر الاختبار، نمط الكوميت، وقائمة فحص PR.',
            'faq-6-q': 'ما هي الرخصة؟',
            'faq-6-a': 'لم تُحسم بعد. سطر الرخصة في README: <code>License: to be decided by the project owner</code>.',
            'footer-github': 'GitHub',
            'footer-issues': 'Issues',
            'footer-security': 'الأمان',
            'footer-contributing': 'المساهمة',
            'footer-meta': 'إصدار <span id="footer-version">—</span> · مبني في <span id="footer-build-date">—</span> · لا تتبع · لا تحليلات',
            'lang-toggle-label': 'تبديل اللغة إلى الإنجليزية',
            'lang-toggle-label-active': 'تبديل اللغة إلى العربية',
            'artifact-status-checking': 'جاري التحقق...',
            'artifact-status-available': 'متاح',
            'artifact-status-unavailable': 'لم يُصدر هذا الإصدار بعد',
            'artifact-status-error': 'تعذر التحقق',
            'skip-link': 'تجاوز إلى المحتوى الرئيسي'
        },
        en: {
            dir: 'ltr',
            lang: 'en',
            'hero-title': 'SAYL',
            'hero-subtitle': '— Independent Download Manager for Windows',
            'hero-description': 'Open-source download manager that respects your privacy, supports resumable and parallel downloads, with Arabic-first UI.',
            'hero-cta-primary': 'Download Now',
            'hero-cta-secondary': 'View on GitHub',
            'hero-meta': 'Version <span id="version-display-en">—</span> · Build date <span id="build-date-display-en">—</span>',
            'nav-features': 'Features',
            'nav-download': 'Download',
            'nav-requirements': 'Requirements',
            'nav-privacy': 'Privacy',
            'nav-faq': 'FAQ',
            'features-title': 'Features',
            'feature-1-title': 'Resumable Downloads',
            'feature-1-desc': 'Safe resume after network interruption or app close, with range-validated integrity checks.',
            'feature-2-title': 'Validated Parallel Downloads',
            'feature-2-desc': 'Dynamic range splitting with server capability verification (206 Partial Content) before starting, default cap of 8 connections per task.',
            'feature-3-title': 'Crash-Safe Task Store',
            'feature-3-desc': 'Write-ahead log with atomic snapshots, automatic recovery after power loss or process crash.',
            'feature-4-title': 'Absolute Privacy',
            'feature-4-desc': 'No analytics, no tracking, no external fonts, no auto crash uploads. Your data never leaves your machine.',
            'feature-5-title': 'RTL & i18n Support',
            'feature-5-desc': 'Arabic-first UI with full RTL support, translation keys for all strings, manual Arabic/English toggle.',
            'feature-6-title': 'Safe Filenames',
            'feature-6-desc': 'Sanitizes filenames from Content-Disposition and URL: prevents path traversal, Windows reserved device names, control chars, preserves Arabic Unicode.',
            'download-title': 'Download SAYL',
            'download-note': 'Choose the installer for your system. Files are signed and verified via SHA256.',
            'platform-windows': 'Windows x64',
            'platform-source': 'Source Code',
            'download-installer-title': 'NSIS Installer (Recommended)',
            'download-installer-desc': 'Per-user or all-users install, Start Menu shortcuts, clean uninstall.',
            'download-msi-title': 'MSI Installer',
            'download-msi-desc': 'Designed for enterprise deployment via Group Policy, silent install, reliable servicing.',
            'download-source-title': 'Source Archive',
            'download-source-desc': 'Complete source code for the release, for verification and building from source.',
            'btn-download-exe': 'Download .exe',
            'btn-download-msi': 'Download .msi',
            'btn-download-source': 'Download Source .zip',
            'checksums-title': 'Verify Integrity',
            'checksums-desc': 'Compare the SHA256 hash of your downloaded file with the official values:',
            'btn-checksums': 'View SHA256SUMS.txt',
            'checksum-help-summary': 'How to verify?',
            'checksum-help-text': 'On Windows PowerShell:',
            'checksum-help-compare': 'Compare the output with the corresponding line in SHA256SUMS.txt.',
            'requirements-title': 'System Requirements',
            'req-os': 'Windows 10 (1903+) or Windows 11, x64 architecture',
            'req-cpu': 'x64 with SSE2 support',
            'req-ram': '4 GB minimum, 8 GB recommended for large parallel downloads',
            'req-disk': '100 MB for install + space for downloads',
            'req-net': 'Internet connection for downloading (no connection required for local operation)',
            'req-note': 'GNU build (rust-lld) works for tests; production MSVC/TAURI build needs Windows SDK (elevated install).',
            'privacy-title': 'Privacy',
            'privacy-core': '&ldquo;We do not send your download history, links, or files to our servers. The app contains no usage analytics, ads, or tracking. Download data is processed locally; external connections required for downloading or optional features are disclosed.&rdquo;',
            'privacy-practical': 'What this means in practice',
            'privacy-1': 'No device IDs, no fingerprinting, no session recording.',
            'privacy-2': 'No automatic crash uploads — error reporting is fully manual.',
            'privacy-3': 'No external fonts, no analytics libraries, no embedded chat widgets.',
            'privacy-4': 'Logs are local, deletable, and exportable after redacting sensitive data.',
            'privacy-5': 'Updates and plugin/theme catalog are opt-in by default.',
            'privacy-6': 'The file server sees a normal download connection; the site/GitHub host may process metadata per their policies.',
            'faq-title': 'FAQ',
            'faq-1-q': 'Is SAYL ready for daily use?',
            'faq-1-a': 'No. The project is in early specification and development (pre-M1). Core (sayl-core), HTTP (sayl-http), test server (sayl-test-http), and storage (sayl-storage) exist and are under test, but the desktop UI, browser integration, and advanced features are under development.',
            'faq-2-q': 'What features are actually implemented today?',
            'faq-2-a-items': [
                '11-state download state machine with guarded transitions (sayl-core, 33 tests)',
                'Classified, retryable error taxonomy (sayl-core)',
                'Bounded event bus with sequence numbering and gap detection (sayl-core)',
                'Idempotency keys with TTL (sayl-core)',
                'HTTP client with range validation, safe redirect tracking, safe filename extraction (sayl-http, 65 tests)',
                'Deterministic HTTP fixture server covering 27 failure modes (sayl-test-http, 25 tests)',
                'Crash-safe task store with WAL and atomic snapshots (sayl-storage, 18 tests)'
            ],
            'faq-3-q': 'What features are planned but not done?',
            'faq-3-a-label': '<strong>Under Development / Planned:</strong>',
            'faq-3-a-items': [
                'Tauri + Svelte desktop UI (Timeline, task list, themes, shortcuts)',
                'Scheduling, speed limits, queue, drop zone',
                'Browser integration (Native Messaging, Chromium/Firefox extensions)',
                'Video via yt-dlp/FFmpeg (playlists, conversion, merge)',
                'Torrent via librqbit (file selection, streaming/seek)',
                'WASM plugin system and plugin/theme catalogs',
                'Android HTTP Preview + SAF + background tasks',
                'Signed MSI/NSIS installers, signed auto-updates'
            ],
            'faq-4-q': 'How do I report a security issue?',
            'faq-4-a': 'See <a href="SECURITY.md">SECURITY.md</a> for instructions. Vulnerability reports are sent privately, not via public Issues.',
            'faq-5-q': 'Can I contribute?',
            'faq-5-a': 'Yes. See <a href="CONTRIBUTING.md">CONTRIBUTING.md</a> for GNU+rust-lld setup, test commands, commit style, and PR checklist.',
            'faq-6-q': 'What is the license?',
            'faq-6-a': 'Not decided yet. License line in README: <code>License: to be decided by the project owner</code>.',
            'footer-github': 'GitHub',
            'footer-issues': 'Issues',
            'footer-security': 'Security',
            'footer-contributing': 'Contributing',
            'footer-meta': 'Version <span id="footer-version-en">—</span> · Built <span id="footer-build-date-en">—</span> · No tracking · No analytics',
            'lang-toggle-label': 'Switch to Arabic',
            'lang-toggle-label-active': 'Switch to English',
            'artifact-status-checking': 'Checking...',
            'artifact-status-available': 'Available',
            'artifact-status-unavailable': 'This version has not been released yet',
            'artifact-status-error': 'Failed to check',
            'skip-link': 'Skip to main content'
        }
    };

    // ==========================================================================
    // State
    // ==========================================================================
    let currentLang = 'ar';
    let artifactCheckTimer = null;

    // ==========================================================================
    // DOM Helpers
    // ==========================================================================
    function $(selector, context = document) {
        return context.querySelector(selector);
    }

    function $$(selector, context = document) {
        return Array.from(context.querySelectorAll(selector));
    }

    function setText(element, text) {
        if (element) {
            element.textContent = text;
        }
    }

    function setHTML(element, html) {
        if (element) {
            element.innerHTML = html;
        }
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

        // Update all elements with data-ar/data-en attributes
        $$('[data-ar][data-en]').forEach(el => {
            const key = lang === 'ar' ? 'data-ar' : 'data-en';
            const text = el.getAttribute(key);
            if (text) {
                // Preserve child elements (like spans with IDs)
                if (el.childElementCount > 0) {
                    // For elements with children, we need to be more careful
                    // Just update text nodes directly
                    updateTextNodes(el, text);
                } else {
                    el.textContent = text;
                }
            }
        });

        // Update specific dynamic elements
        updateDynamicContent(t);

        // Update lang toggle button
        const langToggle = $('#lang-toggle');
        if (langToggle) {
            langToggle.setAttribute('aria-label', t['lang-toggle-label' + (lang === 'en' ? '-active' : '')]);
            langToggle.setAttribute('aria-pressed', lang === 'en');
            const currentSpan = langToggle.querySelector('.lang-current');
            if (currentSpan) {
                currentSpan.textContent = lang === 'ar' ? 'EN' : 'AR';
            }
        }

        // Update skip link
        const skipLink = $('.skip-link');
        if (skipLink) {
            skipLink.textContent = t['skip-link'];
        }

        // Update FAQ items (they have nested HTML)
        updateFAQContent(t);
    }

    function updateTextNodes(element, newText) {
        // Simple approach: if the element has only text content, replace it
        // If it has children, we don't modify to preserve structure
        if (element.childElementCount === 0) {
            element.textContent = newText;
        }
    }

    function updateDynamicContent(t) {
        // Hero meta version/build date placeholders
        const versionEl = $('#version-display');
        const buildDateEl = $('#build-date-display');
        const versionElEn = $('#version-display-en');
        const buildDateElEn = $('#build-date-display-en');
        const footerVersion = $('#footer-version');
        const footerBuildDate = $('#footer-build-date');
        const footerVersionEn = $('#footer-version-en');
        const footerBuildDateEn = $('#footer-build-date-en');

        // These are populated from build metadata if available
        // For now they show em dash as placeholder
    }

    function updateFAQContent(t) {
        // FAQ items need special handling due to nested HTML
        const faqItems = $$('.faq-item');
        const faqKeys = ['faq-1', 'faq-2', 'faq-3', 'faq-4', 'faq-5', 'faq-6'];

        faqItems.forEach((item, idx) => {
            const key = faqKeys[idx];
            if (!key || !t[key + '-q']) return;

            const summary = item.querySelector('summary');
            if (summary) {
                // Update summary text (first text node)
                const textNodes = [];
                const walker = document.createTreeWalker(summary, NodeFilter.SHOW_TEXT, null, false);
                let node;
                while ((node = walker.nextNode())) {
                    textNodes.push(node);
                }
                if (textNodes.length > 0) {
                    textNodes[0].textContent = ' ' + t[key + '-q'] + ' ';
                }
            }

            const answer = item.querySelector('.faq-answer');
            if (answer) {
                if (key === 'faq-2' && t['faq-2-a-items']) {
                    // Special handling for list
                    const itemsHtml = t['faq-2-a-items'].map(item => `<li>${item}</li>`).join('');
                    answer.innerHTML = `<ul>${itemsHtml}</ul>`;
                } else if (key === 'faq-3' && t['faq-3-a-items']) {
                    const label = t['faq-3-a-label'] || '';
                    const itemsHtml = t['faq-3-a-items'].map(item => `<li>${item}</li>`).join('');
                    answer.innerHTML = `<p class="planned-label">${label}</p><ul>${itemsHtml}</ul>`;
                } else if (t[key + '-a']) {
                    answer.innerHTML = t[key + '-a'];
                }
            }
        });
    }

    function toggleLanguage() {
        const newLang = currentLang === 'ar' ? 'en' : 'ar';
        applyLanguage(newLang);

        // Re-check artifacts after language change (they might have different status text)
        checkAllArtifacts();
    }

    // ==========================================================================
    // Artifact Availability Probing
    // ==========================================================================
    async function probeArtifact(url) {
        try {
            const response = await fetch(url, { method: 'HEAD', cache: 'no-cache' });
            if (response.ok) {
                return { available: true, status: response.status };
            } else if (response.status === 404) {
                return { available: false, status: 404 };
            } else {
                return { available: false, status: response.status };
            }
        } catch (e) {
            // Network error or CORS - treat as unavailable
            return { available: false, error: e.message };
        }
    }

    function updateArtifactButton(btn, result, t) {
        const labelEl = btn.querySelector('.artifact-label');
        const statusEl = btn.querySelector('.artifact-status');

        if (!statusEl) return;

        btn.removeAttribute('disabled');
        btn.classList.remove('disabled');
        statusEl.className = 'artifact-status';

        if (result.available) {
            btn.href = btn.getAttribute('data-artifact') || '#';
            statusEl.textContent = t['artifact-status-available'];
            statusEl.classList.add('available');
            btn.setAttribute('aria-disabled', 'false');
        } else {
            btn.href = '#';
            btn.setAttribute('aria-disabled', 'true');
            btn.classList.add('disabled');
            btn.setAttribute('disabled', '');

            if (result.status === 404 || (result.error && result.error.includes('404'))) {
                statusEl.textContent = t['artifact-status-unavailable'];
                statusEl.classList.add('unavailable');
            } else {
                statusEl.textContent = t['artifact-status-error'];
                statusEl.classList.add('unavailable');
            }
        }
    }

    async function checkAllArtifacts() {
        const t = translations[currentLang];
        const buttons = $$('.artifact-btn[data-artifact]');

        for (const btn of buttons) {
            const artifactPath = btn.getAttribute('data-artifact');
            if (!artifactPath) continue;

            // Show checking state
            const statusEl = btn.querySelector('.artifact-status');
            if (statusEl) {
                statusEl.textContent = t['artifact-status-checking'];
                statusEl.className = 'artifact-status checking';
            }

            const fullUrl = ARTIFACT_BASE + artifactPath;
            const result = await probeArtifact(fullUrl);
            updateArtifactButton(btn, result, t);
        }
    }

    function startArtifactPolling() {
        if (artifactCheckTimer) {
            clearInterval(artifactCheckTimer);
        }
        artifactCheckTimer = setInterval(checkAllArtifacts, CHECK_INTERVAL_MS);
    }

    function stopArtifactPolling() {
        if (artifactCheckTimer) {
            clearInterval(artifactCheckTimer);
            artifactCheckTimer = null;
        }
    }

    // ==========================================================================
    // FAQ Accordion Enhancement
    // ==========================================================================
    function enhanceFAQ() {
        $$('.faq-item summary').forEach(summary => {
            summary.addEventListener('keydown', (e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    summary.click();
                }
            });
        });
    }

    // ==========================================================================
    // Build Metadata (placeholder - would be injected at build time)
    // ==========================================================================
    function injectBuildMetadata() {
        // In a real build, these would be replaced with actual values
        const version = '—';
        const buildDate = '—';

        const versionEls = $$('#version-display, #version-display-en, #footer-version, #footer-version-en');
        const dateEls = $$('#build-date-display, #build-date-display-en, #footer-build-date, #footer-build-date-en');

        versionEls.forEach(el => el.textContent = version);
        dateEls.forEach(el => el.textContent = buildDate);
    }

    // ==========================================================================
    // Initialization
    // ==========================================================================
    function init() {
        // Apply initial language (Arabic)
        applyLanguage('ar');

        // Language toggle
        const langToggle = $('#lang-toggle');
        if (langToggle) {
            langToggle.addEventListener('click', toggleLanguage);
        }

        // FAQ enhancement
        enhanceFAQ();

        // Build metadata
        injectBuildMetadata();

        // Initial artifact check
        checkAllArtifacts();

        // Start polling
        startArtifactPolling();

        // Cleanup on page unload
        window.addEventListener('beforeunload', stopArtifactPolling);
    }

    // Run when DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();