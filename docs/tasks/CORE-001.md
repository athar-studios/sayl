# CORE-001 — Core state machine, error types, events, and idempotence

state: todo
scope: تنفيذ النواة الأساسية في `crates/core/`: آلة حالات التنزيل، نظام الأخطاء، نظام الأحداث (events)، ومفاتيح التكرار المحمي (idempotence keys). لا واجهة مستخدم، لا شبكة، لا تخزين دائم — فقط عقود النواة وأنواع البيانات.

## القبول
1. **آلة الحالات (State Machine):**
   - تنفيذ `enum DownloadState` يغطي جميع الحالات من MASTER-PLAN §7:
     `Queued | Probing | Downloading | Verifying | Finalizing | Completed | Pausing | Paused | Cancelling | Cancelled | Failed`
   - انتقالات صريحة ومحمية: دالة `transition(from, to) -> Result` ترفض الانتقالات غير الصالحة.
   - اختبارات property-based لـ state transitions (لا انتقالات غير معرفة، لا حلقات لا نهائية).

2. **نظام الأخطاء (Error Types):**
   - `enum CoreError` يغطي فئات الفشل: `Http`, `Io`, `Storage`, `Scheduler`, `Validation`, `Cancelled`, `Internal`.
   - تنفيذ `std::error::Error`، `From` conversions للأنواع القياسية (`io::Error`, `reqwest::Error`, `rusqlite::Error`, `serde_json::Error`).
   - تصنيف قابل للتجريب: `is_retryable() -> bool`، `is_fatal() -> bool`، `is_user_cancelled() -> bool`.

3. **نظام الأحداث (Events):**
   - `struct Event { id: u64, task_id: TaskId, kind: EventKind, timestamp: DateTime<Utc>, payload: EventPayload }`
   - `EventKind`: `StateChanged`, `Progress`, `Checkpoint`, `Split`, `Retry`, `Error`, `Cancelled`, `Completed`.
   - قناة `mpsc` غير محظورة (bounded) لتوصيل الأحداث من النواة للمستهلكين (UI، تخزين، تسجيل).
   - تسلسل أرقام الأحداث (`sequence_number`) مع snapshot دوري للسماح بالاستعادة بعد إعادة تشغيل.

4. **مفاتيح التكرار المحمي (Idempotence Keys):**
   - كل أمر عام (start, pause, resume, cancel, retry) يحمل `IdempotencyKey: Uuid` من العميل.
   - مخزن `IdempotencyStore` في الذاكرة (spawn task) يفحص المفاتيح المستهلكة حديثاً ويعيد نفس الرد للمسارات المتطابقة.
   - TTL للمفاتيح (افتراضي 24h) وتنظيف دوري.

5. **معرفات المهام (TaskId):**
   - نوع `TaskId` غلاف حول `Uuid v7` (timestamp-ordered) مع `Display` و `FromStr`.

6. **التنظيم والاختبارات:**
   - جميع الأنواع في `crates/core/src/` مع modules: `state`, `error`, `event`, `idempotency`, `id`.
   - `lib.rs` يعيد تصدير API العام.
   - اختبارات وحدة (`#[cfg(test)]`) في نفس الملفات.
   - اختبارات تكامل في `tests/core_state_machine.rs` و `tests/core_idempotency.rs`.

7. **التوثيق:**
   - كل نوع عام و دالة عامة موثقة بـ `///` doc comments.
   - مثال استخدام في `crates/core/README.md`.

## أدلة
- `crates/core/Cargo.toml` مع dependencies: `thiserror`, `serde`, `uuid` (features: `v7`, `serde`), `chrono`, `tokio` (للـ mpsc channel), `bytes`.
- `crates/core/src/lib.rs` و modules.
- اختبارات تمر بـ `cargo test -p sayl-core` (بمجرد توفر toolchain).
- تقارير الأدوار المطلوبة: planner, builder, runtime_tester, debater, final_reviewer, security_reviewer.

## التفويض
- تنفيذ في `crates/core/` فقط.
- لا network I/O، لا file I/O خارج اختبارات fixtures.
- لا UI، لا Tauri، لا CLI.
- الاعتماديات المسموحة: `thiserror`, `serde`, `uuid`, `chrono`, `tokio`, `bytes`, `tracing`.

## ملاحظة حول Toolchain
الترجمة تتطلب MSVC VCTools أو GNU toolchain مستقر. يمكن البدء بتصميم الأنواع والواجهات (`.rs` files) دون ترجمة فورية. الترجمة والتحقق بالاختبارات يتم عند توفر toolchain عامل.