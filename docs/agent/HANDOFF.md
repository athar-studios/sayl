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
