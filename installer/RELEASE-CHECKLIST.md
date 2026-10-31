# قائمة التحقق قبل النشر — SAYL ("سيل")
# Pre-Publish Release Checklist — SAYL

يجب على المالك أو مسؤول الإصدار التحقق من كل بند من البنود التالية والتأشير عليها يدويًا قبل إطلاق ونشر أي إصدار رسمي.
The repository owner or release manager must manually verify and tick every item in this checklist before publishing any official release.

---

## 1. بيئة البناء والأدوات / Build Environment & Toolchain
- [ ] **Windows SDK متوفر ومثبت / Windows SDK is present:**
  - التحقق من وجود `kernel32.lib` في مسار Windows Kits (`Windows Kits\10\Lib\*\um\x64\kernel32.lib`).
  - Verify MSVC linker and Windows 10/11 SDK are present with `kernel32.lib`.
- [ ] **أدوات الحزم متوفرة / Packaging tools available:**
  - تثبيت NSIS (`makensis.exe`) و WiX Toolset v4 (`wix.exe`).
  - NSIS and WiX v4 CLI installed and available in PATH.

## 2. حالة المستودع والإصدار / Repository State & Versioning
- [ ] **شجرة عمل نظيفة / Clean working tree:**
  - تنفيذ `git status --porcelain` والتأكد من عدم وجود أي تغييرات غير محسومة أو ملفات غير متتبعة.
  - Verify `git status --porcelain` returns completely empty.
- [ ] **تحديث ومطابقة الإصدار / Version bumped and synchronized:**
  - تحديث الإصدار في `installer/version.txt`.
  - التحقق من مطابقة الإصدار في:
    - `installer/version.txt`
    - `apps/desktop/src-tauri/tauri.conf.json`
    - `apps/desktop/package.json`
    - `Cargo.toml`
  - Ensure version string matches across all project configuration files.

## 3. التحقق الآلي والاختبارات / Automated Checks & Tests
- [ ] **جميع اختبارات الحزم ناجحة / All Rust crate tests green:**
  - تشغيل واجتياز اختبارات مساحة العمل:
    ```powershell
    $env:RUSTFLAGS = "-C linker=rust-lld"
    cargo test --workspace
    ```
- [ ] **اختبارات الواجهة الأمامية ناجحة / Frontend tests green:**
  - تشغيل واجتياز اختبارات واجهة سطح المكتب:
    ```powershell
    cd apps/desktop
    npm test
    ```
- [ ] **فحص النوع والتجميع للواجهة / Frontend build passes:**
  - نجاح بناء واجهة Svelte بدون أخطاء:
    ```powershell
    cd apps/desktop
    npm run build
    ```

## 4. بناء الإصدار والتحقق من المخرجات / Release Build & Verification
- [ ] **خيار أ — البناء على GitHub Actions (لا يحتاج SDK محليًا) / Option A: build in CI (no local SDK needed):**
  - ادفع الوسم ليبدأ `.github/workflows/release.yml`:
    ```powershell
    git tag v0.1.0
    git push origin v0.1.0
    ```
  - السير العمل يبني على `windows-latest` (فيه Windows SDK)، ويتحقق من وجود الملفات الأربعة، ويرفعها كـ artifact باسم `sayl-<version>-windows-x64`.
  - إن كان GitHub Release للوسم موجودًا مسبقًا، تُرفع الملفات إليه في وظيفة منفصلة تملك `contents: write`. **إنشاء الإصدار نفسه يبقى قرار المالك.**
- [ ] **خيار ب — البناء محليًا (يتطلب Windows SDK مثبتًا بصلاحيات مدير) / Option B: local build (requires elevated Windows SDK):**
  - تنفيذ البناء الرسمي:
    ```powershell
    pwsh -NoProfile -File tools/release.ps1
    ```
- [ ] **فحص مخرجات الإصدار والتجزئات / Check artefacts and hashes:**
  - تشغيل سكربت التدقيق والتأكد من خروجه برمز صفر:
    ```powershell
    pwsh -NoProfile -File tools/check_release.ps1
    ```
  - التحقق من وجود الملفات الأربعة بدقة:
    - `releases/SAYL-Setup-x64-<version>.exe`
    - `releases/SAYL-Setup-x64-<version>.msi`
    - `releases/SAYL-<version>-source.zip`
    - `releases/SHA256SUMS.txt`

## 5. توقيع الكود الرقمي (يدوي وحصري للمالك) / Code Signing (Manual by Owner Only)
- [ ] **توقيع الملفات التنفيذية وحزم التثبيت بشهادة المالك الرسمية:**
  - **Code signing performed by the owner with their own certificate.**
  - لا تستخدم شهادات وهمية أو مفاتيح تجريبية غير معتمدة.
  - توقيع `sayl.exe` قبل الحزم، وتوقيع ملفي `.exe` و `.msi` بعد الحزم باستخدام `signtool.exe`:
    ```powershell
    signtool.exe sign /sha1 <OWNER_THUMBPRINT> /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "releases\SAYL-Setup-x64-<version>.exe"
    signtool.exe sign /sha1 <OWNER_THUMBPRINT> /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "releases\SAYL-Setup-x64-<version>.msi"
    ```
  - التحقق من صحة التوقيع والختم الزمني:
    ```powershell
    Get-AuthenticodeSignature "releases\SAYL-Setup-x64-<version>.exe"
    Get-AuthenticodeSignature "releases\SAYL-Setup-x64-<version>.msi"
    ```
  - تحديث `releases/SHA256SUMS.txt` إذا تم التوقيع بعد توليد التجزئات.

## 6. تطابق الموقع والتوثيق / Website & Documentation Parity
- [ ] **تطابق أسماء الملفات مع الموقع / Website artefact names match:**
  - مراجعة `apps/website/index.html` والتأكد من مطابقة أسماء الملفات وروابط التنزيل تماماً.
  - Verify download buttons in `apps/website/index.html` point to the exact generated file names.
- [ ] **مراجعة سجل التغييرات / Changelog updated:**
  - توثيق الميزات والإصلاحات والتغييرات الخاصة بالإصدار.

## 7. مراجعة الأمان والخصوصية / Security & Privacy Review
- [ ] **توثيق المراجعة الأمنية / Security review recorded:**
  - التأكد من عدم وجود أي تتبع أو تحليلات مخفية (Zero telemetry / Zero analytics).
  - التأكد من عدم تضمين مفاتيح أو أسرار أو عناوين URL داخلية في الحزم.
  - فحص أسماء الملفات وتعقيم المسارات وفق متطلبات الأمان في `SECURITY.md`.
  - مراجعة تبعات الطرف الثالث (Supply chain / dependency audit).
