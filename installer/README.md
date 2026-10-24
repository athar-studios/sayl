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
- `installer/version.txt`: المصدر الموحد الوحيد لإصدار المشروع الحالي.
- `installer/RELEASE-CHECKLIST.md`: قائمة التحقق الإلزامية قبل النشر.
