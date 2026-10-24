# قياس أداء SAYL (PERF-001)

يوثق هذا المستند القياسات المعيارية المرجعية والقابلة لإعادة الإنتاج لأداء SAYL مقابل خادم الاختبارات الحتمي المحلي (`HttpFixtureServer`) دون استخدام أرقام تسويقية أو تقديرية.

## كيفية التشغيل (How to Run)

نظراً لعدم توفر Windows SDK لـ MSVC في هذه البيئة، يتم استخدام سلسلة أدوات Rust GNU مع `rust-lld`:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"

# تشغيل القياس الكامل بملف حجمه 32 MiB مع مخرجات نصية مجدولة
cargo +stable-x86_64-pc-windows-gnu run --release -p sayl-bench -- --size 33554432

# تشغيل القياس بمخرجات JSON للتحليل البرمجي
cargo +stable-x86_64-pc-windows-gnu run --release -p sayl-bench -- --size 33554432 --json

# تشغيل سيناريو محدد (مثل sequential أو parallel أو resume)
cargo +stable-x86_64-pc-windows-gnu run --release -p sayl-bench -- --size 33554432 --scenario resume

# تشغيل اختبارات التحقق
cargo +stable-x86_64-pc-windows-gnu test -p sayl-bench
```

## مواصفات بيئة الاختبار (Test Environment)

- **المعالج (CPU):** Intel(R) Core(TM) i7-1265U (10 cores, 12 logical processors)
- **الذاكرة العشوائية (RAM):** 32 GB
- **نظام التشغيل (OS):** Windows 11 (x86_64)
- **أداة البناء (Toolchain):** `stable-x86_64-pc-windows-gnu` مع `rust-lld` linker
- **ملف البناء المستخدم (Build Profile):** `release` (محسّن بدون debug assertions)

## النتائج المقاسة فعلياً (Actual Benchmark Results)

تم قياس هذه الأرقام في هذه الجلسة على ملف اختباري بحجم 33,554,432 بايت (32.00 MiB):

| السيناريو (Scenario) | البيانات المنقولة | زمن التنفيذ (Wall Time) | معدل النقل (Throughput) | التفاصيل والنسب |
| :--- | :--- | :--- | :--- | :--- |
| **1. التدفق التسلسلي (Sequential - 1 conn)** | 32.00 MiB | 0.045 s | 712.17 MB/s | عدد الطلبات: 1 |
| **2a. التدفق المتوازي (Parallel - 4 conns)** | 32.00 MiB | 0.021 s | 1555.14 MB/s | تسريع مقارنة بالتسلسلي: 2.18x (4 طلبات) |
| **2b. التدفق المتوازي (Parallel - 8 conns)** | 32.00 MiB | 0.014 s | 2219.26 MB/s | تسريع مقارنة بالتسلسلي: 3.12x (8 طلبات) |
| **3. عبء طلبات النطاق (Range-Request Overhead)** | 32.00 MiB | - | - | نسبة النقل: 1.0000 (بدون أي تداخل في النطاقات) |
| **4. تكلفة استئناف التنزيل (Resume Correctness Cost)** | 32.00 MiB | - | - | إعادة التنزيل: 0 بايت (0.0%، استئناف دقيق من النصف) |
| **5. الذاكرة المستهلكة (Process Memory)** | - | - | - | ذروة الذاكرة الفعلية (Peak RSS): 44.04 MiB (46,174,208 بايت عبر `K32GetProcessMemoryInfo`) |

## قيود وشفافية القياس (Honest Limitations)

1. **غياب المقارنة مع أدوات خارجية (No external comparison):** لم تُجرَ أي مقارنة ضد aria2 أو IDM نظراً لعدم تثبيتها محلياً والتزاماً بسياسة حظر تحميل برمجيات خارجية خلال الجلسة.
2. **خادم الاختبار المحلي مقابل الإنترنت (Fixture server vs Internet):** الأرقام الناتجة مقاسة عبر خادم HTTP حتمي يعمل على الـ loopback المحلي (`127.0.0.1`) بدون زمن تأخير شبكي (zero latency) وبدون فقدان للحزم أو اختناق للمزودات؛ لذا فإنها تمثل كفاءة معالجة التطبيق وحزمة الشبكة ولا تماثل سرعات الإنترنت الواقعية.
3. **فروقات بيئة البناء (Debug vs Release):** نتائج بيئة `debug` أبطأ بكثير من بيئة `release` نظراً لتعطيل التحسينات وإضافة فحوصات السلامة؛ القياسات الموثقة أعلاه تخص تحديداً وضع `release`.
