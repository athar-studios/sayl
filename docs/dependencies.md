# سجل الاعتماديات والمكتبات (Dependencies Inventory & Tracking)

هذا المستند هو المرجع الرسمي المعتمد لإصدارات المكتبات الخارجية، وحالات التحقق، وقرارات التراخيص لمشروع SAYL Downloader وفقًا لمتطلبات النزاهة الهندسية.

---

## 1. تصنيف حالات الاعتماديات (Status Taxonomy)

تنقسم الاعتماديات في المشروع إلى ثلاث فئات محددة تعاقديًا:
1. **`Approved for M1`**: مكتبات تم التحقق من بياناتها الوصفية وتراخيصها وتوافقها مع متطلبات النواة وواجهة المرحلة الأولى (M1/M2).
2. **`Blocked pending spike`**: مكتبات مؤكدة في بنية المشروع لكنها مقيّدة تعاقديًا بانتهاء فحص تجريبي (Spike) تقني أو أمني محدد قبل اعتماد استخدامها أو تجميعها في كود الإنتاج.
3. **`Proposed / unverified`**: أدوات ومكتبات خارجية مقترحة لم تُفحص بعد في السجلات الرسمية، ومؤجلة بالكامل إلى المهام المحددة لها.

---

## 2. جدول الاعتماديات الشامل (Master Dependencies Table)

| الحزمة / المكتبة | السجل الرسمي | الإصدار المقاس المستقر | الترخيص الرسمي | إصدار Rust الأدنى / المحرك | مرجع القيد (Spike Ref) | الحالة (Status) | قرار الترخيص (License Decision) | ملف الدليل |
|---|---|---|---|---|---|---|---|---|
| `tokio` | crates.io | `1.54.1` | MIT | `1.85` | — | **Approved for M1** | معتمد (Permissive MIT) | `artifacts/BOOT-001/registry/tokio.json` |
| `reqwest` | crates.io | `0.13.5` | MIT OR Apache-2.0 | `1.85.0` | — | **Approved for M1** | معتمد (Dual permissive MIT/Apache-2.0) | `artifacts/BOOT-001/registry/reqwest.json` |
| `serde` | crates.io | `1.0.229` | MIT OR Apache-2.0 | `1.56` | — | **Approved for M1** | معتمد (Dual permissive MIT/Apache-2.0) | `artifacts/BOOT-001/registry/serde.json` |
| `serde_json` | crates.io | `1.0.154` | MIT OR Apache-2.0 | `1.71` | — | **Approved for M1** | معتمد (Dual permissive MIT/Apache-2.0) | `artifacts/BOOT-001/registry/serde_json.json` |
| `rusqlite` | crates.io | `0.40.2` | MIT | غير محدد | — | **Approved for M1** | معتمد (Permissive MIT) | `artifacts/BOOT-001/registry/rusqlite.json` |
| `sha2` | crates.io | `0.11.1` | MIT OR Apache-2.0 | `1.88` | — | **Approved for M1** | معتمد (Dual permissive MIT/Apache-2.0) | `artifacts/BOOT-001/registry/sha2.json` |
| `tracing` | crates.io | `0.1.44` | MIT | `1.65.0` | — | **Approved for M1** | معتمد (Permissive MIT) | `artifacts/BOOT-001/registry/tracing.json` |
| `thiserror` | crates.io | `2.0.21` | MIT OR Apache-2.0 | `1.77` | — | **Approved for M1** | معتمد (Dual permissive MIT/Apache-2.0) | `artifacts/BOOT-001/registry/thiserror.json` |
| `clap` | crates.io | `4.6.7` | MIT OR Apache-2.0 | `1.85` | — | **Approved for M1** | معتمد (Dual permissive MIT/Apache-2.0) | `artifacts/BOOT-001/registry/clap.json` |
| `tauri` | crates.io | `2.12.5` | Apache-2.0 OR MIT | `1.90` | — | **Approved for M1** | معتمد (Dual permissive MIT/Apache-2.0) | `artifacts/BOOT-001/registry/tauri.json` |
| `@tauri-apps/api` | npm | `2.12.4` | Apache-2.0 OR MIT | غير محدد | — | **Approved for M1** | معتمد (Dual permissive MIT/Apache-2.0) | `artifacts/BOOT-001/registry/@tauri-apps-api.json` |
| `@tauri-apps/cli` | npm | `2.12.4` | Apache-2.0 OR MIT | `node >= 10` | — | **Approved for M1** | معتمد (Dual permissive MIT/Apache-2.0) | `artifacts/BOOT-001/registry/@tauri-apps-cli.json` |
| `svelte` | npm | `5.57.2` | MIT | `node >=18` | — | **Approved for M1** | معتمد (Permissive MIT) | `artifacts/BOOT-001/registry/svelte.json` |
| `typescript` | npm | `7.0.2` | Apache-2.0 | `node >=16.20.0` | — | **Approved for M1** | معتمد (Permissive Apache-2.0) | `artifacts/BOOT-001/registry/typescript.json` |
| `vite` | npm | `8.3.5` | MIT | `node ^20.19.0 \|\| >=22.12.0` | — | **Approved for M1** | معتمد (Permissive MIT) | `artifacts/BOOT-001/registry/vite.json` |
| `librqbit` | crates.io | `9.0.1` | Apache-2.0 | غير محدد | `TOR-001` | **Blocked pending spike** | مقيد بـ spike التورنت (Apache-2.0 مسموح) | `artifacts/BOOT-001/registry/librqbit.json` |
| `uniffi` | crates.io | `0.32.2` | MPL-2.0 | غير محدد | `AND-001` | **Blocked pending spike** | مشروط بسياسة MPL-2.0 (انظر الملاحظة أدناه) | `artifacts/BOOT-001/registry/uniffi.json` |
| `wasmtime` | crates.io | `50.0.0` | Apache-2.0 WITH LLVM-exception | `1.97.0` | `PLUG-001` | **Blocked pending spike** | مقيد بـ spike نموذج التهديد وعزل الإضافات post-M6 | `artifacts/BOOT-001/registry/wasmtime.json` |
| `yt-dlp` | غير متوفر (Proposed) | غير مقاس (تثبيت لاحق) | Unlicense / public domain | N/A | `MEDIA-001` | **Proposed / unverified** | مؤجل لمرحلة تقييم أدوات الوسائط | `artifacts/BOOT-001/proposed/yt-dlp.json` |
| `FFmpeg` | غير متوفر (Proposed) | غير مقاس (تثبيت لاحق) | LGPL-2.1+ / GPL-2.0+ | N/A | `MEDIA-001` | **Proposed / unverified** | مؤجل لمرحلة تقييم تراخيص الوسائط والربط | `artifacts/BOOT-001/proposed/ffmpeg.json` |

---

## 3. قرارات التراخيص والملاحظات القانونية (License Decisions & Notes)

### 3.1. التراخيص المتساهلة (Permissive Licenses: MIT, Apache-2.0)
- كافة مكتبات النواة لـ M1 (`tokio`, `reqwest`, `serde`, `serde_json`, `rusqlite`, `sha2`, `tracing`, `thiserror`, `clap`, `tauri`, `vite`, `svelte`, `typescript`) تستخدم تراخيص MIT أو Apache-2.0 (أو ترخيصًا مزدوجًا بينهما).
- القرار: معتمدة بالكامل دون قيود نشر تجاري أو توزيع ثنائي مغلق، مع الالتزام بتضمين إشعارات الحقوق (Copyright Notices) وملفات الترخيص في الحزمة المجمعة.

### 3.2. ملاحظة ترخيص MPL-2.0 لمكتبة UniFFI (Weak Copyleft Note)
- رخصة `MPL-2.0` (Mozilla Public License Version 2.0) هي رخصة حقوق تشاركية ضعيفة على مستوى الملفات (File-level copyleft).
- **الالتزامات القانونية:**
  1. أي تعديلات أو تحسينات يتم إجراؤها على الملفات الأصلية التابعة لمكتبة UniFFI ذاتها يجب أن تظل مفتوحة المصدر تحت رخصة MPL-2.0.
  2. لا تسري شروط MPL-2.0 على الشيفرة البرمجية الخاصة بتطبيق SAYL التي تستخدم UniFFI للربط (FFI bindings) ما دامت محفوظة في ملفات مستقلة ومنفصلة (Separate Files / Larger Work).
- **القرار الهندسي:**
  - معتمد بشرط عدم تعديل أي ملف مصدري داخلي لمكتبة UniFFI، واستخدامها حصريًا كمولد أكواد ربط وحزمة وقت تشغيل مستقلة.
  - سيتم التحقق العملي من العزل وسلسلة التوريد في مهمة `AND-001`.

### 3.3. أدوات وتراخيص الوسائط (FFmpeg / yt-dlp)
- `FFmpeg` يحمل ترخيص LGPL-2.1+ افتراضيًا، أو GPL-2.0+/GPL-3.0+ في حال تفعيل مكتبات غير حرة أو مرخصة بـ GPL (مثل libx264).
- `yt-dlp` يحمل ترخيص The Unlicense.
- **القرار الهندسي:**
  - يتم استدعاؤهما كعمليات خارجية منفصلة (external processes / sidecars) وتجنب الربط الساكن (Static Linking) بـ FFmpeg لمنع انتقال متطلبات GPL إلى نواة SAYL.
  - التثبيت والتأكيد القانوني النهائي سيتم في مهمة `MEDIA-001`.

---

## 4. حساب سقف متطلبات Rust (Effective Rust Floor)

- **المكتبات المعتمدة لـ M1 فقط:**
  - `tokio`: 1.85
  - `reqwest`: 1.85.0
  - `serde`: 1.56
  - `serde_json`: 1.71
  - `rusqlite`: غير محدد
  - `sha2`: **1.88** (أعلى إصدار مطلوب بين مكتبات النواة التسع)
  - `tracing`: 1.65.0
  - `thiserror`: 1.77
  - `clap`: 1.85
  - **السقف الأدنى الفعلي لمكتبات النواة (Core M1 Effective Floor):** `1.88`
- **إطار عمل واجهة سطح المكتب (Desktop Framework):**
  - `tauri` (v2.12.5): `1.90`
- **المكتبات المحجوبة (Blocked / Deferred):**
  - `wasmtime` (50.0.0): `1.97.0` — مستبعد تمامًا من حسابات M1 لأنه مقيد بمهمة `PLUG-001` اللاحقة للمرحلة M6.
- **القرار:** الإصدار النهائي المعتمد لـ Rust Toolchain سيتم تثبيته رسميًا في ملف `rust-toolchain.toml` وتكوين البيئة في مهمة `BOOT-002`.
