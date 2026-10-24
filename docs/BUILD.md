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
- **اختياري (للبناء الإنتاجي النهائي لـ Tauri عبر MSVC):**
  - Visual Studio 2022 Build Tools مع مكون **Windows SDK**.

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

## 4. لماذا يتعذر استخدام MSVC حالياً؟ وكيف يتم إصلاحه؟

### المشكلة والتشخيص الفني (Missing Windows SDK)
- أدوات البناء الرسمية لـ Visual Studio 2022 (`MSVC 14.44.35207`) والرابط `link.exe` مثبتة على النظام في المسار:  
  `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools`
- ولكن، **حزمة تطوير برمجيات ويندوز (Windows SDK) غير مثبتة**.
- عند محاولة الربط بواسطة أداة MSVC الافتراضية (`stable-x86_64-pc-windows-msvc`)، يفشل البناء فوراً بالخطأ:
  ```text
  LINK : fatal error LNK1181: cannot open input file 'kernel32.lib'
  ```
- السبب: ملفات النظام الأساسية مثل `kernel32.lib` و `user32.lib` و `shell32.lib` ورؤوس Win32 تتبع حزمة Windows SDK وليس مجمع MSVC نفسه.

### الحل المطلوب لتفعيل MSVC
إذا أردت تفعيل مسار MSVC للبناء النهائي الكامل:
1. افتح **Visual Studio Installer** بصلاحيات مسؤول النظام (Run as administrator).
2. اختر **تعديل (Modify)** لتثبيت Build Tools 2022 الحالي.
3. من علامة تبويب المكونات الفردية (Individual Components)، حدد:  
   **Windows 11 SDK (10.0.22621.0)** أو **Windows 10 SDK (10.0.19041.0)**.
4. أتمم التثبيت؛ فور انتهاء التثبيت سيتعرف `link.exe` تلقائياً على مسارات المكتبات.

---

## 5. مصفوفة الأوامر: ما يعمل الآن مقابل ما يحتاج Windows SDK

| الأمر / الإجراء | الحالة الحالية | الأداة المستخدمة | متطلبات إضافية |
|---|---|---|---|
| `cargo check` لكافة الحزم | ✅ يعمل فوراً | GNU toolchain | لا شيء |
| `cargo check -p sayl-desktop` | ✅ يعمل فوراً | GNU toolchain | لا شيء |
| `cargo test` لجميع حزم النواة | ✅ يعمل فوراً | GNU + `rust-lld` | لا شيء |
| اختبارات واجهة Svelte (Vitest) | ✅ يعمل فوراً (21 اختباراً) | Node.js / npm | لا شيء |
| بناء واجهة Svelte للإنتاج (`npm run build`) | ✅ يعمل فوراً | Node.js / Vite | لا شيء |
| `cargo test` عبر MSVC toolchain | ❌ يتعطل حالياً (`LNK1181`) | MSVC `link.exe` | يتطلب تثبيت Windows SDK |
| `cargo build -p sayl-desktop` (إنتاجي) | ❌ يتعطل حالياً (`LNK1181`) | MSVC `link.exe` | يتطلب تثبيت Windows SDK |
| `cargo tauri build` (حزم المثبت) | ❌ يتعطل حالياً | Tauri CLI + MSVC | يتطلب تثبيت Windows SDK وصلاحيات رفع |

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
