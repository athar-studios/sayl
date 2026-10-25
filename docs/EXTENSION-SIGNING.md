# توقيع امتداد المتصفح وإدارة المفاتيح / Browser Extension Signing & Key Management

يوثّق هذا الملف كيفية إدارة زوج مفاتيح التشفير لامتداد Chromium الخاص بـ SAYL، وثبات مُعرّف الامتداد (Extension ID)، وتخزين المفتاح الخاص، وإجراءات التدوير، والحدود التشغيلية الحالية لخطوط الإنتاج (CI).

---

## 1. سبب تثبيت مُعرّف الامتداد (Why the Extension ID is Fixed)

في متصفحات Chromium (مثل Google Chrome وMicrosoft Edge وBrave وOpera وVivaldi):
- يُشتق مُعرّف الامتداد (32 محرفًا من `a` إلى `p`) رياضيًا من تجزئة SHA-256 للمفتاح العام المشفر بصيغة DER (`SubjectPublicKeyInfo`): تؤخذ أول 16 بايت وتُحوّل كل 4 بتات (nibble) إلى الحرف المقابل (`0 -> a`, ..., `f -> p`).
- عند تضمين المفتاح العام بصيغة Base64 في حقل `"key"` داخل `manifest.json`، يضمن المتصفح حصول الامتداد دائمًا على نفس المعرّف الثابت:
  ```
  cpigidmkbhcfkgljofmmpnfdhdaoldoj
  ```
- **ضرورة الثبات:** يتطلب بروتوكول الرسائل الأصلية (Native Messaging) تسجيل معرّف الامتداد المصرح له مسبقًا في سجل النظام ومستندات الإعداد (`allowed_origins: ["chrome-extension://cpigidmkbhcfkgljofmmpnfdhdaoldoj/"]`). لو تغيّر المعرّف مع كل تثبيت، لفشل اتصال المتصفح بتطبيق `sayl-bridge.exe`.

---

## 2. تخزين المفتاح الخاص وسر GitHub (Key Storage & GitHub Secret)

- **المسار المحلي للمفتاح الخاص:**
  `%USERPROFILE%\.sayl\extension-private-key.pem`
  - المفتاح الخاص لا يُحفظ **أبدًا** داخل مستودع الشفرة أو مجلدات العمل (`.gitignore` يتجاهل `*.pem` و `*.key`).
  - يتم تقييد صلاحيات الملف محليًا (ACL) لتكون مقصورة على المستخدم الحالي فقط.
- **السر في GitHub Actions:**
  - الاسم: `SAYL_EXTENSION_PRIVATE_KEY`
  - المستودع: `athar-studios/sayl`
  - يُرفع تلقائيًا أو يدويًا عبر معيار التدفق القياسي (stdin) دون طباعته على الشاشة:
    ```powershell
    Get-Content -Raw "$env:USERPROFILE\.sayl\extension-private-key.pem" | gh secret set SAYL_EXTENSION_PRIVATE_KEY --repo athar-studios/sayl
    ```

---

## 3. أدوات إدارة المفتاح (Tooling: `tools/extension-key.ps1`)

السكربت `tools/extension-key.ps1` يوفر ثلاث وظائف رئيسية:
1. **توليد مفتاح جديد (`-Generate`):**
   ينشئ زوج مفاتيح RSA-2048 في `%USERPROFILE%\.sayl\extension-private-key.pem` ويرفض الكتابة فوق مفتاح قائم إلا عند تمرير `-Force`. لا يطبع السكربت المفتاح الخاص إطلاقًا.
2. **عرض المفتاح والتحقق منه (`-Show`):**
   يقرأ المفتاح الخاص المحلي ويستخرج المفتاح العام والمعرّف المحسوب، ويتحقق من مطابقته لـ `manifest.json`.
3. **التحقق من الاتساق (`-Verify` - الافتراضي):**
   يتحقق من اتساق المفتاح العام في `manifest.json` مع المعرّف المشتق ومع كافة الثوابت في Rust ومثبتات التشغيل وقوالب المضيف:
   ```powershell
   pwsh -NoProfile -File tools/extension-key.ps1 -Verify
   ```

---

## 4. خطوات تدوير المفتاح (Key Rotation Steps)

عند الحاجة لتدوير المفتاح (Rotation) لسبب أمني أو إداري:
1. **توليد زوج مفاتيح جديد:**
   ```powershell
   pwsh -NoProfile -File tools/extension-key.ps1 -Generate -Force
   ```
2. **تحديث سر المستودع في GitHub:**
   ```powershell
   Get-Content -Raw "$env:USERPROFILE\.sayl\extension-private-key.pem" | gh secret set SAYL_EXTENSION_PRIVATE_KEY --repo athar-studios/sayl
   ```
3. **تحديث الثوابت في المستودع بالمعرّف الجديد:**
   - `extensions/browser/chromium/manifest.json`: تحديث حقل `"key"`.
   - `tools/build-browser-extensions.ps1`: تحديث `$EXPECTED_CHROMIUM_ID`.
   - `crates/bridge/src/install.rs`: تحديث ثابت `PINNED_CHROMIUM_EXTENSION_ID` واختبارات `allowed_origins`.
   - `extensions/browser/host/com.sayl.downloader.json`: تحديث `allowed_origins`.
   - ملفات التثبيت (WiX وNSIS في `installer/**`): تحديث بارامترات `--extension-id` وقيم `allowed_origins`.
   - التوثيق في `extensions/browser/README.md` و `extensions/browser/chromium/README.md`.
4. **فحص الاتساق:**
   ```powershell
   pwsh -NoProfile -File tools/extension-key.ps1 -Verify
   pwsh -NoProfile -File tools/build-browser-extensions.ps1
   cargo test -p sayl-bridge
   ```
5. **إعادة الرفع للمتاجر:**
   تنبيه: تغيير المفتاح يعني توليد معرّف جديد، وهو ما تعتبره متاجر الإضافات (Chrome Web Store / Edge Add-ons) إضافة جديدة تمامًا وليست مجرد تحديث.

---

## 5. حالة الأتمتة في CI والحدود التشغيلية (CI Status & Limitations)

- **ما يقوم به CI حاليًا:**
  - فحص صحة شفرة جافاسكريبت وJSON عبر `tools/build-browser-extensions.ps1`.
  - حزم ملفات الامتداد في أرشيفات مضغوطة (`.zip`) جاهزة للتحميل كحزم غير مضغوطة أو للرفع اليدوي لمتاجر الإضافات.
- **ما لم تتم أتمتته بعد (Limitations):**
  - **حزم `.crx` الموقعة:** لا يتم توقيع أو حزم `.crx` بشكل ذاتي لتوزيع Chromium المنفصل خارج المتجر.
  - **توقيع Mozilla AMO:** توقيع إضافة Firefox عبر واجهة برمجة تطبيقات AMO ليس مؤتمتًا في مسار العمل الحالي.
  - **النشر التلقائي للمتاجر:** يتطلب التوزيع على Chrome Web Store و Edge Add-ons حسابات مطورين وعمليات مراجعة يدوية.

---

## English Summary

### Fixed Extension ID & Rationale
Chromium extension IDs are deterministically calculated as the first 16 bytes of the SHA-256 digest of the SubjectPublicKeyInfo DER-encoded RSA public key, mapped to characters `a..p`. Embedding the base64-encoded public key in `manifest.json` under `"key"` fixes the extension ID to `cpigidmkbhcfkgljofmmpnfdhdaoldoj`. This pinned ID is essential for Native Messaging host registration, ensuring the browser permits communication with `sayl-bridge.exe`.

Firefox uses gecko add-on ID `sayl@atharstudios.com` and is unaffected by this RSA key pair.

### Key Storage
- **Local Private Key:** Stored outside the repository at `%USERPROFILE%\.sayl\extension-private-key.pem` with NTFS ACL restricted to the current user.
- **GitHub Secret:** `SAYL_EXTENSION_PRIVATE_KEY` under repository `athar-studios/sayl`.
- **Git Protection:** `.gitignore` excludes `*.pem` and `*.key` to prevent accidental commits.

### Tooling
- `tools/extension-key.ps1 -Generate`: Generates 2048-bit RSA key outside repo.
- `tools/extension-key.ps1 -Show`: Inspects private key, prints derived ID and public key, asserts match with `manifest.json`.
- `tools/extension-key.ps1 -Verify`: Ensures all repository constants match the public key in `manifest.json`.

### Rotation Workflow
1. Run `pwsh -NoProfile -File tools/extension-key.ps1 -Generate -Force`.
2. Update GitHub secret: `Get-Content -Raw "$env:USERPROFILE\.sayl\extension-private-key.pem" | gh secret set SAYL_EXTENSION_PRIVATE_KEY --repo athar-studios/sayl`.
3. Update `manifest.json`, `tools/build-browser-extensions.ps1`, `crates/bridge/src/install.rs`, host manifests, installer definitions, and docs.
4. Verify tests pass and submit the extension to stores (note: new key generates a new store ID).

### Current Automation Status & Limitations
- CI packages extensions into ZIP archives (`sayl-chromium-0.1.0.zip`, `sayl-firefox-0.1.0.zip`).
- Standalone `.crx` packaging and Mozilla AMO signing/publishing are not automated yet and require separate store developer pipelines.
