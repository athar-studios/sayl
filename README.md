# سيل (SAYL) — مدير تحميل مستقل لـ Windows

[![CI](https://github.com/SAYL/SAYL/workflows/CI/badge.svg)](https://github.com/SAYL/SAYL/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](#license)

**سيل (SAYL)** مدير تحميل (Download Manager) مكتوب بلغة Rust، بواجهة سطح مكتب Tauri 2 + Svelte + TypeScript، **Windows أولًا**، مصمم للاستئناف الآمن، التنزيل المتوازي المُحقق، والخصوصية المطلقة.

> **حالة المشروع:** مواصفات وتطوير مبكر (ما قبل M1). النواة والمكتبات الأساسية موجودة وتخضع للاختبار؛ واجهة سطح المكتب وتكامل المتصفح والميزات المتقدمة قيد التطوير. راجع [خريطة الطريق](#خريطة-الطريق-roadmap) أدناه.

---

## ما هو سيل؟ (What is SAYL?)

| الجانب | الوصف |
|----------|---------|
| **الهدف** | مدير تحميل مستقل، مفتوح المصدر، يحترم الخصوصية، لـ Windows (ثم Linux/Android). |
| **المحرك** | Rust — `sayl-core` (نماذج/حالات/أحداث)، `sayl-http` (نقل HTTP، تحقق النطاقات)، `sayl-storage` (مخزن مهام آمن من التعطل)، `sayl-test-http` (خادم اختبارات حتمي لـ 27 وضع فشل). |
| **الواجهة** | Tauri 2 + Svelte 5 + TypeScript — عربية أولاً (RTL)، i18n، ثيمات، Timeline حقيقي من المحرك. |
| **الخصوصية** | لا تحليلات، لا تتبع، لا خطوط خارجية، لا رفع تقارير تعطل تلقائي. البيانات لا تغادر جهازك. |
| **السلامة** | استئناف مُحقق بالنطاقات (Range validation)، تحقق ETag قوي، أسماء ملفات آمنة، كتابة بـ fsync، استعادة بعد تعطل. |

---

## الحالة الحالية (Current Status)

| الحزمة (Crate) | الوصف | اختبارات (Tests) | الحالة |
|---------------|---------|------------------|--------|
| `sayl-core` | نماذج المجال، آلة الحالات (11 حالة)، نظام الأخطاء، ناقل الأحداث، مفاتيح عدم التكرار | **33** | ✅ تمر |
| `sayl-test-http` | خادم اختبارات HTTP حتمي، 27 وضع فشل، توليد بيانات حتمي | **25** | ✅ تمر |
| `sayl-http` | عميل HTTP، تحقق النطاقات، تتبع إعادة توجيه آمن، استخراج أسماء ملفات آمنة | **132** | ✅ تمر |
| `sayl-storage` | مخزن مهام WAL + لقطات ذرية، قفل دليل، استعادة بعد تعطل | **18** | ✅ تمر |
| `sayl-app` | المحرك، الجدولة، الإيجارات، الاسترداد بعد الانهيار | **46** | ✅ تمر |
| `sayl-cli` | واجهة سطر أوامر | **12** | ✅ تمر |
| `sayl-bridge` | بروتوكول Native Messaging + ثنائي `sayl-bridge` | **36** | ✅ تمر |
| `sayl-bench` | منصة قياس أداء محلية | **2** | ✅ تمر |
| `sayl-media` | مُحول yt-dlp/FFmpeg | — | 📋 مخطط (v0.2.0) |
| `sayl-torrent` | محول librqbit | — | 📋 مخطط (v0.4.0) |
| `sayl-plugin-host` | وقت تشغيل WASM، وسيط الصلاحيات | — | 📋 مخطط (v0.5.0) |
| `apps/desktop` | واجهة Tauri + Svelte (عربي RTL/إنجليزي) | **26** (vitest) | ✅ تُبنى وتُختبر — لم تُشغَّل نافذة Tauri هنا |
| `apps/website` | موقع تنزيل ثابت (هذا المستودع) | فحص ساكن + تدقيق خصوصية 4/4 | ✅ مكتمل |
| `extensions/browser` | ملحق Chromium/Edge MV3 (Native Messaging) | — | ✅ مبني — لم يُحمَّل في متصفح فعلي |

> **ملاحظة:** الأرقام أعلاه تعكس اختبارات الوحدة/التكامل الفعلية في المستودع. لا توجد اختبارات زائفة.

---

## البناء والتشغيل (Build & Run)

### المتطلبات

- **Rust:** `stable-x86_64-pc-windows-gnu` (GNU toolchain مع `rust-lld`)
- **Windows:** Windows 10 1903+ x64
- **للاختبارات فقط:** لا حاجة لـ Windows SDK ولا `link.exe` MSVC
- **للبناء الإنتاجي (Tauri/MSVC):** مطلوب Windows SDK (تثبيت مرفوع الامتيازات)

### أوامر موثقة (Verified Commands)

وفقًا لـ `docs/TOOLCHAIN.md`، البيئة الحالية تعمل مع GNU + `rust-lld`:

```powershell
# إعداد المسار والعلميات
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cd "C:\Users\Dragonfly\Downloads\Download Manager"

# فحص الأنواع
cargo +stable-x86_64-pc-windows-gnu check -p sayl-core
cargo +stable-x86_64-pc-windows-gnu check -p sayl-http
cargo +stable-x86_64-pc-windows-gnu check -p sayl-test-http
cargo +stable-x86_64-pc-windows-gnu check -p sayl-storage

# تشغيل الاختبارات
cargo +stable-x86_64-pc-windows-gnu test -p sayl-core
cargo +stable-x86_64-pc-windows-gnu test -p sayl-http
cargo +stable-x86_64-pc-windows-gnu test -p sayl-test-http
cargo +stable-x86_64-pc-windows-gnu test -p sayl-storage

# مساحة العمل كاملة
cargo +stable-x86_64-pc-windows-gnu test --workspace
```

> **تحذير:** البناء الإنتاجي لـ Tauri يتطلب toolchain MSVC مع Windows SDK. البناء GNU يعمل للاختبارات فقط.

---

## هيكل المستودع (Repository Layout)

```
.
├── apps/
│   ├── desktop/          # Tauri + Svelte Desktop UI (قيد التطوير)
│   └── website/          # موقع تنزيل ثابت (HTML/CSS/JS)
├── crates/
│   ├── core/             # sayl-core — نماذج، حالات، أحداث، أخطاء
│   ├── http/             # sayl-http — عميل HTTP، تحقق نطاقات، أسماء ملفات
│   ├── test-http/        # sayl-test-http — خادم اختبارات حتمي (27 وضع)
│   ├── storage/          # sayl-storage — WAL + لقطات ذرية، قفل دليل
│   ├── app/              # sayl-app — خدمات تطبيق، أوامر Tauri
│   ├── cli/              # sayl-cli — واجهة سطر أوامر
│   ├── media/            # sayl-media — yt-dlp/FFmpeg
│   ├── torrent/          # sayl-torrent — librqbit
│   └── plugin-host/      # sayl-plugin-host — WASM runtime
├── extensions/
│   └── browser/          # ملحقات المتصفح (Native Messaging)
├── catalog/
│   ├── plugins/          # بيانات إضافية مراجعة
│   └── themes/           # بيانات ثيمات
├── locales/              # موارد ترجمة نسخة
├── docs/
│   ├── MASTER-PLAN.md    # الخطة الموحدة للمنتج
│   ├── TOOLCHAIN.md      # إعداد سلسلة الأدوات
│   └── tasks/            # بطاقات مهام مفصلة
├── .github/
│   ├── workflows/ci.yml  # GitHub Actions CI
│   ├── ISSUE_TEMPLATE/   # نماذج Issues
│   └── PULL_REQUEST_TEMPLATE.md
├── CONTRIBUTING.md       # دليل المساهمة
├── SECURITY.md           # سياسة الأمان
├── CODE_OF_CONDUCT.md    # مدونة السلوك
├── .editorconfig         # تنسيق محرر موحد
├── .gitattributes        # خصائص Git
└── README.md             # هذا الملف
```

---

## خريطة الطريق (Roadmap)

### ✅ مكتمل /Implemented (ما قبل M1)

- **sayl-core**: آلة حالات 11 حالة، انتقالات محمية، أخطاء مصنفة، ناقل أحداث محدود السعة مع ترقيم تسلسلي، مفاتيح Idempotency مع TTL، معرفات مهام UUID v7.
- **sayl-test-http**: خادم fixture محلي، 27 وضع فشل (206 صحيح، لا Accept-Ranges، تجاهل Range، Content-Range فاسد، اقتطاع body، TCP reset، ETag متغير، ETag ضعيف، لا validator، 416، HEAD مرفوض، حجم مجهول، ملف فارغ، ترميز غير متوقع، 429 Retry-After، إعادة توجيه بأسرار، لا مساحة، إذن مرفوض، أسماء ملفات خبيثة، إيقاف أثناء probe، عامل بطيء، lease قديم، استعادة عند finalize، proxy/TLS، حدود أصل، اقتطاع ملف، pooling).
- **sayl-http**: تحقق النطاقات النشط (HEAD → GET range=0-0)، تصنيف الدعم (Supported/Ignored/Unsupported/Unknown)، تتبع إعادة توجيه آمن (حد 5 هوب، تجريد Authorization/Cookie عبر أصول مختلفة، قائمة بيضاء http/https، كشف حلقات)، استخراج أسماء ملفات آمنة (RFC 5987، منع اجتياز مسارات، أسماء أجهزة Windows محجوزة، تحكم، Unicode NFC، حد 255 حرف)، تدفق للملف مع تحقق الطول و fsync.
- **sayl-storage**: WAL بـ JSON-Lines، لقطات ذرية (tmp → fsync → rename)، تسامح مع ذيول مقطوعة، استعادة عبر snapshot + replay، قفل دليل حصري (Windows `share_mode(0)` + سجل في العملية).
- **apps/website**: موقع ثابت RTL-first، تبديل لغة، فحص توفر ملفات بـ HEAD، FAQ accordion، لا تبعيات خارجية.

### 🚧 قيد التطوير / Under Development

- **sayl-app / apps/desktop**: Tauri 2 + Svelte 5 UI — Timeline حقيقي (نطاقات/leases/checkpoints/splits/retries من المحرك)، بطاقات مهام، طابور، منطقة سحب، اختصارات عامة، ثيمات JSON schema، i18n كامل.
- **جدولة/طابور**: حد سرعة، جدولة زمنية، أولويات، إعادة تشغيل مهام فاشلة.
- **تكامل المتصفح**: Native Messaging bridge، ملحقات Chromium/Firefox، صفحة "تكامل المتصفح" داخل التطبيق.
- **دعم الفيديو**: yt-dlp/FFmpeg pinned، metadata، تقدم هيكلي، دمج/تحويل، قوائم تشغيل.
- **دعم التورنت**: librqbit — magnet/.torrent، DHT/PEX، اختيار ملفات، بث/Seek.
- **نظام الإضافات**: WASM runtime، hooks (extract، headers مقيدة، post-process، ui-tile declarative)، وسيط صلاحيات (DNS rebinding defense، allowlist شبكة، حدود ذاكرة/وقود/إخراج).
- **كتالوج الإضافات/الثيمات**: داخل التطبيق والموقع — اسم/مطور/إصدار/مصدر/رخصة/توافق/صلاحيات/مراجعة/تثبيت/إزالة.
- **Android**: Kotlin + UniFFI، HTTP Preview، SAF export، WorkManager/UIDT، إشعارات.
- **التوزيع**: MSI/NSIS موقعة، تحديثات Tauri موقعة، SBOM، فحص تراخيص التبعيات.

### 📋 مخطط / Planned (M5–M8 وما بعده)

- Safari extension (مرحلة macOS منفصلة).
- WASM plugins catalog مع remote refresh opt-in.
- مراجعة أمنية قبل الإصدار (Strix).
- مثبتات clean-machine، ترقية/إلغاء تثبيت نظيفة.
- Android media/torrent/plugins بعد spikes.

---

## المساهمة (Contributing)

راجع **[CONTRIBUTING.md](CONTRIBUTING.md)** لـ:

- إعداد بيئة GNU + `rust-lld`
- أوامر الاختبار والفحص
- نمط رسائل الكوميت (English imperative)
- قاعدة عدم الأوامر التدميرية لـ Git
- تشغيل اختبارات HTTP fixture الحتمية دون شبكة
- قائمة فحص Pull Request

---

## الأمان (Security)

راجع **[SECURITY.md](SECURITY.md)** لـ:

- الإصدارات المدعومة
- كيفية الإبلاغ عن ثغرة أمنية (خاصًا)
- داخل النطاق / خارج النطاق
- وعد عدم وجود تتبع/تحليلات
- المراجعة الأمنية قبل الإصدار

---

## مدونة السلوك (Code of Conduct)

راجع **[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)** — نسخة مختصرة على نمط Contributor Covenant.

---

## الرخصة (License)

**License: [Apache-2.0](LICENSE)**

لم تُحسم الرخصة بعد. القرار يعود لمالك المشروع.

---

## روابط مفيدة

- **الموقع:** `apps/website/` — افتح محليًا بـ `python -m http.server`
- **Issues:** [GitHub Issues](https://github.com/SAYL/SAYL/issues)
- **الأمان:** [SECURITY.md](SECURITY.md)
- **خطة المنتج:** [docs/MASTER-PLAN.md](docs/MASTER-PLAN.md)
- **سلسلة الأدوات:** [docs/TOOLCHAIN.md](docs/TOOLCHAIN.md)

---

## English Summary

**SAYL** is an independent, open-source, privacy-respecting download manager for Windows, written in Rust with a Tauri 2 + Svelte desktop UI. Arabic-first, RTL, i18n-ready.

### Current Status (v0.1.0, Windows-only)

| Crate | Description | Tests |
|-------|-------------|-------|
| `sayl-core` | Domain models, 11-state machine, error taxonomy, bounded event bus, idempotency keys | 33 |
| `sayl-test-http` | Deterministic HTTP fixture server, 27 failure modes | 25 |
| `sayl-http` | HTTP client, range validation, safe redirects, safe filenames | 132 |
| `sayl-storage` | Crash-safe task store (WAL + atomic snapshots), directory locking | 18 |
| `sayl-app` | Engine, scheduler, leases, crash recovery | 46 |
| `sayl-cli` | Command-line interface | 12 |
| `sayl-bridge` | Native-messaging protocol and `sayl-bridge` host binary | 36 |
| `sayl-bench` | Local performance harness | 2 |
| `apps/desktop` | Tauri + Svelte UI (Arabic RTL / English) | 26 (vitest) |
| `apps/website` | Static download site | static checks + privacy audit 4/4 |
| `extensions/browser` | Chromium/Edge MV3 extension with native messaging | built, not yet loaded in a real browser |

Video (`sayl-media`), torrent (`sayl-torrent`), WASM plugins (`sayl-plugin-host`) and Android are **not started** — each is planned in `docs/versions/`.

Total verified Rust tests: **304**.

### Build (Test Environment)

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cargo +stable-x86_64-pc-windows-gnu test --workspace
```

Production builds require MSVC toolchain + Windows SDK (elevated install).

### Links

- **Website:** `apps/website/` — `python -m http.server` to preview
- **Issues:** GitHub Issues
- **Security:** `SECURITY.md`
- **Roadmap:** `docs/MASTER-PLAN.md`
- **Toolchain:** `docs/TOOLCHAIN.md`

**License:** [Apache-2.0](LICENSE).