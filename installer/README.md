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
- `installer/version.txt`: المصدر الموحد الوحيد لإصدار المشروع الحالي.
- `installer/RELEASE-CHECKLIST.md`: قائمة التحقق الإلزامية قبل النشر.

---

## 5. هيكل مجلد التثبيت والتكامل مع المتصفح / Installed Layout & Browser Integration

### هيكل مجلد التثبيت بعد التنصيب / Installed Directory Layout
تُنشر الملفات للمستخدم الحالي (`$LOCALAPPDATA\Programs\SAYL` أو مسار التثبيت المختار) بالهيكل التالي:

```text
<InstallDir>\
├── sayl.exe                               # البرنامج الرئيسي (SAYL Desktop)
├── Uninstall.exe                          # برنامج إلغاء التثبيت
├── host\
│   ├── sayl-bridge.exe                    # جسر الاتصال الأصلي مع المتصفح (Native Messaging Host)
│   ├── com.sayl.downloader.json           # ملف تعريف الجسر (Native Host Manifest)
│   └── icon.ico                           # أيقونة الهوية المعتمدة
└── extensions\
    └── browser\
        └── chromium\                      # إضافة المتصفح غير المضغوطة (Unpacked Chromium MV3 Extension)
            ├── manifest.json              # تعريف الإضافة متضمناً المفتاح الثابت (Fixed Key)
            ├── background.js              # معالج الخلفية لرسائل التنزيل
            ├── popup.html / popup.js      # واجهة الحالة المنبثقة
            ├── icons\                     # أيقونات الإضافة (16, 32, 48, 128)
            └── _locales\                  # الترجمة العربية والإنجليزية
```

### معرف الإضافة ومفاتيح السجل / Pinned Extension ID & Registry Keys
- **مفتاح الإضافة الثابت (Pinned Extension ID):**
  - المفتاح العام المُضمن في `manifest.json`: `MIIBIjANBgkq...` (RSA-2048 DER SubjectPublicKeyInfo).
  - مُعرّف الإضافة المحسوب (SHA-256 first 16 bytes mapped 0-f -> a-p): `ipfenakpblanjjiboejfobkjhegnciif`.
  - النطاق المصرح به (`allowed_origins`): `chrome-extension://ipfenakpblanjjiboejfobkjhegnciif/`.
- **مفاتيح سجل Windows (نطاق المستخدم فقط HKCU):**
  - `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader`
  - `HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.sayl.downloader`
  - القيمة الافتراضية لكليهما تشير إلى المسار المطلق للملف: `<InstallDir>\host\com.sayl.downloader.json`.

### نهج التسجيل وحدوده (NSIS مقابل WiX) / Registration Approach & Limitations
1. **مثبت NSIS (`installer/nsis/hooks.nsh` و `installer/nsis/sayl.nsi`):**
   - عند التثبيت: ينسخ الملفات، ويُنفذ `sayl-bridge.exe --register-host --extension-id ipfenakpblanjjiboejfobkjhegnciif` لكتابة المسار المطلق الدقيق وتحديث سجل النظام. كما يضمن السكربت تحديث ملف `com.sayl.downloader.json` بمسار `$INSTDIR\host\sayl-bridge.exe` المطلق وتسجيل مفاتيح HKCU في حال غياب المشغّل.
   - عند إلغاء التثبيت: يُنفذ `sayl-bridge.exe --unregister-host`، ويحذف مفاتيح HKCU لـ Chrome و Edge، وينظف مفتاح بدء التشغيل التلقائي `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\SAYL`، ويزيل ملفات ومجلدات `host` و `extensions`.
   - لا يتطلب أي صلاحيات مسؤول (User Scope).

2. **مثبت WiX MSI (`installer/wix/native-host.wxs` و `installer/wix/sayl.wxs`):**
   - يسجل المفاتيح `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader` و Edge مشيرةً إلى `[INSTALLDIR]host\com.sayl.downloader.json` عبر مكونات WiX القياسية للمستخدم الحالي (`Scope="perUser"`).
   - **الحدود المعلنة (Documented Limitation):** مثبتات MSI تعمل بدون صلاحيات مسؤول وتتجنب تشغيل Custom Actions معقدة قد تعطل التثبيت الصامت. يُسجل المفتاح مسار المانيفست في السجل تلقائياً. في حال تم نقل مجلد التثبيت يدوياً، يتم تحديث المسار داخل ملف المانيفست تلقائياً عند أول تشغيل لجسر سيل عبر تطبيق سطح المكتب.
