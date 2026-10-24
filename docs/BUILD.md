# دليل بناء واختبار سيل (SAYL Build & Verification Guide)

يشرح هذا الدليل متطلبات وخطوات بناء واختبار كافة مكونات مشروع **سيل (SAYL)** على بيئة تطوير نظام Windows، مع توضيح الفروقات الدقيقة بين سلاسل الأدوات وأوامر التحقق المعتمدة.

---

## 1. المتطلبات الأساسية (Prerequisites)

- **نظام التشغيل:** Windows 10 (إصدار 1903 أو أحدث) أو Windows 11 بنواة x64.
- **Rust Toolchain:**
  - تثبيت سلسلة أدوات GNU: `stable-x86_64-pc-windows-gnu`
  - الرابط السريع المدمج: `rust-lld`
- **بيئة الويب (Frontend):**
  - Node.js (الإصدار 18 أو أحدث)
  - مدير الحزم npm (أو pnpm)
- **للبناء الإنتاجي الكامل لـ Tauri عبر MSVC:**
  - Visual Studio 2022 Build Tools (14.44.35207) مع حزمة **Windows SDK (10.0.26100.0)** مثبتة وجاهزة للربط عبر `vcvars64.bat`.

---

## 2. سلسلة الأدوات المعتمدة محلياً (GNU + rust-lld)

لتفادي الاعتماد على C-compilers خارجية أو مكتبات Windows SDK غير المتوفرة، تعتمد بيئة التطوير والاختبار الحالية على سلسلة أدوات `stable-x86_64-pc-windows-gnu` مع ميزة الربط المدمجة بـ `rust-lld`.

### إعداد المتغيرات والبيئة (PowerShell)

قبل تنفيذ أي أوامر، يجب تجهيز المسار ومتغيرات بيئة التجميع في جلسة PowerShell:

```powershell
# 1. إضافة مسار أدوات Cargo
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# 2. تحديد رابط rust-lld المدمج لتفادي أي طلبات لمكتبات خارجية
$env:RUSTFLAGS = "-C linker=rust-lld"

# 3. الانتقال لمجلد المشروع
cd "C:\Users\<you>\Downloads\Download Manager"
```

---

## 3. أوامر الفحص والاختبار المعتمدة لكل حزمة (Verified Commands per Crate)

### فحص الأنواع وبنية الشفرة (Type Checking)

تعمل هذه الأوامر على فحص الكود والتحقق من الأنواع بنجاح لكافة حزم المشروع:

```powershell
# فحص الحزم الأساسية
cargo +stable-x86_64-pc-windows-gnu check -p sayl-core
cargo +stable-x86_64-pc-windows-gnu check -p sayl-test-http
cargo +stable-x86_64-pc-windows-gnu check -p sayl-http
cargo +stable-x86_64-pc-windows-gnu check -p sayl-storage
cargo +stable-x86_64-pc-windows-gnu check -p sayl-app

# فحص حزمة سطح المكتب (Backend Crate)
cargo +stable-x86_64-pc-windows-gnu check -p sayl-desktop
```

### تشغيل الاختبارات لكل حزمة (Unit & Integration Tests)

أوامر الاختبارات المعتمدة التي تعمل بنجاح 100% بدون أي حاجة لـ Windows SDK:

```powershell
# 1. اختبارات sayl-core (33 اختباراً ناجحاً)
cargo +stable-x86_64-pc-windows-gnu test -p sayl-core

# 2. اختبارات sayl-test-http (25 اختباراً ناجحاً)
cargo +stable-x86_64-pc-windows-gnu test -p sayl-test-http

# 3. اختبارات sayl-http (65 اختباراً ناجحاً)
cargo +stable-x86_64-pc-windows-gnu test -p sayl-http

# 4. اختبارات sayl-storage (18 اختباراً ناجحاً)
cargo +stable-x86_64-pc-windows-gnu test -p sayl-storage

# 5. اختبارات sayl-app (9 اختبارات ناجحة)
cargo +stable-x86_64-pc-windows-gnu test -p sayl-app

# تشغيل اختبارات مساحة العمل بالكامل دفعة واحدة
cargo +stable-x86_64-pc-windows-gnu test --workspace
```

---

## 4. مسار البناء الإنتاجي عبر MSVC (MSVC Production Toolchain)

### حالة بيئة MSVC و Windows SDK (Verified MSVC Status)
- أدوات البناء الرسمية لـ Visual Studio 2022 (`MSVC 14.44.35207`) والرابط `link.exe` مثبتة على النظام في المسار:  
  `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools`
- **حزمة تطوير برمجيات ويندوز (Windows SDK):** مثبتة بإصدار `10.0.26100.0` (المكتبة الأساسية: `C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64\kernel32.lib`).
- **حالة الربط عبر MSVC:** ربط MSVC يعمل بنجاح تام بعد تفعيل بيئة `vcvars64.bat`. خطأ `LNK1181: cannot open input file 'kernel32.lib'` السابق تم حله بالكامل.
- **تكامل Tauri وخط الإصدار:** مجمّع Tauri وحزم المثبتات (NSIS و WiX) يُبنى بنجاح عبر سلسلة أدوات MSVC الرسمية.
- **مسار GNU البديل:** يظل مسار `stable-x86_64-pc-windows-gnu` المقترن بـ `rust-lld` مساراً صالحاً ومتاحاً لاختبارات الوحدات والفحص السريع.

### كيفية تفعيل بيئة MSVC للبناء
للبناء عبر MSVC، يتم تهيئة بيئة المطور أولاً عبر سطر الأوامر:
```cmd
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
cargo build --release -p sayl-desktop
```

---

## 5. مصفوفة الأوامر وحالة الدعم (Command Support Matrix)

| الأمر / الإجراء | الحالة الحالية | الأداة المستخدمة | ملاحظات |
|---|---|---|---|
| `cargo check` لكافة الحزم | ✅ يعمل بنجاح | GNU أو MSVC | فحص الشفرة والأنواع |
| `cargo check -p sayl-desktop` | ✅ يعمل بنجاح | GNU أو MSVC | فحص كود الواجهة الخلفية لسطح المكتب |
| `cargo test` لجميع حزم النواة | ✅ يعمل بنجاح | GNU + `rust-lld` / MSVC | اختبارات كاملة |
| اختبارات واجهة Svelte (Vitest) | ✅ يعمل بنجاح (21 اختباراً) | Node.js / npm | اختبارات المكونات الأمامية |
| بناء واجهة Svelte للإنتاج (`npm run build`) | ✅ يعمل بنجاح | Node.js / Vite | توليد مخرجات `dist/` |
| `cargo test` عبر MSVC toolchain | ✅ يعمل بنجاح | MSVC `link.exe` | يتطلب تهيئة بيئة `vcvars64.bat` |
| `cargo build -p sayl-desktop` (إنتاجي) | ✅ يعمل بنجاح | MSVC `link.exe` | ينتج الثنائي الإنتاجي لسطح المكتب |
| `npm run tauri -- build` (حزم المثبت) | ✅ يعمل بنجاح | Tauri CLI + MSVC | ينتج مثبت NSIS (.exe) ومثبت WiX (.msi) |

---

## 6. بناء واختبار واجهة سطح المكتب (Svelte / npm Build)

توجد واجهة المستخدم في المجلد `apps/desktop`:

```powershell
cd "apps\desktop"

# 1. تثبيت التبعيات
npm install

# 2. فحص بنية وأنواع Svelte وTypeScript
npm run check

# 3. تشغيل اختبارات الواجهة (21 اختباراً ناجحاً عبر vitest)
npm run test

# 4. بناء الأصول الثابتة المجمعة للإنتاج
npm run build
```

تُنتج عملية البناء مجلداً جاهزاً باسم `apps/desktop/build` يتم تضمينه تلقائياً داخل نافذة Tauri.

---

## 7. حل مشكلة أمان التطبيقات الذكي (Smart App Control Workaround)

على أنظمة Windows 11 المزودة بخاصية **Smart App Control (SAC)** أو برامج الحماية من الفيروسات التفاعلية:
- قد يحدث في حالات نادرة أن تقوم ميزة الحماية بحظر أو قفل الملف التنفيذي للاختبارات المنشأ حديثاً داخل مسار البناء:  
  `target\debug\deps\<crate_test>-*.exe`
- يظهر الخطأ عادة بصيغة: `Access is denied (os error 5)`.
- **خطوات الحل السريع:**  
  لا حاجة لإعادة بناء مساحة العمل كاملة؛ يكفي حذف الملف التنفيذي العالق داخل مجلد `deps` لإجبار `rust-lld` على إنشاء ملف برابط جديد:
  ```powershell
  # حذف الملف المحظور بعينه
  Remove-Item "target\debug\deps\sayl_http-*.exe" -Force
  
  # ثم أعد تشغيل أمر الاختبار مباشرة
  cargo +stable-x86_64-pc-windows-gnu test -p sayl-http
  ```
