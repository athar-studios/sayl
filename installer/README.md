# خط أنابيب الإصدار والحزم — SAYL ("سيل")
# Release and Packaging Pipeline — SAYL

هذا المجلد يحتوي على مواصفات وبنية حزم التثبيت لتطبيق **سيل (SAYL)** لنظام Windows، متضمنةً مثبت NSIS ومثبت WiX v4 MSI.

This directory contains installer specifications and packaging configurations for **SAYL**, including the NSIS executable installer and the WiX v4 MSI package.

---

## 1. أسماء مخرجات الإصدار / Release Artefacts

ينتج خط أنابيب الإصدار الملفات التالية بدقة داخل مجلد `releases/`:
The release pipeline produces the following exact artefacts in `releases/`:

| اسم الملف / Artefact Name | الوصف / Description | التقنية / Format |
|---|---|---|
| `releases/SAYL-Setup-x64-<version>.exe` | مثبت NSIS (للمستخدم الحالي دون امتيازات مسؤول) | NSIS (Nullsoft Scriptable Install System) |
| `releases/SAYL-Setup-x64-<version>.msi` | مثبت MSI (نطاق المستخدم `Scope="perUser"`) | WiX Toolset v4 |
| `releases/SAYL-<version>-source.zip` | أرشيف كود المصدر الكامل للإصدار | `git archive` |
| `releases/SHA256SUMS.txt` | جدول تجزئات SHA-256 لجميع الملفات | تنسيق متوافق مع `sha256sum` |

تتطابق هذه الأسماء بدقة مع الروابط المعلنة في موقع المشروع (`apps/website/index.html`).

---

## 2. متطلبات Windows SDK / Windows SDK Requirements

### لماذا نحتاج إلى Windows SDK؟ / Why is the Windows SDK needed?
- **بناء الإنتاج (Tauri/MSVC Release):** يتطلب بناء تطبيق Tauri بنمط الإنتاج وجود مترجم MSVC ومكتبات **Windows 10/11 SDK**، وبشكل خاص ملف `kernel32.lib` الموجود عادة في:
  `${env:ProgramFiles(x86)}\Windows Kits\10\Lib\*\um\x64\kernel32.lib`
- بدون تثبيت Windows SDK، يفشل ربط كود Rust/C++ المترجم بـ MSVC بالخطأ الشهير `LNK1181: cannot open input file 'kernel32.lib'`.
- **أدوات الحزم (Packaging Tools):**
  - مترجم NSIS (`makensis.exe`) مطلوب لبناء ملف `.exe`.
  - أداة WiX v4 CLI (`wix.exe`) مطلوبة لبناء ملف `.msi`.

### بيئة التطوير المحلية / Local Development Toolchain
- تقتصر بيئة التطوير المحلية الحالية على سلسلة أدوات `stable-x86_64-pc-windows-gnu` مع ربط `rust-lld`، والتي تتيح تشغيل الاختبارات وفحص الكود دون الحاجة لـ Windows SDK.
- بناء حزم التثبيت الرسمية (`.exe` و `.msi`) يتطلب تشغيل خط الأنابيب على جهاز مثبت عليه Windows SDK (يحتاج صلاحيات مسؤول للتثبيت لمرة واحدة عبر Visual Studio Installer).

---

## 3. سياسة التوقيع والتحقق / Signing and Checksum Policy

### تنبيه أمان صريح: لا توجد ملفات موقعة حاليًا
### Clear Security Notice: Nothing Is Signed Yet
> **تنبيه هام:** لا توجد أي ملفات موقعة في هذا المستودع، ولا يستخدم خط الأنابيب أي شهادات وهمية أو مفاتيح تجريبية مؤقتة.
> **IMPORTANT:** No release artefacts are signed in this repository yet. The build scripts intentionally do NOT sign with placeholder, self-signed, or dummy certificates.

### سياسة التوقيع الرقمي (Code-Signing Policy)
1. **التوقيع خطوة يدوية للمالك فقط:** يجب على مالك المشروع حصراً توقيع الملفات التنفيذية وحزم التثبيت باستخدام شهادة Authenticode تجارية موثوقة خاصة به.
2. **أمر التوقيع المعتمد (Authenticode via SignTool):**
   ```powershell
   # 1. توقيع الملف التنفيذي قبل الحزم
   signtool.exe sign /sha1 <THUMBPRINT> /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "target\release\sayl.exe"

   # 2. توقيع مثبت NSIS ومثبت MSI
   signtool.exe sign /sha1 <THUMBPRINT> /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "releases\SAYL-Setup-x64-<version>.exe"
   signtool.exe sign /sha1 <THUMBPRINT> /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "releases\SAYL-Setup-x64-<version>.msi"
   ```
3. **الختم الزمني (Timestamping):** استخدام خادم ختم زمني متوافق مع RFC 3161 (مثل DigiCert) لضمان بقاء التوقيع صالحاً بعد انتهاء صلاحية الشهادة.

### سياسة التحقق من التجزئات (Checksum Policy)
- يُحسب تجزئة SHA-256 لكل ملف من مخرجات الإصدار (`.exe`, `.msi`, `-source.zip`).
- تُحفظ النتائج في `releases/SHA256SUMS.txt` بصيغة قياسية:
  ```text
  <sha256-hash>  SAYL-Setup-x64-0.1.0.exe
  <sha256-hash>  SAYL-Setup-x64-0.1.0.msi
  <sha256-hash>  SAYL-0.1.0-source.zip
  ```
- يتحقق المستخدم من سلامة التنزيل عبر:
  ```powershell
  Get-FileHash -Algorithm SHA256 .\SAYL-Setup-x64-0.1.0.exe
  ```

---

## 4. أدوات إدارة الإصدار / Release Tools

- `tools/release.ps1`: سكربت بناء وتجميع مخرجات الإصدار (يدعم `-Version`, `-SkipFrontend`, `-DryRun`).
- `tools/check_release.ps1`: سكربت فحص وتدقيق مجلد `releases/` ومطابقة التجزئات وصحة أسماء الملفات.
- `tools/check-brand-assets.ps1`: أداة التحقق من تكامل وأبعاد أيقونات وصور الهوية لمثبتات NSIS و WiX ومطابقتها للمواصفات.
- `tools/check-installer-icon.ps1`: أداة الفحص الآلي للأيقونة المرتبطة بملفات التثبيت التنفيذية والتحقق من طغيان ألوان العلامة التجارية (Teal/Navy) وغياب اللون الأحمر غير المعتمد.
- `tools/check-installer-flow.ps1`: أداة الفحص الساكن لمسار التثبيت، وتأكيد غياب PowerShell للمستخدم النهائي، وتكامل استدعاءات تسجيل وإلغاء تسجيل الجسر، ومواقع LocalAppData، ونظافة إلغاء التثبيت.
- `installer/version.txt`: المصدر الموحد الوحيد لإصدار المشروع الحالي.
- `installer/RELEASE-CHECKLIST.md`: قائمة التحقق الإلزامية قبل النشر.

---

## 5. هيكل مجلد التثبيت والتكامل مع المتصفح / Installed Layout & Browser Integration

### تدفق الإضافة بدون تكوين مسبق / Zero-Config Browser Extension Flow
يقدم مثبت NSIS ومثبت WiX تدفقاً فورياً بدون خطوات إضافية لربط المتصفحات مع سيل:
1. تثبيت ملفات التطبيق والجسر وإضافات المتصفح إلى مجلد البرنامج المختار.
2. تجهيز إضافات المتصفح للمستخدم الحالي تحت:
   - Chromium: `%LOCALAPPDATA%\Sayl\extension\chromium`
   - Firefox: `%LOCALAPPDATA%\Sayl\extension\firefox`
3. تسجيل جسر الاتصال الأصلي عبر استدعاء موحد للجسر:
   `sayl-bridge.exe --register-host`
   (نطاق المستخدم HKCU، بدون طلب صلاحيات مسؤول، وبدون تشغيل PowerShell للمستخدم النهائي).
4. إلغاء التسجيل والتنظيف الكامل عند إزالة التثبيت عبر:
   `sayl-bridge.exe --unregister-host`
   مع حذف مجلدات `%LOCALAPPDATA%\Sayl\extension` و `%LOCALAPPDATA%\Sayl\host`.

### هيكل مجلد التثبيت بعد التنصيب / Installed Directory Layout
تُنشر الملفات للمستخدم الحالي (`$LOCALAPPDATA\Programs\SAYL` أو مسار التثبيت المختار) بالهيكل التالي:

```text
<InstallDir>\
├── sayl.exe                               # البرنامج الرئيسي (SAYL Desktop)
├── Uninstall.exe                          # برنامج إلغاء التثبيت (NSIS)
├── host\
│   ├── sayl-bridge.exe                    # جسر الاتصال الأصلي مع المتصفح (Native Messaging Host)
│   ├── com.sayl.downloader.json           # ملف قالب الجسر (Template/Documentation)
│   └── icon.ico                           # أيقونة الهوية المعتمدة
└── extensions\
    └── browser\
        ├── chromium\                      # إضافة Chromium المفكوكة (Unpacked Chromium MV3 Extension)
        │   ├── manifest.json              # تعريف الإضافة متضمناً المفتاح الثابت (Fixed Key)
        │   ├── background.js              # معالج الخلفية لرسائل التنزيل
        │   ├── popup.html / popup.js      # واجهة الحالة المنبثقة
        │   ├── icons\                     # أيقونات الإضافة (16, 32, 48, 128)
        │   └── _locales\                  # الترجمة العربية والإنجليزية
        └── firefox\                       # إضافة Firefox المفكوكة (Unpacked Firefox MV3 Extension)
            ├── manifest.json              # تعريف إضافة فايرفوكس
            ├── background.js              # معالج الخلفية لرسائل التنزيل
            ├── popup.html / popup.js      # واجهة الحالة المنبثقة
            ├── icons\                     # أيقونات الإضافة (16, 32, 48, 128)
            └── _locales\                  # الترجمة العربية والإنجليزية
```

### ملفات التعريف وسجل النظام / Manifests & Registry Keys
يتولى المشغّل `sayl-bridge.exe --register-host` حصرياً إنشاء وتحديث ملفات التعريف ومفاتيح السجل لكافة المتصفحات الستة المدعومة تحت نطاق المستخدم الحالي (HKCU):
- **ملفات التعريف المكتوبة آلياً بواسطة الجسر في `%LOCALAPPDATA%\Sayl\host\`:**
  - Chromium manifest: `%LOCALAPPDATA%\Sayl\host\com.sayl.downloader.json`
  - Firefox manifest: `%LOCALAPPDATA%\Sayl\host\com.sayl.downloader.firefox.json`
- **مفاتيح سجل Windows (نطاق المستخدم فقط HKCU):**
  - Google Chrome: `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader`
  - Microsoft Edge: `HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.sayl.downloader`
  - Brave Browser: `HKCU\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.sayl.downloader`
  - Opera: `HKCU\Software\Opera Software\Opera Stable\NativeMessagingHosts\com.sayl.downloader`
  - Vivaldi: `HKCU\Software\Vivaldi\NativeMessagingHosts\com.sayl.downloader`
  - Mozilla Firefox: `HKCU\Software\Mozilla\NativeMessagingHosts\com.sayl.downloader`

### معرف الإضافة الثابت / Pinned Extension ID
- المفتاح العام المُضمن في `manifest.json`: `MIIBIjANBgkq...` (RSA-2048 DER SubjectPublicKeyInfo).
- مُعرّف Chromium المحسوب: `ipfenakpblanjjiboejfobkjhegnciif`.
- مُعرّف Firefox المعتمد: `sayl@atharstudios.com`.

### ملكية مفتاح التشغيل التلقائي / Autostart Key Ownership
تطبيق SAYL المكتبي (Desktop UI) هو المالك الوحيد لمفتاح التشغيل التلقائي عند بدء تشغيل ويندوز (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run\SAYL`).
نظراً لأن مثبت النظام لم ينشئ هذا المفتاح ولا يملك تفضيلات المستخدم الخاصة بالإقلاع التلقائي، فإن سكربتات وإجراءات إلغاء التثبيت لا تحذف هذا المفتاح لتجنب التدخل في إعدادات التطبيق دون إثبات الملكية الصريحة.

### طبقة تكوين الموارد للإصدارات / Release Resources Overlay
- **ملف التكوين الأساسي (`apps/desktop/src-tauri/tauri.conf.json`):** لا يحتوي على حقل `bundle.resources`، مما يتيح تشغيل أوامر التطوير والبناء القياسية دون الحاجة للموارد المجهزة.
- **تراكب الإنتاج (`installer/tauri.bundle-resources.json`):** يحدد تخطيط موارد الحزمة النهائي (`host/sayl-bridge.exe` و `host/com.sayl.downloader.json` و `extensions/browser/chromium/**` و `extensions/browser/firefox/**`). يقوم سكربت الإصدار `tools/release.ps1` ببناء `sayl-bridge`، وتجهيز مجلد الموارد تحت `apps/desktop/src-tauri/resources/`، ثم تمرير التراكب عبر `--config installer/tauri.bundle-resources.json`.

### نهج التسجيل والتنظيف (NSIS مقابل WiX) / Registration & Cleanup Approach
1. **مثبت NSIS (`installer/nsis/hooks.nsh` و `installer/nsis/sayl.nsi`):**
   - عند التثبيت: ينسخ الملفات، وينظف مجلدات الوجهة مسبقاً، ويجهز إضافات Chromium و Firefox تحت `%LOCALAPPDATA%\Sayl\extension` باستخدام `xcopy` عبر `nsExec::ExecToLog`، ثم يُنفذ `sayl-bridge.exe --register-host` لإنشاء ملفات المانيفست في `%LOCALAPPDATA%\Sayl\host` وتسجيل مفاتيح HKCU للمتصفحات الستة.
   - عند إلغاء التثبيت: يُنفذ `sayl-bridge.exe --unregister-host` لحذف مفاتيح HKCU والمانيفست، ثم يزيل المجلدات المنشأة بواسطة التثبيت (`%LOCALAPPDATA%\Sayl\extension` و `%LOCALAPPDATA%\Sayl\host`)، دون المساس ببيانات المستخدم أو مفتاح التشغيل التلقائي.

2. **مثبت WiX MSI (`installer/wix/native-host.wxs` و `installer/wix/sayl.wxs`):**
   - نطاق المستخدم بالكامل (`Scope="perUser"`).
   - يتضمن إجراءات مخصصة مُرجأة ومُجسدة لمستخدم الجلسة (`Execute="deferred" Impersonate="yes"`):
     - `StageBrowserExtensions`: تجهيز إضافات المتصفح إلى `%LOCALAPPDATA%\Sayl\extension` بعد `InstallFiles`.
     - `RegisterNativeHost`: تشغيل `sayl-bridge.exe --register-host` بعد التجهيز.
     - `UnregisterNativeHost`: تشغيل `sayl-bridge.exe --unregister-host` قبل `RemoveFiles` عند الإزالة الكاملة (`REMOVE="ALL"`).
     - `CleanupStagedFiles`: إزالة مجلدات `%LOCALAPPDATA%\Sayl\extension` و `%LOCALAPPDATA%\Sayl\host`.
