# HTTP-001 — Probe, range validation, redirect policy, sequential streaming

state: todo
scope: تنفيذ عميل HTTP لمدير التحميل في `crates/http/` — الاستكشاف (probe)، التحقق من دعم النطاقات، سياسة إعادة التوجيه الآمنة، والبث التسلسلي مع أسماء ملفات آمنة.

## القبول
1. **Probe:**
   - `probe(url) -> Result<ProbeResult>` ينفّذ `HEAD` أولاً، ويعود إلى `GET` مع `Range: bytes=0-0` عند رفض `HEAD` (405/501) أو غياب `Content-Length`
   - `ProbeResult { final_url, status, content_length: Option<u64>, accept_ranges: bool, etag: Option<String>, last_modified: Option<String>, content_type: Option<String>, filename: Option<String> }`
   - تصنيف القدرة: `RangeSupport::{Supported, Ignored, Unsupported, Unknown}`
   - رفض `Content-Length` غير الصالح أو السالب

2. **التحقق من صحة النطاق (range validation):**
   - عند إعلان دعم النطاقات، تحقق فعلياً بطلب `Range: bytes=0-0` والتحقق من: status=206، وجود `Content-Range`، تطابق إجمالي الحجم مع `Content-Length` المُعلن
   - عند فشل التحقق، خفّض القدرة إلى `Ignored`/`Unsupported` — لا تفترض الدعم أبداً
   - تعامل مع `416` كإشارة على نطاق غير صالح لا كفشل قاتل

3. **سياسة إعادة التوجيه الآمنة:**
   - تنفيذ `RedirectPolicy` بحد أقصى 5 قفزات
   - **حذف** `Authorization` و `Cookie` و `Proxy-Authorization` عند تغيّر الـ origin (scheme+host+port)
   - رفض التحويل إلى مخططات غير `http`/`https` (رفض `file:`, `data:`, `ftp:`)
   - منع الحلقات (تتبع URLs المُزارة)
   - تسجيل `final_url` في النتيجة

4. **أسماء ملفات آمنة:**
   - استخراج الاسم من `Content-Disposition` (مع دعم `filename*=UTF-8''...`)، ثم من مسار URL، ثم fallback
   - تطهير: إزالة مسارات `..`، فواصل المسار، أحرف التحكم، محارف Windows المحجوزة `<>:"/\|?*`
   - رفض/استبدال الأسماء المحجوزة (`CON`, `PRN`, `AUX`, `NUL`, `COM1..9`, `LPT1..9`)
   - الحفاظ على الامتداد، وحد أقصى للطول (255 حرفاً)
   - دعم Unicode (بما فيه العربية) مع NFC

5. **البث التسلسلي:**
   - `stream_to_file(probe, path, cancel_token, progress_cb) -> Result<StreamOutcome>`
   - كتابة تدريجية (chunk 64 KiB افتراضي، قابل للضبط)
   - التحقق من تطابق البايتات المكتوبة مع `Content-Length` عند الانتهاء — لا "complete" زائف
   - عند الانقطاع: إرجاع `StreamOutcome::Incomplete { written }` لا خطأ قاتل
   - احترام `CancellationToken`
   - `fsync` عند الإكمال قبل إعلان النجاح

## أدلة
- `crates/http/Cargo.toml` (deps: `reqwest` مع features المطلوبة، `tokio`, `bytes`, `thiserror`, `url`, `unicode-normalization`, `tracing`)
- `crates/http/src/{lib,probe,range,redirect,filename,stream,error}.rs`
- اختبارات وحدة لكل وحدة + اختبارات تكامل في `crates/http/tests/` **تستخدم `sayl-test-http`** لكل سيناريو ذي صلة (`Ok206`, `NoAcceptRanges`, `IgnoredRange200`, `BadContentRange`, `BodyTruncation`, `HeadRejected`, `MaliciousFilename`, `UnknownSize`, `EmptyFile`)
- `crates/http/README.md`

## التفويض
- تنفيذ في `crates/http/` فقط
- لا اتصالات إنترنت خارجية في الاختبارات — استخدم `sayl-test-http` (fixture محلي)
- لا تعديل على `crates/core`, `crates/test-http`, `reports/`, `STATUS.json`

## التحقق
```
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cargo +stable-x86_64-pc-windows-gnu test -p sayl-http
```
يجب أن تنجح كل الاختبارات. راجع `docs/TOOLCHAIN.md`.