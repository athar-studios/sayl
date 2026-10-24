# TEST-001 — Deterministic HTTP fixture server and failure modes

state: todo
scope: تنفيذ خادم HTTP حتمي (fixture server) في `crates/test-http/` لمحاكاة سيناريوهات التنزيل المختلفة وأنماط الفشل المحددة في MASTER-PLAN §8. يعمل كخادم محلي للاختبارات التكاملية دون حاجة لاتصالات إنترنت خارجية.

## القبول
1. **خادم HTTP حتمي:**
   - تنفيذ `HttpFixtureServer` في `crates/test-http/src/server.rs`
   - ربط على `127.0.0.1:0` (منفذ عشوائي) مع واجهة `start() -> Result<SocketAddr>` و `shutdown()`
   - استجابة `GET` و `HEAD` مع تحكم كامل في: status code، headers، body، تأخير الاستجابة
   - دعم `Range` requests مع تحقق صحيح من `Content-Range` و `206 Partial Content`
   - توليد body حتمي قابل للتكرار (مثلاً: بايت متسلسل `0x00..0xFF` مكرر)

2. **سيناريوهات الفشل الإلزامية (MASTER-PLAN §8):**
   - `Ok206`: استجابة 206 صحيحة مع `Content-Range` و `Accept-Ranges: bytes`
   - `NoAcceptRanges`: استجابة 200 تتجاهل `Range` header
   - `IgnoredRange200`: استجابة 200 رغم وجود `Range` (بدون `Content-Range`)
   - `BadContentRange`: استجابة 206 بـ `Content-Range` خاطئ (مثال: `bytes */100` أو نطاق خارج الحدود)
   - `BodyTruncation`: body أقصر من `Content-Length` أو `Content-Range` المعلن
   - `ConnectionReset`: قطع TCP مفاجئ (`RST`) أثناء النقل
   - `ETagChanged`: استجابة 206 مع `ETag` متغير بين الطلبات
   - `WeakETagOnly`: استجابة بضعيف `ETag` فقط (بمقدم `W/`)
   - `NoValidator`: لا `ETag` ولا `Last-Modified`
   - `416RangeNotSatisfiable`: استجابة 416 عند طلب نطاق خارج الحدود
   - `HeadRejected`: استجابة 405/501 لـ `HEAD`
   - `UnknownSize`: استجابة 200 بدون `Content-Length` ولا `Transfer-Encoding: chunked`
   - `EmptyFile`: ملف بحجم 0 بايت
   - `UnexpectedEncoding`: `Content-Encoding: gzip` غير متوقع عند طلب `identity`
   - `429RetryAfter`: استجابة 429 مع `Retry-After` header
   - `RedirectWithSecrets`: redirect عبر نطاق مختلف مع تسريب `Authorization`/`Cookie`
   - `NoSpace`: محاكاة `ENOSPC` عند الكتابة
   - `PermissionDenied`: محاكاة `EACCES` عند الوصول للملف
   - `MaliciousFilename`: أسماء ملفات ضارة/عربية في `Content-Disposition`
   - `PauseCancelDuringProbe`: إلغاء/إيقاف أثناء مرحلة probe
   - `SlowWorker`: عامل بطيء جداً (تأخير متعمد)
   - `OldLease`: lease منتهية الصلاحية
   - `RecoveryAtFinalize`: سيناريو استعادة عند finalize
   - `ProxyTLS`: استجابة عبر proxy مع TLS
   - `LimitsOrigin`: تجاوز حدود الأصل (origin limits)
   - `FileTruncation`: ملف مقطوع عند الاستكمال
   - `Pooling`: إعادة استخدام اتصالات HTTP/2

3. **API للتحكم في السيناريوهات:**
   - `HttpFixtureServer::with_scenario(name) -> Self`
   - `server.set_delay(duration)` — تأخير استجابة قابل للضبط
   - `server.set_chunk_size(bytes)` — حجم chunk للتدفق
   - `server.record_request(request)` — تسجيل الطلبات للفحص اللاحق
   - `server.verify_requests(expected)` — تحقق من تسلسل الطلبات

4. **التكامل مع `sayl-core`:**
   - إضافة `crates/test-http` كـ dev-dependency في `sayl-core` للاختبارات
   - استخدام `fixtures/sample.bin` من `crates/core/fixtures/` كـ body مرجعي

5. **التوثيق:**
   - `crates/test-http/README.md` مع أمثلة استخدام كل سيناريو
   - توثيق كل سيناريو: السلوك المتوقع، headers، codes

## أدلة
- `crates/test-http/Cargo.toml` مع dependencies: `tokio`, `hyper`, `bytes`, `tokio-tungstenite` (للـ WebSocket إن لزم), `tracing`, `serde`, `uuid`
- `crates/test-http/src/server.rs`, `scenarios.rs`, `body_gen.rs`, `lib.rs`
- اختبارات وحدة في `crates/test-http/src/`
- توثيق `crates/test-http/README.md`
- تحقق: `cargo check -p sayl-test-http` و `cargo check --tests -p sayl-test-http` نجاح

## التفويض
- تنفيذ في `crates/test-http/` فقط
- لا اتصالات شبكة خارجية، لا اعتماديات خارجية غير مسموحة
- `cargo check -p sayl-test-http` يجب أن ينجح (بدون linker)
- الاعتماديات المسموحة: `tokio`, `hyper`, `bytes`, `tracing`, `serde`, `uuid`, `chrono`

## ملاحظة حول Toolchain
التنفيذ والتحقق بالأنواع عبر `cargo check` ممكن بدون MSVC linker. تشغيل الاختبارات التنفيذية (`cargo test`) سيتطلب MSVC linker لاحقاً.