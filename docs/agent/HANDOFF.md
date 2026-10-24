# SAYL — HANDOFF (حالة التسليم)

> ملف التسليم الرسمي بين الجلسات. يُحدَّث في نهاية كل جلسة بأوامر ونتائج حقيقية فقط.
> آخر تحديث: 2026-10-24 · الفرع `master`

---

## 1. ملخص الحالة

| العنصر | القيمة |
|---|---|
| المشروع | سيل / SAYL — مدير تحميل Windows-first (Rust + Tauri 2 + Svelte) |
| المرحلة | M0 مكتملة · M1 قيد التنفيذ |
| آخر commit موثّق | `f245629` — STORE-001 |
| عدد الحزم | 9 (core, storage, http, app, cli, media, torrent, plugin-host, test-http) |
| الاختبارات الناجحة | core 33 · test-http 25 · http 65 · storage 18 = **141** |

---

## 2. حل مشكلة الـ Toolchain (قرار حاسم)

**المشكلة:** لم يكن ممكناً تشغيل أي اختبار تنفيذي. `cargo build` يفشل بـ `link.exe not found`، ثم بعد تثبيت MSVC بقيت المشكلة.

**ما تم التحقق منه فعلياً:**

1. MSVC VCTools **مثبّت** الآن: `14.44.35207`، و `link.exe`/`cl.exe` موجودان في
   `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\`.
2. لكن **Windows SDK غير مثبّت**: لا يوجد `kernel32.lib` في `C:\Program Files (x86)\Windows Kits\10` (يوجد `Catalogs` و `Redist` فقط).
3. النتيجة: ربط MSVC يفشل بـ `LINK : fatal error LNK1181: cannot open input file 'kernel32.lib'`.
4. الجلسة **غير مرتفعة الصلاحيات** (`IsAdmin: False`)، لذا لا يمكن تثبيت SDK من الطرفية.

**الحل العامل (مُتحقَّق منه):** استخدام toolchain GNU المثبّت مسبقاً مع `rust-lld` المرفق به:
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cargo +stable-x86_64-pc-windows-gnu test -p <crate>
```
التفاصيل الكاملة في [`docs/TOOLCHAIN.md`](../TOOLCHAIN.md).

**قيود مرتبطة يجب معرفتها:**
- **لا يوجد مجمّع C** (لا gcc/clang/cl صالح). لذلك: ممنوع `rusqlite`/`libsqlite3-sys`، ممنوع `ring`/`rustls-tls`/`openssl`.
- TLS يعمل عبر `native-tls` (SChannel — واجهة ويندوز الأصلية، بلا C). هذا هو المسار المعتمد في `crates/http`.
- ثنائي doctest الخاص بـ `sayl-http` ينهار عند التحميل (`0xc0000005`) لأنه يربط SChannel تحت mingw — لذلك `doctest = false` في `[lib]` مع بقاء نفس التأكيدات كاختبارات وحدة.
- **Smart App Control** في وضع التنفيذ (Enforce) يحجب أحياناً ثنائيات اختبار مبنية حديثاً برسالة
  `An Application Control policy has blocked this file (os error 4551)`. الحل العملي: حذف الثنائي وإعادة الربط، أو إعادة المحاولة.
- **بناء Tauri النهائي سيحتاج Windows SDK** (تثبيت واحد بصلاحيات مدير).

---

## 3. الأعطال الحقيقية التي كشفتها الاختبارات وأُصلحت

1. **`crates/core/src/state.rs`** — آلة الحالات كانت ترفض الانتقالات الذاتية للحالات غير النهائية، فسقط اختباران
   (`test_cancellation_paths` و `test_exhaustive_transition_matrix_properties`). أُصلح: `transition(from,to)` تعيد `Ok` عند
   `from == to` للحالة غير النهائية (no-op متكرر الأمان)، وتظل خطأ للحالة النهائية. المصفوفة الآن **33 انتقالاً صالحاً / 88 غير صالح**.
2. **`crates/test-http/src/server.rs`** — سيناريو `FileTruncation` كان يتجاهل نهاية النطاق المطلوبة فيعيد `bytes 0-511/512`
   بدل `bytes 0-99/512`. أُصلح بـ `req_end.min(shrunk_size - 1)`.
3. **`crates/test-http/Cargo.toml`** — مثال التوثيق يفشل لأن `tokio` بلا ميزة `macros`. أُضيفت.

---

## 4. الحزم المنجزة

### `crates/core` (CORE-001) — 33 اختباراً
`DownloadState` (11 حالة) + `transition()` محمية + property tests لكل الـ 121 احتمالاً · `CoreError` (7 فئات) مع
`is_retryable`/`is_fatal`/`is_user_cancelled` · `Event`/`EventKind` (8 أصناف) + قناة mpsc محددة السعة + `SequenceGapDetector` ·
`IdempotencyKey` + `IdempotencyStore` (TTL 24 ساعة) · `TaskId` (Uuid v7).

### `crates/test-http` (TEST-001) — 25 اختباراً
خادم HTTP حتمي على `127.0.0.1:0` يغطي **27 سيناريو فشل** من MASTER-PLAN §8 (`Ok206`, `NoAcceptRanges`, `IgnoredRange200`,
`BadContentRange`, `BodyTruncation`, `ConnectionReset`, `ETagChanged`, `WeakETagOnly`, `NoValidator`, `416`,
`HeadRejected`, `UnknownSize`, `EmptyFile`, `UnexpectedEncoding`, `429RetryAfter`, `RedirectWithSecrets`, `NoSpace`,
`PermissionDenied`, `MaliciousFilename`, `PauseCancelDuringProbe`, `SlowWorker`, `OldLease`, `RecoveryAtFinalize`, `ProxyTLS`,
`LimitsOrigin`, `FileTruncation`, `Pooling`) مع تسجيل الطلبات والتحقق من تسلسلها ومولّد جسم حتمي.

### `crates/http` (HTTP-001 + HTTP-002) — 65 اختباراً
`probe` (HEAD ثم GET مع `Range: bytes=0-0`) · `range` (`RangeSupport` وتحقق فعلي وتخفيض القدرة) · `redirect`
(حد 5 قفزات، حذف `Authorization`/`Cookie` عند تغيّر الأصل، رفض `file:`/`data:`، كشف الحلقات) · `filename`
(`Content-Disposition` + RFC 5987، تطهير `..` والمحارف المحجوزة وأسماء الأجهزة، سقف 255، NFC للعربية) · `stream`
(كتابة تدريجية، منع الإكمال الزائف، `Incomplete{written}`، إلغاء، fsync) · `resume` (قرارات الاستئناف الآمن:
ETag قوي فقط، ETag ضعيف لا يسمح أبداً، fallback لـ `Last-Modified`، تغيّر الحجم ⇒ إعادة من الصفر) · `retry`
(تراجع أسي + jitter + `Retry-After` بصيغتيه + عدم إعادة محاولة الملغى/القاتل).

### `crates/storage` (STORE-001) — 18 اختباراً
مخزن Rust خالص (لا SQLite — لا مجمّع C): journal append-only بصيغة JSON-Lines + لقطة ذرية + fsync + rename ذري ·
تجاهل tail تالف عند الإقلاع مع استرجاع السجلات السليمة · `compact()` يدمج ويقصّ · `TaskRecord` + `Checkpoint`
بخريطة نطاقات متعددة · قفل كاتب واحد · 10,000 upsert في 0.31 ثانية.

---

## 5. العوائق الحالية (Blockers)

1. **Windows SDK غائب** ⇒ بناء MSVC (وTauri النهائي) غير ممكن حتى تثبيت واحد بصلاحيات مدير.
2. **إثبات هوية الموديل** في تقارير `status=passed` قائم على متغيرات بيئة (`SAYL_PRIMARY_*`) وليس تشفيرياً
   (REPORT-CONTRACT.md:9). المهام السابقة حلّتها بـ `identity_source=runtime_metadata` مع تسجيل القيد.
3. **Smart App Control** يحجب ثنائيات جديدة أحياناً (يُعالج بإعادة الربط).
4. **لا مجمّع C** ⇒ استُبعد SQLite؛ إن لزم SQLite لاحقاً فيلزم توفر MSVC SDK/مجمّع C.

---

## 6. الخطوة التالية المؤهلة

1. `crates/app` — محرك التحميل: دورة حياة المهمة، الطابور، الجدولة بعدة اتصالات مع lease/ranges، ربط core+http+storage، بث الأحداث.
2. `apps/desktop` — Tauri 2 + Svelte: واجهة عربية RTL، بطاقات المهام، الخط الزمني، السحب/Spotlight.
3. `apps/website` — موقع التحميل (روابط حقيقية) + `docs/` + ملفات GitHub (README، LICENSE، CONTRIBUTING، SECURITY، ISSUE/PR templates، CI).
4. `SEC-STRIX-001` — فحص أمني في النهاية فقط، كما طلب المالك.

---

## 7. أوامر التحقق المعتمدة

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cd "C:\Users\Dragonfly\Downloads\Download Manager"

cargo +stable-x86_64-pc-windows-gnu test -p sayl-core       # 33 passed
cargo +stable-x86_64-pc-windows-gnu test -p sayl-test-http  # 25 passed
cargo +stable-x86_64-pc-windows-gnu test -p sayl-http       # 65 passed
cargo +stable-x86_64-pc-windows-gnu test -p sayl-storage    # 18 passed

python tools/workflow.py validate
python tools/workflow.py check-report reports/<task>/<role>.json
```

---

## 8. سجل الالتزامات (الأحدث أولاً)

| commit | الوصف |
|---|---|
| `7cfd571` | docs: user guide, architecture, security model, build, FAQ, roadmap, changelog + NSIS/WiX packaging and release scripts |
| `81984e3` | feat(desktop): Tauri 2 + Svelte Arabic-first desktop app (UI-001) |
| `9b508de` | feat(app): download engine (APP-001) |
| `c932e6c` | feat(site): Arabic RTL website + GitHub files (SITE-001) |
| `f245629` | feat(storage): crash-safe task store (STORE-001) |
| `98cdf36` | feat(http): validator-safe resume + retry policy; doctest scope off |
| `0570b61` | feat(http): HTTP-001 probe/range/redirect/filename/stream + native-tls |
| `e414a5a` | fix(core) self-transitions; feat(test-http) fixture server; docs(TOOLCHAIN) |
| `28013d8` | feat(core): CORE-001 state machine, errors, events, idempotency, TaskId |
| `2bb1410` | chore: BOOT-003 gate passed; M0 complete |
| `38be0fe` | BOOT-003 baseline |

---

## 9. حالة الإغلاق لمرحلة M1 (الجلسة الحالية)

**منجز ومُتحقَّق منه بأمر حقيقي:**
- `crates/storage` — 18 اختباراً (journal + لقطة + fsync + قفل كاتب واحد + 10,000 upsert في 0.31s).
- `crates/http` — 65 اختباراً؛ أُضيف `resume.rs` و `retry.rs` (ETag ضعيف لا يسمح بالاستئناف أبداً؛ `Retry-After` بصيغتيه؛ لا إعادة محاولة للملغى/القاتل).
- `crates/app` — 9 اختبارات تكامل: تنزيل كامل، اقتطاع، إيقاف/استئناف، تغيّر ETag، إلغاء idempotent، fallback، تنزيل متوازٍ بأربعة نطاقات مُدقَّقة، استعادة، حد التزامن.
- `apps/desktop` — 21 اختبار vitest + بناء Vite ناجح + `cargo check` نظيف؛ أسماء أوامر Tauri مطابقة لما تستدعيه الواجهة.
- `apps/website` — موقع عربي RTL بلا أي أصول خارجية، وأزرار تحميل تتحقق من وجود المخرجات فعلاً.
- الوثائق: 8 ملفات في `docs/` + خط أنابيب إصدار NSIS/WiX + سكربتات تحقق.

**قيود معلنة (لا تُخفى):**
- لا يمكن بناء EXE/MSI/NSIS هنا: Windows SDK غائب والجلسة غير مرتفعة.
- `cargo test -p sayl-desktop` لا يعمل محلياً (`STATUS_ENTRYPOINT_NOT_FOUND` بسبب WebView2Loader) — الاختبارات الثلاثة تعمل على CI/MSVC.
- لم يُشغَّل التطبيق فعلياً (لا webview)؛ الواجهة مُتحقَّق منها بالبناء والاختبارات الوحدوية والفحص الساكن فقط.
- Smart App Control يحجب بعض الثنائيات الجديدة (حل: حذف الثنائي لإعادة الربط).
- لم يُوثَّق ترخيص المشروع بعد: قرار المالك.
- لم تبدأ بعد: SCHED/PERF/REC/MEDIA/TORRENT/PLUGIN/CATALOG/PRIV/AND و`SEC-STRIX-001` (الأخير في النهاية كما طُلب).

## 10. الجولة 12 — الجدولة والأداء والخصوصية

**commits:** 8674255 (SCHED-001 leases) · ed378ce (HTTP-006 client/headers/budget) · 80e415 (PERF-001 bench) ·  5b89e9 (queue scheduler core) · 8e84e3c (PRIV-001) · cd98a1a (HTTP-007 wiring)

### ما أُنجز
- **SCHED-001**: crates/app/src/lease.rs — تقسيم ديناميكي بعقود إيجار، سرقة الإيجار المنتهي، رفض إكمال المالك القديم، و`assert_full_coverage`. 13 اختباراً.
- **HTTP-006**: `client.rs`/`headers.rs`/`budget.rs` — حجب بيانات اعتماد الـproxy في كل رسالة خطأ، منع حقن CRLF في الترويسات، رفض `Host`/`Content-Length`/`Transfer-Encoding`/`Proxy-Authorization`/`If-Range` من المستخدم، وحد اتصالات لكل أصل. sayl-http: 68 إلى 132 اختباراً.
- **HTTP-007**: `crates/app/src/queue.rs` (Scheduler + TokenBucket + ميزانية إعادة المحاولة) وربطها بـ`Engine`. 4 اختبارات ربط.
- **PERF-001**: حزمة `crates/bench` بأرقام مقيسة فعلاً و`docs/PERF.md` شفاف يصرّح بغياب المقارنة مع aria2.
- **PRIV-001**: `docs/PRIVACY.md` + `tools/privacy_audit.py` (4/4 PASS) + `docs/PRIVACY-AUDIT.md` + تحديث نص الخصوصية في الموقع.

### أخطاء حقيقية اكتُشفت وأُصلحت
1. `run_download` كان يعيد `Ok(())` حتى عند فشل التنزيل — كشفه اختبار جذع الملف.
2. `cancel()` كان يزيل خريطة الـscheduler قبل إلغائه، فلا يُلغى المهمة المنتظرة فعلياً في الـscheduler.
3. اختبار الإلغاء كان يعُدّ طلب `probe` الذي يجريه `Engine::add` ويعدّه تنزيلاً — صُحّح بأخذ خط أساس بعد `add`.

### فشل الأدوات المتكرر
ثلاث محاولات إيفاد لوحدة `queue.rs` أعادت صفر ملفات (استنفاد budget). كُتبت الوحدة يدوياً وأُعلن ذلك في التقرير.

### عقبات بيئية ما زالت قائمة
- Smart App Control يحجب أحياناً binaries حديثة الربط (`os error 4551`)؛ الحل المؤقت حذف `target\\debug\\deps\\sayl_app-*.exe` وإعادة الربط. هذا يجعل CI على runner عادي هو الإثبات الدائم.
- Windows SDK غائب: لا تطبيق فعلي ولا EXE/MSI/NSIS.

## 11. الجولة 23 — نطاق v0.1.0 (ويندوز فقط)

**commits:** `ba17d7a` (brand kit) · `3012861` (خطط الإصدارات) · `04e9764` (جسر المتصفح) · `dad6697` (هوية سطح المكتب) · `8ec3eda` (الموقع) · `5ecd8ab` (استثناء في أداة تدقيق الخصوصية)

### ما أُنجز ومُتحقَّق منه بأمر فعلي
- **محرك + CLI:** بلا تغيير منطقي هذه الجولة. sayl-core 33 · test-http 25 · http 132 · storage 18 · bench 2 · bridge 36 · cli 12.
- **sayl-app:** 14 (lib) + 13 (lease) + 6 (recovery) + 13 (integration) = **46** اختباراً. نجحت lib وlease هذه الجولة بعد انتظار سمة Smart App Control. `integration_tests` نجح 13/13 في هذه الجلسة على `2c49259`، ثم حجب SAC بصمة الثنائي بعد إعادة ربط ولم تنفتح خلال 16 محاولة؛ لم يتغير أي مصدر في ذلك الهدف بعده.
- **سطح المكتب:** 26 اختبار vitest + `npm run build` + `cargo check -p sayl-desktop`. أُضيفت: هوية سيل والشعار، تدرّج `#00B6BD → #0B1A4E`، خلفية نيلي `#0B1A4E`/`#070F2E`، شارة إصدار من `package.json`، أرقام ووحدات عربية (`52.5٪`, `ميجا/ث`, `3د 20ث`)، خط IBM Plex Sans Arabic محلي (400/600/700 woff2)، أيقونات نوع ملف، تأكيد الإلغاء، زر «فتح الملف»، ألسنة (الكل/جارية/مكتملة/متوقفة) وشريط ملخص، منتقي مجلد Tauri عبر `tauri-plugin-dialog`، نقل الاختصارات إلى الإعدادات، حالة فارغة بالشعار، سحب وإفلات رابط، وحركات تحترم `prefers-reduced-motion`. أيقونات Tauri استُبدلت من `assets/brand`.
- **المتصفح:** `crates/bridge` صار فيه `EngineDispatcher` مربوط بمحرك حقيقي + ثنائي `sayl-bridge`، وملحق Chromium/Edge MV3، وقالب manifest للـnative host، وسكربت `tools/register-native-host.ps1` (يرفض التسجيل إذا لم يوجد الملف التنفيذي — تم اختباره). 36 اختباراً ناجحاً.
- **الوثائق:** `docs/versions/README.md` + خمس خطط (v0.2.0 … v1.0.0) كل واحدة `PLAN.md` و`STATUS.md`، وتحديث CHANGELOG/USER-GUIDE/ROADMAP.
- **الموقع:** إصلاح التجاوب (قائمة موبايل، 360/768/1280)، إزالة النص الخام تحت زر التنزيل، أزرار تشير إلى `releases/latest` فقط، الهوية الجديدة والخط والألوان، وعدادات عند التمرير. تدقيق الخصوصية: **4/4 PASS** بعد إضافة استثناء موثّق واحد في `tools/privacy_audit.py` لقالب عنوان loopback يُبنى وقت التشغيل في `crates/bench`.

### ما لم يُنفَّذ ولِماذا (لا ادعاء)
- **لقطات قبل/بعد:** لم تُلتقط. التطبيق Tauri لا يعمل هنا (لا Windows SDK ولا WebView2 runtime)، ولا يوجد متصفح آلي لأخذ لقطة للموقع. المعاينة في hero الموقع هي **محاكاة HTML/CSS موسومة «معاينة الواجهة»** وليست لقطة شاشة.
- **مثبتات NSIS/MSI وSHA256SUMS.txt:** `tools/release.ps1 -DryRun` يمرّ بنجاح (exit 0) لكن الإنتاج الفعلي يحتاج تثبيت Windows SDK بصلاحيات مدير (إجراء المالك).
- **الملحق في متصفح حقيقي:** لم يُحمَّل. تم اختبار البروتوكول والـmanifests ورفض السكربت فقط.
- **منتقي المجلد وسحب/إفلات الحركات:** مُتحقَّق منها بالمصدر والفحص والبناء فقط، لا تنفيذاً.

### الخطوة التالية
1) المالك: تثبيت Windows SDK بصلاحيات مدير، ثم `pwsh -File tools/release.ps1` لإنتاج المثبتات والتجزئات.
2) نشر الإصدار على `github.com/ashrymahrm/sayl` حتى يعمل رابط `releases/latest` في الموقع.
3) بعده: v0.2.0 (yt-dlp/FFmpeg) وفق `docs/versions/v0.2.0/PLAN.md`.

## 12. الجولة 24 — الإغلاق الأمني لـ v0.1.0

**commits:** `522ed34` (CI للموقع/الواجهة/الخصوصية + أيقونات الملحق) · `923dd22` (اعتماد Tauri CLI + سير عمل الإصدار) · `46b2683` (إصلاح نتائج المراجعة الأمنية)

### ما أُضيف
- **GitHub Actions:** `ci.yml` صار 9 وظائف: أُضيف فحص الواجهة الأمامية (`npm ci` + `npm test` + `npm run build`)، وفحص الموقع والملحق (`node --check` + تحقّق JSON + `privacy_audit.py`) والتأكد من وجود ملفات خطط الإصدارات.
- **سير عمل إصدار:** `.github/workflows/release.yml` يشغّل `tools/release.ps1` على runner فيه Windows SDK، ويتحقق من وجود الـexe/msi/zip/SHA256SUMS ثم يرفعها كـartifact. الوظيفة التي تملك صلاحية الكتابة منفصلة (`attach-to-release`) ولا تنشر شيئًا ما لم يكن الإصدار موجودًا مسبقًا — النشر يبقى قرار المالك.
- **إصلاح عطل حقيقي:** `@tauri-apps/cli` لم يكن ضمن التبعيات، فكان `npm run tauri -- build` داخل `release.ps1` سيفشل. أُضيف الآن (`tauri-cli 2.12.4`، تحقّقت بـ `npx tauri --version`)، و`npm test` ما زال 26/26.
- **أيقونات الملحق:** استُبدلت بأيقونات العلامة بالمقاسات الصحيحة (16/32/48/128) بدل تكبير 128.

### المراجعة الأمنية المستقلة (v0.1.0)
التقرير في `docs/SECURITY-REVIEW-V0.1.0.md`: **2 متوسط + 3 منخفض، صفر حرج/عالي**. الخمسة كلها حقيقية وأُصلحت:
1. حقن مدخلات في `release.yml` عبر استيفاء `${{ inputs.version }}` و`${{ github.ref_name }}` داخل سكربت PowerShell ⇒ صارت تُمرَّر عبر `env:`.
2. `permissions: contents: read` كان سيفشل عنده `gh release upload` بخطأ 403 ⇒ نُقلت صلاحية الكتابة لوظيفة منفصلة.
3. سكربت التسجيل كان يقبل أي `ExtensionId` ⇒ صار يرفض ما لا يطابق `^[a-pA-P]{32}$` (مُختبَر: يرفض ولا يكتب أي مفتاح).
4. الملحق كان يطلب `storage` دون استخدامه ⇒ أُزيل.
5. الاستثناء الذي أضفته في `privacy_audit.py` كان عامًّا لنمط `{}` ⇒ صار محصورًا في `crates/bench/` بسبب موثّق، والتدقيق 4/4 PASS.

### ما لم يُثبت (لا ادعاء)
- لم يُشغَّل أي سير عمل على GitHub runner؛ التحقق YAML parsing فقط.
- لم يُنتج أي مثبت NSIS/MSI؛ ولا SHA256SUMS.txt حقيقي.
- التطبيق لم يُشغَّل، ولا الملحق في متصفح، ولا استُخدمت هجمات شبكية حية.
- لا توجد لقطات قبل/بعد: البيئة لا تسمح بتشغيل Tauri أو متصفح آلي.

### الخطوة التالية (إجراء المالك)
1) `git push` ثم `git tag v0.1.0 && git push origin v0.1.0` ⇒ سير عمل الإصدار يبني المثبتات على runner فيه SDK.
2) إنشاء GitHub Release للوسم حتى يرفع السير العمل الملفات، فيعمل رابط `releases/latest` في الموقع.
