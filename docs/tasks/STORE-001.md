# STORE-001 — Crash-safe task store and checkpoints (pure Rust)

state: todo
scope: تنفيذ طبقة تخزين دائمة في `crates/storage/` لتسجيل المهام ونقاط التقدم القابلة للاستعادة، بدون أي اعتماد على C (لا SQLite مضمّن، لا C compiler متاح في هذه البيئة).

## قرار معماري مفروض بالبيئة
لا يوجد مجمّع C على الجهاز، لذا `rusqlite`/`libsqlite3-sys` (bundled) غير قابل للبناء. الحل: مخزن append-only journal + لقطة سنابشوت ذرية، بلغة Rust خالصة. يُعاد النظر في SQLite عند توفر Windows SDK/مجمّع C (موثّق في `docs/TOOLCHAIN.md`).

## القبول
1. **مخطط السجلات:**
   - `TaskRecord { id: TaskId, url, final_url, save_path, filename, state: DownloadState, total_size: Option<u64>, written: u64, etag: Option<String>, last_modified: Option<String>, created_at, updated_at, error: Option<String> }`
   - `Checkpoint { task_id, written, total_size, range_map: Vec<(u64,u64)>, updated_at }` لدعم التنزيل المتوازي لاحقاً
2. **واجهة `Store`:**
   - `Store::open(dir) -> Result<Store>` / `open_in_memory()` للاختبارات
   - `upsert_task(&TaskRecord)` / `get_task(TaskId)` / `list_tasks()` / `delete_task(TaskId)`
   - `save_checkpoint(&Checkpoint)` / `load_checkpoint(TaskId)`
   - `flush()` / `compact()` / `stats() -> StoreStats { records, journal_bytes }`
3. **الأمان ضد الأعطال (إلزامي):**
   - كتابة عبر temp file ثم `fsync` ثم `rename` ذري
   - journal بصيغة JSON Lines؛ السجل التالف في نهاية الملف (tail) يُتجاهل عند الإقلاع مع تسجيل تحذير، ولا يفسد المخزن
   - إعادة تشغيل (replay) الـ journal فوق آخر لقطة عند `open`
   - `compact()` يدمج الـ journal في لقطة ويقصّه ذرياً
4. **الأداء:** `upsert` لـ 10,000 سجل يجب أن يكمل في زمن معقول (< 5 ثوانٍ) في اختبار
5. **الاختبارات (إلزامية):**
   - إقلاع من مخزن فارغ
   - إعادة قراءة بعد إعادة `open`
   - سجل tail مقطوع جزئياً → إقلاع ناجح مع تجاهل السجل التالف
   - مقاطعة الكتابة (محاكاة بمحتوى ناقص) → لا فقدان للسجلات السابقة
   - delete ثم compact ثم إعادة فتح → السجل محذوف فعلاً
   - checkpoint لخريطة نطاقات متعددة ثم استرجاعها
   - 10,000 upsert + list + get

## أدلة
- `crates/storage/Cargo.toml` (deps: `serde`, `serde_json`, `thiserror`, `chrono`, `uuid`, `tracing`, `sayl-core`; dev: `tempfile`) — **بدون** أي crate يبني C
- `crates/storage/src/{lib,record,store,journal,error}.rs`
- `crates/storage/tests/store_tests.rs`
- `crates/storage/README.md`

## التحقق
```
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cargo +stable-x86_64-pc-windows-gnu test -p sayl-storage
```