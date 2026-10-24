# مراجعة أمان SAYL — تقييم مستقل

## 1. نطاق وطريقة العمل

مراجعة من نوع **read-only** على مشروع SAYL (مدير تنزيلات Rust، 9 حزم + تطبيق Tauri 2 + Svelte، موقع ويب، وبرمجيات إطلاق).

**ما تمت مراجعته:** جميع ملفات المصدر في `crates/{core,storage,http,app,cli,media,torrent,plugin-host,test-http}` و`apps/desktop/src-tauri/src/lib.rs` وملفات الواجهة الأمامية Svelte و`apps/website/app.js` و`index.html`. **Cargo.lock** (506 حزمة، 0 مصادر git).

**ما تم تشغيله:**
- `cargo +stable-x86_64-pc-windows-gnu test -p sayl-http` — 65 اختباراً نجحت (53 وحدة + 12 تكامل). ملحوظ: `doctest = false` مفعّل في `crates/http/Cargo.toml:11` بسبب انهيار native-tls/Schannel عند تشغيل doctests على الاستهداف GNU.
- `npm audit --json` في `apps/desktop` — 3 ثغرات، كلها في `devDependencies` (vitest، @vitest/mocker، tinypool).

**ما لم يتم تشغيله / لا يمكن إثباتها:** تحليل ديناميكي، fuzzing، اختبار شبكة، اختبار اختراق ثالث، تشغيل استغلاع Windows على صعيد التشفير بينية الفواصل. مراجعة read-only لا تبني الثبات التشغيلي.

---

## 2. جدول ملخص الحيثيات

| المعرّف | العنوان | الشدة | الموقع | الحالة |
|--------|---------|-------|--------|--------|
| S1 | لا يوجد Content-Security-Policy في إطار ويب Tauri | حرج | `apps/desktop/src-tauri/tauri.conf.json:23` (`"csp": null`) | مُؤكد |
| S2 | SSRF — لا يوجد حظر للعناوين الخاصة/الـ loopback عبر الـ redirect | عالي | `crates/http/src/redirect.rs:146-179`، `probe.rs:260-324` | مُؤكد |
| S3 | إخفاء اسم جهاز محجوز عبر مسافة (CON .txt) | متوسط | `crates/http/src/filename.rs:168-185` | مُؤكد |
| S4 | محارف تحكم bidi/RLO غير مفلترة في أسماء الملفات | متوسط | `crates/http/src/filename.rs:29-33` | مُؤكد |
| S5 | `.unwrap()` على مزيل mutex مسموح (poison) في مسار التحميل | متوسط | `crates/app/src/engine.rs:213` | مُؤكد |
| S6 | لا يعاد تعقيم اسم الملف عند التحميل من المخزن | منخفض | `engine.rs:316,528`، `lib.rs:335` | مُؤكد |
| S7 | لا توجد إعدادات إذن صريحة على ملفات المخزن | منخفض | `crates/storage/src/store.rs:368-377` | مُؤكد |
| S8 | لا يوجد تحقق من مسار download_dir في الإعدادات | منخفض | `apps/desktop/src-tauri/src/lib.rs:47-64` | مُؤكد |
| S9 | استخدام innerHTML في الموقع الإلكتروني (مع ترجمات ثابتة) | معلومات | `apps/website/app.js:257,366,370,372` | مُؤكد |
| S10 | روابط خارجية في الموقع تفتقر إلى rel="noreferrer" | معلومات | `apps/website/index.html:61,301,302` | مُؤكد |
| S11 | npm audit: 3 ثغرات، كلها في devDependencies | معلومات | `apps/desktop/package-lock.json` | مُؤكد |

---

## 3. تفاصيل الحيثيات

### S1 — لا يوجد Content-Security-Policy (حرج)

**الدليل:** `tauri.conf.json:23` — `"csp": null`. لا يوجد CSP في إطار الويب Tauri.

**التأثير:** تطبيق Tauri 2 يحمّل SPA Svelte من الموقع المحلي (`frontendDist: "../dist"`) داخل webview يملك وصولاً كاملاً إلى 9 أوامر Tauri — بما فيها `set_settings` (تكتب ملف إعدادات) و`reveal_task` (تفتح `explorer.exe` مع مسار ملف) و`open_download_folder`. بدون CSP، أي ثغرة XSS في الواجهة الأمامية — حتى من خلال ترميز Unicode في اسم مهمة يُظهره Svelte دون هروب — تصبح **تنفيذاً عن بُعد لجميع الأوامر**، بما فيها القراءة والكتابة على نظام الملفات. هذا يحوّل أي خط ثمبس XSS إلى RCE كامل.

**التوصية:** ضع CSP صريح في `tauri.conf.json`:
```json
"security": { "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'" }
```

### S2 — SSRF عبر المتابعة للتحويلات (عالي)

**الدليل:** `redirect.rs:146-179` — `RedirectTracker::step()` يتحقّق من الحد الأقصر للـ hops (5) ويتطابق المخطط (http/https فقط) ويزيل بيانات الاعتماد على النطاقات المتباعدة، لكنه **لا يتحقّق من هوست الموقع المستهدف**. لا توجد فحص لعناوين `127.0.0.1`، `169.254.169.254` (بيانات تعريف السحابة)، أو `0.0.0.0`. `probe.rs:285` و `probe.rs:318` ينفّذان المتابعة بنفس المنطق.

**التأثير:** خادم ضار يملك URL تنزيل يعيد توجيه المتصفّح إلى `http://127.0.0.1:8080/internal-proxy/secret`. عميل SAYL ينفّذ الطلب، يحصل على المحتوى الداخلي، ويحفظه على القرص كملف تنزيل. هذا SSRF فعال — يمكن استخدامه لتسريب بيانات من خدمات محلية أو استغلال صلاحيات الوصول إلى endpoints داخلية.

**التوصية:** أضف فحصاً للعناوين في `RedirectTracker::step()` — استخدم `socket::lookup_host` مع رفض العناوين في نطاقات `127.0.0.0/8`، `10.0.0.0/8`، `172.16.0.0/12`، `192.168.0.0/16`، `169.254.0.0/16`، والـ `[::1]`.

### S3 — إخفاء اسم جهاز محجوز عبر مسافة (متوسط)

**الدليل:** `filename.rs:168-174` — `neutralize_reserved_device_names()` يفصل الامتداد عبر `find('.')` ثم يقارن `base_stem` مع القائمة `WINDOWS_RESERVED_NAMES`. للسلسلة `"CON .txt"`، يصبح `base_stem = "CON "` (بمسافة لاحقة) ولا يطابق `"CON"`. ومع ذلك، Windows يقطع المسافات اللاحقة من اسم المكون قبل التحقّق من الأسماء المحجوزة، لذا `CON .txt` يُعامل كجهاز CON فعلي على Windows.

**التأثير:** خادم ضار يرسل `Content-Disposition: filename="CON .txt"` — فتُنشأ ملفاً كجهاز CON بدلاً من ملف عادي، مما قد يسبب سلوكاً غير متوقّع أو يتجاوز آليات فحص الأسماء.

**التوصية:** قطّع المسافات المحيطة من `base_stem` قبل المقارنة: `base_stem.trim().to_uppercase()`.

### S4 — محارف تحكم bidi/RLO غير مفلترة (متوسط)

**الدليل:** `filename.rs:29-33` — `is_disallowed_char()` يفحص الأحرف بين `0x00-0x1F` و`0x7F` فقط. محارف bidi مثل U+202E (RLO)، U+202B (RLE)، U+202D (LRO) تنجو بلا تصفية.

**التأثير:** اسم ملف مثل `file.txt‮exe.` (مع RLO) يُعرَض في Explorerr كـ `file.exe.txt` — إخفاء ملحوظ للامتداد الفعلي. هذا يُمكّن خداع المستخدم لتشغيل ملفات تنفيذية يعتقد أنها بيانات.

**التوصية:** أضف RLO/RLE/LRO/PDF إلى القائمة البيضاء المحظورة، أو استخدم `unicode-security` crate للكشف عن محارف bidi.

### S5 — `.unwrap()` على mutex poisoned (متوسط)

**الدليل:** `engine.rs:213` — `self.inner.active_tasks.lock().unwrap()`. إذا طرأ panic داخل مهمة أثناء حمل القفل (مثلاً من `total_size.unwrap()` على السطرين `engine.rs:405,693`)، يصبح الـ mutex مسموخاً وكل استدعاء لاحق `start()` يفشل بـ panic. باقي الدوال (`pause`، `cancel`) تستخدم `if let Ok(...)` أو `.lock().map_err(...)`.

**التأثير:** عطل واحد في مهمة واحدة يُعطّل جميع التنزيلات اللاحقة في كل جلسة — رفض خدمة شبه كامل.

**التوصية:** استبدل `.unwrap()` بنمط `.map_err(|_| ...)?` أو `if let Ok(guard) = ... else { return Err(...) }`.

### S6 — لا يعاد تعقيم المسار عند التحميل من المخزن (منخفض)

**الدليل:** `engine.rs:316-327` و `engine.rs:528` و `lib.rs:335` — في `start()` و `recover()` و `reveal_task`، يُستخدم `record.filename` و `record.save_path` مباشرة دون إعادة تعقيم. التعقيم يحدث فقط في `add()` (engine.rs:122-129) قبل الحفظ. ملف `snapshot.json` أو `journal.jsonl` المُخرب يحمل اسماً ملف ضاراً.

**التأثير:** حقن مسار ملف عبر تعديل الملفات المحفوظة. هجوم محلي فقط (يتطلّب كتابة على ملفات المخزن)، لكنه ينتهك مبدأ الدفاع في العمق.

**التوصية:** أعد استدعاء `sanitize_filename` على `record.filename` في `start()` و `recover()` و `reveal_task` قبل دمجه في المسار.

### S7 — لا توجد إعدادات إذن على ملفات المخزن (منخفض)

**الدليل:** `store.rs:368-371` — يفتتح `snapshot.tmp.<uuid>` عبر `OpenOptions::new().write(true).create_new(true)` بدون تعيين صلاحيات. كذلك `store.rs:84-99` و `journal.rs` لا تحدد صلاحياتاً صريحة. ملفات `snapshot.json`، `journal.jsonl`، `store.lock` تُنشأ بأذون صلاحية افتراضية للـ OS.

**التأثير:** على أنظمة غير Windows (حيث umask قد يسمح بقراءة الآخرين)، قد يتم الوصول غير المصرح به إلى بيانات المهام (الروابط، أسماء الملفات، إلخ). على Windows، الـ default ACLs عادةً مقيدة بالمستخدم فقط.

**التوصية:** استخدم `OpenOptionsExt::mode(0o600)` على Unix، أو `File::set_permissions` بعد الإنشاء.

### S8 — لا يوجد تحقق من مسار download_dir (منخفض)

**الدليل:** `lib.rs:47-64` — `validate_settings()` يتحقّق فقط من أن `download_dir` غير فارغ. يُسمح بأي سلسلة — بما فيها `..\..\..` أو مسارات نظام ملفات Windows. `lib.rs:335` (`reveal_task`) و `lib.rs:298-321` (`open_download_folder`) يمرران `download_dir` غير المتحقّق إلى `explorer.exe`.

**التأثير:** ملف إعدادات مُعدّل يحدد `download_dir` كمسار نظام — `open_download_folder` يفتحه في المستكشر. هجوم يتطلّب تعديل settings.json محلياً.

**التوصية:** استخدم `canonicalize()` وتحقّق أن المسار يقع ضمن الدليل المسموح.

### S9 — innerHTML في الموقع الإلكتروني (معلومات)

**الدليل:** `app.js:257` — تُعرّف `setHTML(element, html)` لكن **لا تُستدعى في أي مكان**. `app.js:366,370,372` — `answer.innerHTML = ...` يُستَخدم مباشرة مع سلاسل ترجمة مُعرّفة في المصدر (`translations` object في نفس الملف). الترجمات ثابتة ولا تأتي من مدخلات خارجية.

**التأثير:** لا يُستغل حالياً — الترجمات موثوقة. لكن النمط ضعيف إذا تم تحميله الترجمات من ملفات أو API خارجي.

**التوصية:** استخدم `textContent` أو Svelte's `{translation}` بدلاً من `innerHTML`.

### S10 — روابط خارجية بدون rel="noreferrer" (معلومات)

**الدليل:** `index.html:61,301,302` — جميع الروابط الخارجية (GitHub) تستخدم `rel="noopener"` لكنها **تفتقر إلى `rel="noreferrer"`**.

**التأثير:** رأس Referrer يُرسل إلى GitHub مع عنوان موقع SAYL. تأثير منخفض — لا يحتوي على بيانات حساسة.

**التوصية:** أضف `noreferrer` إلى كل رابط خارجي: `rel="noopener noreferrer"`.

### S11 — npm audit: 3 ثغرات في devDependencies (معلومات)

**الدليل:** `npm audit --json` — 3 ثغرات (1 متوسطة في `@vitest/mocker`، 2 حرجتين في `tinypool`). جميعها في `devDependencies` (vitest، @vitest/mocker، tinypool). `package.json` يحتوي على حزمة إنتاج واحدة فقط: `@tauri-apps/api` (بلا إصدارات ضعيفة). إجمالي 123 حزمة في `node_modules`، 122 منها dev-only.

**التأثير:** لا يؤثر على البناء الإنتاجي — الأدوات تطويرية فقط. إصلاح `@vitest/mocker` والثغرات الحرجة يتطلب ترقية vitest إلى 5.x (تغيير مُعكس).

**التوصية:** راقب ترقية vitest عند الانتقال إلى 5.x. لا تأثير أمني على الموقع أو التطبيق.

---

## 4. صلاحيات تم التحقّق منها (Verified Controls)

- مراجعة المخطط الأولي: `redirect.rs:110` `validate_scheme(&initial_url)` يرفض `file://`، `data:`، `javascript:` ✓
- حد أقصر 5 redirects: `redirect.rs:29` ✓
- إزالة بيانات الاعتماد بين المصادر المتباعدة: `redirect.rs:82-88` ✓
- اكتشاف الحلقات عبر مجموعة URL المزورّة: `redirect.rs:161-166` ✓
- منع اجتياز المسار (`..`) والمسافات المتسللة: `filename.rs:137-143` ✓
- تصفية الأحرف المحجوزة على Windows (`< > : " | ? * /`): `filename.rs:29-33` ✓
- مكافحة أسماء الأجهزة المحجوزة: `filename.rs:168-185` ✓ (مع انقحاب S3)
- حد أقصر 255 حرفاً: `filename.rs:164-165` ✓
- تطبيع NFC: `filename.rs:135` ✓
- رفض Content-Length السالب/غير الصالح: `probe.rs:242-258` ✓
- فحوصات Content-Range: `start > end` مرفوض (`range.rs:131-136`)، `end >= total` مرفوض (`range.rs:138-145`) ✓
- كشف الاقتطاع في التحميل المتوازي: `parallel.rs:142-151` (range lease)، `parallel.rs:177-184` (تحقق النطاق) ✓
- فحص الحجم النهائي: `parallel.rs:332-337`، `sequential.rs:263-281`، `engine.rs:510-521` ✓
- fsync قبل الإكمال: `sequential.rs:254`، `parallel.rs:305-308` ✓
- weak ETag لا يُفوّض If-Range: `resume.rs:96-100`، `resume.rs:72-74` ✓
- تغيير الحجم/الـ validator يُجبر على إعادة التشغيل: `resume.rs:102-140+` ✓
- إعادة المحاولة محدودة بالحد الأقصى، لا تعيد المحاولة على Cancelled/Fatal: `retry.rs:200-269` ✓
- تحليل Retry-After (delta-seconds + HTTP-date): `retry.rs:76-93` ✓
- التعامل مع ذيل المجلّد المقطوع في replay: `journal.rs` ✓
- لقطة ذرية عبر ملف مؤقت + إعادة تسمية: `store.rs:364-381` ✓
- قفل كاتب واحد على مستوى الـ OS: `store.rs:405-428` (Windows `share_mode(0)`) ✓
- معالجة mutex poisoned في store.rs: نمط `map_err(... StoreError::Other("Store mutex poisoned"))` ✓
- لا توجد مصادر git في Cargo.lock ✓
- لا توجد أسرار/مفاتيح خاصة في المصدر ✓
- لا توجد سكريبتات بناء في الـ crates الخاصة بالمشروع (أما build.rs الخاص بـ Tauri فهو tauri_build::build()) ✓
- لا توجد سكريبتات خارجية أو تحليلات أو خطوط في الموقع ✓
- جميع الروابط الخارجية في الموقع تحتوي على `rel="noopener"` ✓
- الواجهة الأمامية Svelte تستخدم نطاقات نصية (ليس innerHTML) ✓

---

## 5. مخاطر مقبولة معروفة

- لا توقيع رمزي (code signing) — المثبتات NSIS/MSI غير موقعة.
- اعتراف هوية النموذج على متغيّرات البيئة (`SAYL_PRIMARY_*`)، وليس تشفيرياً — انظر `docs/REPORT-CONTRACT.md:9`.
- `doctest` معطّل لحزمة sayl-http (`crates/http/Cargo.toml:11`) بسبب انهيار native-tls/Schannel على الاستهداف GNU.
- تداخل Smart App Control — قد يمنع تشغيل الـ binaries الجديدة (`os error 4551`).
- عدم وجود ملف LICENSE — الخيار يبقى على المالك.
- استخدام native-tls (Schannel) — لا C compiler، لذا لا ring/rustls/openssl.
- إصدار `uuid` مثبّت على `=1.9.0` (نسخة ثابتة).

---

## 6. الحدود الصريحة

- مراجعة read-only على مستوى المصدر — **لم تتم** تشغيل الـ binaries المدمجة (Tauri test executable) بسبب `STATUS_ENTRYPOINT_NOT_FOUND` على WebView2Loader (انظر STATUS.json).
- لم يتم التحقّق من الاستغلاب الديناميكي لأي حيثية — جميع النتائج بناءً على قراءة الشيفرة الكودية.
- لم يتم فحص بروتوكول TLS بشكل ديناميكي — الاعتماد على `reqwest 0.12.28` مع `native-tls` (Schannel) على Windows.
- لا يمكن إثبات أن ثغرة SSRF (S2) قابلة للاستغلال في بيئة معيّنة — يعتمد ذلك على بنية الشبكة المحلية.
- إصدارات الحزم المكررة (`reqwest 0.12.28` + `0.13.5`، `hyper 0.14.32` + `1.12.0`، `http 0.2.12` + `1.5.0`) — اقتياسية وعادية لبيئة Tauri 2، ولا تُعتبر ثغرة.

---

## 7. Accepted risks and residual exposure (المخاطر المقبولة والتعرض المتبقي)

- **S7 — وراثة ملفات المخزن لـ DACL الافتراضي (Store files inherit default DACL):**
  - **الأساس المنطقي (Rationale):** تطبيق SAYL يعمل كتطبيق مكتبي محلي مخصص للمستخدم الحالي (`%APPDATA%\SAYL`) على بيئة Windows NTFS، حيث ترث الملفات واللقطات المنشأة تلقائياً قوائم التحكم بالوصول التقديرية (Default DACL) المقيدة بحساب المستخدم ومسؤولي النظام فقط، مما يمنع وصول المستخدمين غير المصرح لهم على الجهاز نفسه.
  - **الخطر المتبقي (Residual Risk):** في البيئات متعددة المستخدمين على أنظمة غير Windows، أو عند حفظ المخزن على وحدات تخزين خارجية لا تدعم الـ ACLs (مثل FAT32/exFAT)، قد ترث الملفات أذونات أكثر مرونة بحسب `umask`. يظل الخطر محلياً بحتاً ويتطلب وصولاً فيزيائياً أو محلياً لنظام الملفات.

- **S9 — استخدام innerHTML مع ترجمات ثابتة في الموقع (Website innerHTML with hardcoded translations):**
  - **الأساس المنطقي (Rationale):** جميع النصوص الممررة إلى `innerHTML` في `apps/website/app.js` مشتقة حصراً من كائن قاموس الترجمة الثابت المدمج في الشيفرة المصدرية، ولا تقبل أو تدمج أي مدخلات من المستخدم أو معلمات URL أو استجابات لشبكة خارجية.
  - **الخطر المتبقي (Residual Risk):** الثغرة غير قابلة للاستغلال حالياً لانعدام وجود ناقل إدخال غير موثوق. يقتصر الخطر المتبقي على الصيانة المستقبلية إذا تم تحويل نظام الترجمة لتحميل نصوص من مصادر خارجية ديناميكية دون استبدال `innerHTML` بـ `textContent` أو تعقيم مسبق.
