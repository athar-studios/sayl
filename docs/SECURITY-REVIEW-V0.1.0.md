# تقرير المراجعة الأمنية المستقلة — SAYL v0.1.0 (إصدار Windows)

## 1. نطاق العمل والمنهجية والحدود الصريحة

أُجريت هذه المراجعة الأمنية كتقييم **مستقل وقائم على القراءة فقط (Read-Only Source Code Review)** لمستودع SAYL Download Manager تمهيداً لإطلاق الإصدار `v0.1.0` لنظام Windows.

### النطاق المفحوص برمجياً:
1. **الوسيط وحزم الرسائل المحلية:** `crates/bridge/src/{protocol.rs, message.rs, host.rs, validate.rs, dispatch.rs, main.rs}`.
2. **إضافات المتصفح وإعدادات المضيف:** `extensions/browser/chromium/{manifest.json, background.js, popup.js, popup.html}` و `extensions/browser/host/com.sayl.downloader.json`.
3. **سكربت تسجيل المضيف المحلي في سجل Windows:** `tools/register-native-host.ps1`.
4. **تطبيق سطح المكتب Tauri وواجهة Svelte:** `apps/desktop/src-tauri/src/lib.rs`، `apps/desktop/src-tauri/capabilities/default.json`، `apps/desktop/src-tauri/tauri.conf.json`، `apps/desktop/src/lib/tauri.js`، `apps/desktop/src/App.svelte`، `apps/desktop/src/lib/SettingsPanel.svelte`.
5. **موقع الويب والترجمة:** `apps/website/{index.html, app.js, styles.css}`.
6. **سير العمل وأتمتة الإطلاق:** `.github/workflows/{ci.yml, release.yml}`، `tools/release.ps1`.
7. **فحص الخصوصية والأسرار المتقاطعة:** `tools/privacy_audit.py`، `assets/brand`.

### الحدود الصريحة وما لم يتم اختباره في هذه البيئة:
- **لم يتم تشغيل متصفح تفاعلي حقيقي**، ولم يتم تحميل إضافة المتصفح داخل Chrome أو Edge لاختبار تكامل Native Messaging الحي، لعدم توفر متصفح مرئي في هذه الحاوية.
- **لم يتم تشغيل نافذة Tauri أو محرك WebView2 تفاعلياً**، ولم يُختبر تفاعل السحب والإفلات أو حوارات النظام بنوافذ حية.
- **لم يتم تنفيذ هجمات شبكية حية أو Dynamic Fuzzing أو اختبار اختراق طرف ثالث**؛ التقييم يعتمد حصراً على التدقيق البرمجي الرياضي والتتبعي للمدخلات والمسارات والتحقق الساكن من الشيفرة.

---

## 2. إعادة التحقق من حيثيات التقرير السابق (S1–S11)

تمت مراجعة نتائج تقرير الأمان السابق (`docs/SECURITY-REVIEW.md`) والتحقق من حالتها في الشيفرة المصدرية الحالية:

| المعرّف | العنوان في المراجعة السابقة | الحالة الحالية | الدليل من الكود الحالي |
|---------|-----------------------------|----------------|------------------------|
| **S1** | غياب Content-Security-Policy في إطار Tauri | **تم الإصلاح (FIXED)** | `apps/desktop/src-tauri/tauri.conf.json:22-25` يُلزم سياسة CSP صارمة تمنع `object-src 'none'` وتحدد `script-src 'self'`. |
| **S2** | SSRF — عدم حظر العناوين الخاصة/loopback عند التحويل | **تم الإصلاح (FIXED)** | `crates/http/src/redirect.rs:125-126, 216-217, 269-324` يفحص النطاقات وعناوين IP الخاصة وشبكات الاسترجاع ويمنع التحويل إليها. |
| **S3** | إخفاء اسم جهاز محجوز في Windows عبر مسافة (`CON .txt`) | **تم الإصلاح (FIXED)** | `crates/http/src/filename.rs:170, 186-203` يقوم بتشذيب المسافات والنقاط وتوحيد المسافات المتكررة قبل مطابقة أسماء الأجهزة المحجوزة. |
| **S4** | محارف تحكم bidi/RLO غير مفلترة في أسماء الملفات | **تم الإصلاح (FIXED)** | `crates/http/src/filename.rs:31-46` يرفض صراحة محارف BiDi من U+202A حتى U+202E و U+2066 حتى U+2069 و U+FEFF وغيرها. |
| **S5** | `.unwrap()` على mutex مسموح (poison) في مسار التنزيل | **تم الإصلاح (FIXED)** | `crates/app/src/engine.rs:38, 892, 952` استبدلت بالكامل بنمط `if let Ok(...)`. |
| **S6** | عدم إعادة تعقيم اسم الملف عند التحميل من المخزن | **معالج في المحرك** | `crates/app/src/engine.rs:393, 1299` يستدعي صراحة `sanitize_filename` عند بدء المهمة وعند التعافي من المخزن. |
| **S7** | وراثة ملفات المخزن لـ DACL الافتراضي للنظام | **خطر مقبول موثق** | انظر البند 7 من التقرير السابق؛ البيئة المستهدفة Windows NTFS محصورة بحساب المستخدم الحالي. |
| **S8** | غياب التحقق من مسار `download_dir` في الإعدادات | **تم الإصلاح (FIXED)** | `apps/desktop/src-tauri/src/lib.rs:52-100, 104` دالة `validate_download_dir` تمنع المسارات النسبية وجذور الأقراص و `..` والتكرار. |
| **S9** | استخدام `innerHTML` في الموقع الإلكتروني | **خطر مقبول موثق** | `apps/website/app.js:161, 167, 187, 191, 193` يعتمد حصراً على نصوص ترجمة ثابتة مدمجة بدون مدخلات خارجية. |
| **S10** | روابط خارجية تفتقر إلى `rel="noreferrer"` | **تم الإصلاح (FIXED)** | `apps/website/index.html:77, 78, 343, 344, 345, 354, 450, 451` جميع الروابط الخارجية أصبحت تحوي `rel="noopener noreferrer"`. |
| **S11** | ثغرات في حزم التطوير `devDependencies` | **خطر مقبول موثق** | حزم اختبار وتطوير لا تُضمّن في حزمة الإنتاج. |

**الخلاصة:** لا توجد أي انتكاسات (Regressions) على أي من الإصلاحات السابقة.

---

## 3. الحيثيات الأمنية الجديدة المرصودة في الإصدار v0.1.0

### الحيثية 1: إمكانية حقن أوامر/نصوص في سياق Shell عبر استيفاء متغيرات غير موثوقة في GitHub Actions
- **الموقع:** `.github/workflows/release.yml:44-48` و `.github/workflows/release.yml:119`
- **الشدة:** **متوسطة (Medium)**
- **التشخيص والأثر (Reasoning & Impact):**
  في سير العمل `release.yml`، يُدمج التعبير `${{ inputs.version }}` والتعبير `${{ github.ref_name }}` نصياً وبشكل مباشر داخل نص سكربت PowerShell المنفذ بواسطة المشغل:
  ```yaml
  run: |
    if ("${{ github.event_name }}" -eq "workflow_dispatch") {
      $v = "${{ inputs.version }}"
    } else {
      $v = "${{ github.ref_name }}" -replace '^v', ''
    }
  ```
  وفي خطوة الرفع:
  ```yaml
  run: |
    $tag = "${{ github.ref_name }}"
  ```
  في محرك GitHub Actions، يتم استبدال تعابير `${{ ... }}` قبل أن يقوم مفسر الأوامر (`pwsh`) بتحليل الشيفرة. إذا تم تشغيل سير العمل يدوياً أو عبر وسم يحتوي على محارف تحكم خاصة بلغة PowerShell (مثل علامات اقتباس مزدوجة، أو علامة الفاصلة المنقوطة `;`، أو استدعاء فرعي `$()`)، فسيتم تفسيرها كأوامر برمجية قابلة للتنفيذ قبل الوصول إلى فحص التعبير النمطي في السطر 49 (`if ($v -notmatch '^\d+\.\d+\.\d+$')`).
- **الإصلاح المقترح (Specific Fix):**
  تمرير المدخلات ومعلومات الوسم عبر متغيرات البيئة (`env`) الخاصة بالخطوة، بدلاً من الدمج المباشر داخل نص الأوامر:
  ```yaml
  - name: Resolve version
    id: ver
    shell: pwsh
    env:
      INPUT_VERSION: ${{ inputs.version }}
      REF_NAME: ${{ github.ref_name }}
      EVENT_NAME: ${{ github.event_name }}
    run: |
      if ($env:EVENT_NAME -eq "workflow_dispatch") {
        $v = $env:INPUT_VERSION
      } else {
        $v = $env:REF_NAME -replace '^v', ''
      }
      if ($v -notmatch '^\d+\.\d+\.\d+$') {
        Write-Error "Invalid SemVer version '$v'"
        exit 1
      }
      "version=$v" >> $env:GITHUB_OUTPUT
  ```

---

### الحيثية 2: تعارض الصلاحيات يمنع رفع المخرجات إلى الإصدارات الرسمية
- **الموقع:** `.github/workflows/release.yml:26-27` مقارنة بالأسطر `113-124`
- **الشدة:** **متوسطة (Medium)**
- **التشخيص والأثر (Reasoning & Impact):**
  يحدد سير العمل في السطر 26 أذونات عامة مقيدة:
  ```yaml
  permissions:
    contents: read
  ```
  بينما تستدعي الخطوة الأخيرة أمر `gh release upload`:
  ```yaml
  - name: Attach artefacts to an existing GitHub Release
    if: startsWith(github.ref, 'refs/tags/v')
    shell: pwsh
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    run: |
      gh release upload $tag releases/... --clobber
  ```
  أداة `gh release upload` تتطلب صلاحية كتابة المحتوى (`contents: write`) لرفع الملفات والملحقات إلى الإصدار المنشور. مع اقتصار الإذن على `contents: read`، سيفشل المشغل بخطأ منع الوصول `HTTP 403: Resource not accessible by integration` عند محاولة رفع الحزم للمستودع، مما يعطل عملية تسليم حزم v0.1.0 الرسمية.
- **الإصلاح المقترح (Specific Fix):**
  إما منح إذن `contents: write` حصراً في وظيفة البناء (`build-installers`) أو توفير وظيفة نشر منفصلة تنفذ عند توفر وسم معتمد بصلاحية `contents: write`.

---

### الحيثية 3: غياب التحقق من سلامة معرّف الإضافة `ExtensionId` في سكربت التسجيل
- **الموقع:** `tools/register-native-host.ps1:28-32` و `tools/register-native-host.ps1:96-100`
- **الشدة:** **منخفضة (Low)**
- **التشخيص والأثر (Reasoning & Impact):**
  في السكربت `register-native-host.ps1`:
  1. المعامل `$ExtensionId` اختياري (`Mandatory = $false`). إذا شغّل المستخدم السكربت دون تمريره، يُسجل ملف البيان في الـ Registry مع الإبقاء على القيمة المؤقتة `"chrome-extension://REPLACE_WITH_EXTENSION_ID/"`، مما يجعل عملية التسجيل غير فعالة للمتصفح.
  2. لا يتحقق السكربت من أن معرّف الإضافة يتوافق مع نسق معرّفات Chromium القياسية المكونة من 32 حرفاً هجائياً من `a` إلى `p` حصراً (`^[a-p]{32}$`). تمرير معرّف غير نظامي قد يفسد ملف الـ JSON أو يسمح بقبول أصول غير معتمدة إذا أدخل المستخدم قيمة خاطئة أو عامة.
- **الإصلاح المقترح (Specific Fix):**
  التحقق الصارم من صحة المعرّف قبل حقنه في ملف البيان:
  ```powershell
  if (-not [string]::IsNullOrWhiteSpace($ExtensionId)) {
      if ($ExtensionId -notmatch '^[a-p]{32}$') {
          Write-Error "Invalid Chromium extension ID format: '$ExtensionId'. Must be 32 lowercase letters (a-p)."
          exit 1
      }
      $newOrigin = "chrome-extension://$ExtensionId/"
      $manifestData.allowed_origins = @($newOrigin)
      $dirty = $true
  }
  ```

---

### الحيثية 4: تصريح غير مستخدم بإذن `storage` في إضافة المتصفح ينتهك مبدأ الصلاحية الأقل
- **الموقع:** `extensions/browser/chromium/manifest.json:10`
- **الشدة:** **منخفضة (Low)**
- **التشخيص والأثر (Reasoning & Impact):**
  يصرّح بيان الإضافة بالصلاحيات التالية:
  ```json
  "permissions": [
    "nativeMessaging",
    "contextMenus",
    "notifications",
    "storage"
  ]
  ```
  بمراجعة الشيفرة البرمجية في `extensions/browser/chromium/background.js` و `popup.js`، تبيّن أن واجهة برمجة التطبيقات `chrome.storage` لا تُستدعى على الإطلاق في أي موضع. طلب صلاحية تخزين غير مستخدمة ينتهك متطلبات المتجر (Chrome Web Store Least Privilege Guidelines) ويثير مخاوف أمنية لا مبرر لها لدى المستخدمين.
- **الإصلاح المقترح (Specific Fix):**
  إزالة إذن `"storage"` من مصفوفة `permissions` في `manifest.json`.

---

### الحيثية 5: استثناء قالب الرابط (`{}`) في أداة فحص الخصوصية يمثل نقطة عمياء في التحليل الساكن
- **الموقع:** `tools/privacy_audit.py:120` مقارنة بـ `crates/bench/src/lib.rs:431`
- **الشدة:** **منخفضة (Low)**
- **التشخيص والأثر (Reasoning & Impact):**
  أُضيف النمط التالي مؤخراً إلى قائمة الروابط المسموحة في أداة التدقيق:
  ```python
  r"^https?://(?:\{\}|\{:\d+\})$",
  ```
  جاء هذا الاستثناء لتمرير السطر 431 في `crates/bench/src/lib.rs` (`let server_url = format!("http://{}", addr);`).
  **تقييم الأثر:** نظراً لأن أداة التدقيق تبحث عن الروابط عبر التعبير النمطي `https?://[^\s"\'`<>)]+` في الشيفرة الساكنة، فإن السماح بنمط `http://{}` يتيح لأي مطور (عن قصد أو غير قصد) في شيفرات التطبيق بناء رابط خارجي ديناميكي عبر المتغيرات مثل:
  ```rust
  let remote_telemetry = "telemetry.evil.com";
  let url = format!("http://{}", remote_telemetry);
  ```
  في هذه الحالة، ستتطابق السلسلة النصية الساكنة `"http://{}"` مع الاستثناء وتجتاز الفحص بنجاح (PASS)، بينما النطاق الفعلي مفصول في متغير آخر، مما يخلق ثغرة تغطية (Blind Spot) في أداة التحقق من الخصوصية.
- **الإصلاح المقترح (Specific Fix):**
  حزمة `crates/bench` هي منصة قياس أداء (Benchmark) وليست تطبيق إنتاج نهائي. الأفضل استثناء مجلد `crates/bench/` ضمن دالة `is_application_code()` في أداة الفحص، وإلغاء هذا الاستثناء العام الشامل لقوالب العناوين من `ALLOWED_URL_HOST_PATTERNS`.

---

## 4. المراجعة التفصيلية للأسطح السبعة

### السطح 1: وسيط الرسائل الأصلي (`crates/bridge`)
- **حماية الذاكرة والتحجيم:**
  - `crates/bridge/src/protocol.rs:9`: الحد الأقصى محدد بـ `MAX_MESSAGE_BYTES = 1_048_576` (1 ميجابايت).
  - `crates/bridge/src/protocol.rs:105-108`: يتحقق التابع `decode_frame` من طول الإطار فور قراءة بادئة الطول وقبل تخصيص أي مصفوفة ذاكرة (`vec![0u8; declared]`). إذا تجاوز الطول المصرح به الحد، يُرفض الطلب فوراً بخطأ `BridgeError::FrameTooLarge` دون استهلاك موارد الذاكرة.
- **التكامل مع المحرك وتحقق المدخلات:**
  - `crates/bridge/src/dispatch.rs:110-120`: لا يتم استدعاء `self.engine.add(&url)` إلا بعد اجتياز فحص `validate_request_url` وفحص `validate_suggested_filename`.
  - `crates/bridge/src/validate.rs:67-91`: يتحقق `validate_suggested_filename` من خلو الاسم من محارف التحكم، ويخضعه لدالة `sanitize_filename`، ويمنع الخروج عبر المكونات النسبية (`..` و `.`) ويتحقق من أن كل مكون مسار هو `Component::Normal(_)`.
  - ملاحظة تشغيلية: الدالة `engine.add(&url)` تعتمد على استكشاف الرابط (probe) واشتقاق الاسم بشكل ذاتي آمن.
- **الشبكات المحلية وتخطي الصلاحيات:**
  - `crates/bridge/src/dispatch.rs:74-76`: القيمة الافتراضية لـ `allow_private_hosts` هي `false`. لا يتم تفعيلها إلا إذا تم ضبط متغير البيئة `SAYL_ALLOW_PRIVATE_HOSTS` صراحة على `"1"` أو `"true"`.
  - `crates/bridge/src/validate.rs:55-59`: يرفض أي استهداف لعناوين الـ loopback أو الشبكات الخاصة في الوضع الافتراضي عبر `is_private_or_loopback_host`.
- **سلامة دفق stdout:**
  - `crates/bridge/src/main.rs:16, 22`: جميع المخرجات إلى `stdout` مؤطرة ببروتوكول موحد عبر `write_message` (`protocol.rs:71-79`) المكون من 4 بايت للطول ثم كائن JSON. لا توجد أي طباعة عشوائية عبر `println!` تفسد التزامن مع المتصفح.
- **استقرار المعرّفات:**
  - `crates/bridge/src/dispatch.rs:128-136`: إرسال `task_id` غير صالح لا يسبب انهياراً (`panic`)، بل يتم التقاطه عبر `parse::<TaskId>()` وإرجاع كائن خطأ `Response::error("unknown_task", ...)`.
  - `crates/bridge/src/dispatch.rs:156-159`: المعالج محمي صراحة بـ `std::panic::catch_unwind` لمنع انهيار العملية في حال حدوث أي استثناء داخلي غير متوقع.

### السطح 2: إضافة المتصفح ومضيف الاتصال (`extensions/browser`)
- **الصلاحيات والحد الأدنى (Manifest V3):**
  - الصلاحيات المعلنة هي `nativeMessaging` و `contextMenus` و `notifications` و `storage`.
  - تم التحقق من حاجة `nativeMessaging` للتواصل مع الوسيط، و `contextMenus` لزر التنزيل بالزر الأيمن، و `notifications` لإخطار المستخدم بالقبول أو الفشل.
  - صُنف إذن `storage` كصلاحية زائدة لعدم وجود أي استدعاء له (انظر الحيثية 4).
- **حقن المدخلات وعدم استخدام `innerHTML`:**
  - `extensions/browser/chromium/popup.js:14, 15, 23, 24, 25, 32, 33, 34, 38, 39, 40`: جميع التحديثات للواجهة تتم حصراً عبر `.textContent`. لا يوجد أي استخدام لـ `innerHTML` في الإضافة.
  - الرابط المأخوذ من الصفحة عبر النقر على القائمة السياقية يمر إلى الوسيط (`background.js:60`) الذي يُخضعه للتحقق الصارم في لغة Rust.
- **التعامل مع أخطاء وقت التشغيل (`lastError`):**
  - تم استهلاك `chrome.runtime.lastError` في جميع استدعاءات ردود النداء (Callbacks): `background.js:15` (الإشعارات)، `background.js:22` (Ping)، `background.js:40` (إنشاء القائمة)، `background.js:65` (إرسال الرسائل الأصلية)، و `popup.js:20` (فحص الحالة).
- **انعدام التنزيل التلقائي:**
  - لا يبدأ التنزيل إلا عند قيام المستخدم بالنقر المباشر والصريح على خيار "تنزيل باستخدام سيل" من القائمة السياقية (`background.js:53`). أحداث التثبيت وبدء التشغيل تنفذ فقط عملية `Ping` استطلاعية (`background.js:42, 46`).

### السطح 3: سكربت تسجيل المضيف (`tools/register-native-host.ps1`)
- **التحقق من وجود المسارات:**
  - `tools/register-native-host.ps1:60-63`: يتحقق من وجود ملف البيان على القرص عبر `Test-Path -LiteralPath $ManifestPath` ويرفض المتابعة في حال غيابه.
  - `tools/register-native-host.ps1:82-85`: يتحقق من وجود الملف التنفيذي للمضيف على القرص ويرفض التسجيل في حال عدم وجوده.
  - تم تسجيل ملاحظة عدم التحقق من صحة معرّف الإضافة كحيثية منخفضة الشدة (انظر الحيثية 3).

### السطح 4: تطبيق سطح المكتب Tauri ومواصفات الأمان (`apps/desktop`)
- **صلاحية الحوار الجديدة (`dialog` capability):**
  - `apps/desktop/src-tauri/capabilities/default.json:8-9`: يمنح التطبيق فقط `"dialog:default"` و `"dialog:allow-open"`. لا يمنح صلاحيات حفظ الملفات أو رسائل التأكيد أو الحوارات الشاملة، مما يحقق مبدأ الحد الأدنى للصلاحيات اللازم لاختيار مجلد التنزيل.
- **التحقق من مسار مجلد التنزيل:**
  - عند اختيار المجلد عبر `openDialog` في `apps/desktop/src/lib/SettingsPanel.svelte:38`، يُمرر المسار إلى أمر Tauri المعنون `set_settings`.
  - `apps/desktop/src-tauri/src/lib.rs:333`: يستدعي `validate_settings(&settings)?`.
  - `apps/desktop/src-tauri/src/lib.rs:52-100`: تنفذ الدالة `validate_download_dir` فحوصات دقيقة: تمنع الفراغ، تلزم المسار المطلق، تمنع جذور الأقراص المجردة (`C:\`)، تمنع وجود عناصر `..` أو `.`، وتمنع الفواصل المزدوجة المتتالية (`//` أو `\\`). المسار محمي تماماً قبل اعتماده.
- **سياسة أمان المحتوى (CSP):**
  - `apps/desktop/src-tauri/tauri.conf.json:23-25`: سياسة CSP صارمة وفعالة:
    `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: data:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost; object-src 'none'; base-uri 'self'; frame-ancestors 'none'`
- **سطح أوامر IPC المعرضة:**
  - `apps/desktop/src-tauri/src/lib.rs:456-466`: تقتصر أوامر IPC المعرضة للواجهة الأمامية على 9 أوامر محددة بدقة.
- **معالجة السحب والإفلات (`onDragDropEvent`):**
  - `apps/desktop/src/App.svelte:274-285`: يستمع لحدث السحب والإفلات ويتحقق صراحة من أن المسار المسحوب يبدأ بـ `http://` أو `https://` (`App.svelte:278`). يتم تجاهل أي ملفات أو مسارات محلية على القرص تماماً. إضافة إلى ذلك، لا يبدأ التنزيل تلقائياً، بل يفتح حوار إضافة المهمة لمراجعة الرابط وتأكيده يدوياً من قِبل المستخدم.

### السطح 5: موقع الويب ومقاومة الثغرات (`apps/website`)
- **استخدام `innerHTML` والترجمة:**
  - `apps/website/app.js:161, 167, 187, 191, 193`: يتم تعبئة المحتوى الغني من كائن `translations` المحلي الثابت والمضمن برمجياً في الأسطر `13-80`.
  - لا توجد أي قراءة لمعلمات الرابط (`location.search`) أو مجزأ الرابط (`location.hash`) أو `document.referrer`.
- **روابط النوافذ الجديدة:**
  - `apps/website/index.html:77, 78, 343, 344, 345, 354, 450, 451`: تحتوي جميع الروابط ذات `target="_blank"` على السمة الأمنية الكاملة `rel="noopener noreferrer"`.
- **الموارد والاعتماديات الخارجية:**
  - لا يعتمد الموقع على أي خطوط خارجية (Google Fonts) أو مكتبات CDN أو نصوص برمجية تابعة لأطراف ثالثة. جميع الأصول محلية وذاتية الاستضافة.

### السطح 6: خط أنابيب الإصدار وسير العمل (`.github/workflows` و `tools/release.ps1`)
- **إذن نشر الإصدارات:**
  - `.github/workflows/release.yml:120`: يتحقق سير العمل من وجود الإصدار في المستودع مسبقاً (`gh release view $tag`). إذا لم يكن المالك قد أنشأ الإصدار يدوياً، لا يقوم سير العمل بإنشائه، مما يضمن بقاء قرار النشر بيد مالك المشروع.
  - تم توثيق حيثية حقن الأوامر في سير العمل كحيثية متوسطة (انظر الحيثية 1)، وحيثية نقص صلاحيات الرفع كحيثية متوسطة (انظر الحيثية 2).
- **سياسة توقيع الكود في `release.ps1`:**
  - `tools/release.ps1:253-260`: يؤكد السكربت الامتناع التام عن توقيع الملفات بمفاتيح اختبارية وهمية، ويلزم المالك بالتوقيع اليدوي بشهادته الخاصة.

### السطح 7: الأسرار والخصوصية وتقييم أداة الفحص
- **البحث المتقاطع عن الأسرار والمسارات الشخصية:**
  - لم يُعثر على أي مفاتيح سرية أو رموز وصول (Tokens) أو مسارات مستخدمين شخصية في `assets/brand` أو `extensions` أو `apps/website`.
- **أداة فحص الخصوصية:**
  - تم تقييم نمط القوالب `{}` في `tools/privacy_audit.py` كحيثية منخفضة توفر نقطة عمياء محتملة في الفحص البرمجي الساكن (انظر الحيثية 5).

---

## 5. مخرجات أوامر التحقق التنفيذية (Success Commands Log)

تم تشغيل حزمة أوامر التحقق المطلوبة داخل بيئة المستودع:

```powershell
# 1. تشغيل أداة تدقيق الخصوصية والشبكة:
python tools/privacy_audit.py
```
**النتيجة الفعلية:**
```text
========================================================================
Audit Summary: 4/4 categories passed.
FINAL RESULT:  PASS
========================================================================
```

```powershell
# 2. فحص بناء جملة جافاسكربت لموقع الويب:
node --check apps/website/app.js
# رمز الخروج: 0 (خالٍ من الأخطاء التكوينية)
```

```powershell
# 3. فحص بناء جملة جافاسكربت لإضافة المتصفح:
node --check extensions/browser/chromium/background.js
# رمز الخروج: 0 (خالٍ من الأخطاء التكوينية)
```

```powershell
# 4. التحقق من حالة شجرة git (يجب أن تقتصر التغييرات حصراً على هذا التقرير):
git status --porcelain
```
**النتيجة الفعلية:**
```text
?? docs/SECURITY-REVIEW-V0.1.0.md
```

---

## 6. ملخص التوصيات لخطة إطلاق v0.1.0

1. **معالجة فورية قبل تفعيل سير العمل:** استبدال الاستيفاء المباشر `${{ inputs.version }}` في `.github/workflows/release.yml` بمتغيرات بيئة (`env`) لمنع مخاطر حقن الأوامر، وتصحيح صلاحية `contents: write` للسماح برفع مخرجات الإصدار الرسمي.
2. **تحسين أمان إضافة المتصفح وسكربت التسجيل:** حذف صلاحية `"storage"` غير المستخدمة من `manifest.json`، وإضافة التحقق من نسق `^[a-p]{32}$` في `tools/register-native-host.ps1`.
3. **تضييق نطاق أداة فحص الخصوصية:** استثناء `crates/bench` في `is_application_code()` وإزالة استثناء `http://{}` العام لسد النقطة العمياء الساكنة.
