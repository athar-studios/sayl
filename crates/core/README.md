# sayl-core

نواة محرك تنزيل SAYL — نماذج المجال، آلة الحالات، تصنيف الأخطاء، ناقل الأحداث، وعقود منع التكرار (Idempotency).

## نظرة عامة (Overview)

توفر حزمة `sayl-core` الأنواع الأساسية والعقود الحتمية دون أي إدخال/إخراج شبكي أو تخزين دائم مباشر:

1. **`state` — آلة حالات التنزيل (Download State Machine):**
   - تعداد `DownloadState` بـ 11 حالة صريحة:
     `Queued | Probing | Downloading | Verifying | Finalizing | Completed | Pausing | Paused | Cancelling | Cancelled | Failed`
   - دالة الانتقال المحمية `transition(from, to) -> Result<DownloadState, StateTransitionError>` التي ترفض أي انتقال غير معرف.
   - تصنيفات دلالية: `is_terminal()`, `is_active()`, `can_pause()`, `can_resume()`, `can_cancel()`.

2. **`error` — نظام الأخطاء والتصنيف (Error Taxonomy):**
   - تعداد `CoreError` بـ 7 فئات فشل: `Http`, `Io`, `Storage`, `Scheduler`, `Validation`, `Cancelled`, `Internal`.
   - دوال تصنيف دلالية: `is_retryable()`, `is_fatal()`, `is_user_cancelled()`.
   - تحويلات قياسية عبر تطبيق `std::error::Error` و `Display` و `From`.

3. **`event` — نظام الأحداث وقناة التوصيل (Event System & Bounded Bus):**
   - هيكل `Event` مع ترقيم تسلسلي صارم (`sequence_number`) وطابع زمني `DateTime<Utc>`.
   - 8 أصناف أحداث `EventKind`: `StateChanged`, `Progress`, `Checkpoint`, `Split`, `Retry`, `Error`, `Cancelled`, `Completed`.
   - قناة `mpsc` محددة السعة غير محظورة مع كاشف فجوات التسلسل `SequenceGapDetector` لدعم الاستعادة بعد انقطاع الاتصال.

4. **`idempotency` — مفاتيح التكرار المحمي (Idempotency Store):**
   - نوع `IdempotencyKey` مغلف لـ `Uuid`.
   - مخزن في الذاكرة `IdempotencyStore` آمن للخيوط مع مدة صلاحية TTL (افتراضي 24 ساعة) وتنظيف دوري للمفاتيح المنتهية.

5. **`id` — معرفات المهام (TaskId):**
   - نوع `TaskId` مغلف لـ `Uuid v7` يضمن الترتيب الزمني الطبيعي (timestamp-ordered) لمهام التنزيل.

---

## مثال الاستخدام (Usage Example)

```rust
use std::time::Duration;
use sayl_core::{
    event_channel, transition, DownloadState, EventKind, EventPayload,
    IdempotencyKey, IdempotencyStore, TaskId, CoreError,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. إنشاء معرف مهمة UUID v7 مرتب زمنياً
    let task_id = TaskId::new();
    println!("Task created: {task_id}");

    // 2. إعداد قناة الأحداث المحدودة السعة (Capacity: 64)
    let (event_sender, mut event_receiver) = event_channel(64);

    // 3. مسار انتقال الحالات المحمي
    let mut state = DownloadState::Queued;
    
    // انتقال Queued -> Probing
    state = transition(state, DownloadState::Probing)?;
    event_sender.try_emit(
        task_id,
        EventKind::StateChanged,
        EventPayload::StateChanged {
            from: DownloadState::Queued,
            to: DownloadState::Probing,
        },
    )?;

    // انتقال Probing -> Downloading
    state = transition(state, DownloadState::Downloading)?;
    event_sender.try_emit(
        task_id,
        EventKind::Progress,
        EventPayload::Progress {
            downloaded_bytes: 1024 * 1024,
            total_bytes: Some(10 * 1024 * 1024),
            speed_bps: 512 * 1024,
        },
    )?;

    // 4. قراءة الأحداث مع التحقق من التسلسل
    while let Ok(event) = event_receiver.try_recv() {
        println!("Received Event #{} [{}]: {:?}", event.id, event.kind, event.payload);
    }

    // 5. استخدام مفاتيح عدم التكرار (Idempotency)
    let store = IdempotencyStore::new(Duration::from_secs(3600));
    let client_key = IdempotencyKey::new();

    // فحص المفتاح قبل التنفيذ
    if let Some(cached) = store.get(&client_key) {
        println!("Duplicate command, returning cached response: {}", cached.response);
    } else {
        // تنفيذ وحفظ النتيجة
        store.insert(client_key, task_id, "PAUSED_AT_BYTE_1048576".to_string());
    }

    // 6. تصنيف الأخطاء
    let err = CoreError::http(Some(503), "Service Unavailable", Some(5));
    assert!(err.is_retryable());
    assert!(!err.is_fatal());

    Ok(())
}
```

---

## البناء والتحقق (Build & Verification)

```bash
# فحص الأنواع وبنية الحزمة
cargo check -p sayl-core

# فحص كود الاختبارات والوحدات
cargo check --tests -p sayl-core
```
