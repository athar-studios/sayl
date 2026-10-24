# BOOT-001 — وثيقة الحصر وتوجيه التنفيذ (Inventory & Execution Routing)

تاريخ الحصر: 23 أكتوبر 2026  
الدور: Builder (ضمن مسار `coder` / `ag/gemini-3.8-flash-high`)  
حالة المهمة: `blocked` تعاقديًا (لغياب Git snapshot نظيف) مع اكتمال الحصر والأدلة.

---

## 1. ما تم قياسه فعليًا في بيئة العمل (Actual Measured Environment)

تم قياس خصائص البيئة المحلية مباشرة عبر أدوات النظام وكتابة الدليل الخام في `artifacts/BOOT-001/02-environment.json`:

- **نظام التشغيل (OS):**  
  - النظام: Windows 11 (النسخة المعرّفة: `Windows-11-10.0.26300-SP0`، Build `10.0.26300.0`).
  - المعمارية: `AMD64` (x86_64).
- **المعالج (CPU):**  
  - معالج `Intel64 Family 6 Model 154 Stepping 4, GenuineIntel`.
  - عدد الأنوية المنطقية: 12 نواة.
- **التخزين (Disk):**  
  - وحدة التخزين الحالية: `C:`.
  - المساحة الإجمالية: 375.86 جيجابايت (403,571,732,480 بايت).
  - المساحة الشاغرة: 190.12 جيجابايت (204,134,633,472 بايت) بنسبة 50.58% مساحة خالية.
- **الأدوات البرمجية المتوفرة على PATH:**  
  - `python`: الإصدار `3.12.10`.
  - `node`: الإصدار `v26.8.1` في المسار `C:\Program Files\nodejs\node.EXE`.
  - `npm`: الإصدار `11.19.0` في المسار `C:\Program Files\nodejs\npm.CMD`.
  - `git`: الإصدار `git version 2.55.0.windows.5` في المسار `C:\Program Files\Git\cmd\git.EXE`.
  - `powershell`: الإصدار `5.1.26100.9711` في المسار `C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.EXE`.
- **الأدوات غير المتوفرة (Absent from PATH):**  
  - `pwsh` (PowerShell Core 7): غير موجود على PATH؛ الصدفة المتاحة افتراضيًا هي Windows PowerShell 5.1.
  - حزمة Rust: أدوات `cargo` و`rustc` و`rustup` غير مثبتة.
  - حزمة Java: `java` و`javac` غير متوفرين.
  - أدوات Android: `adb` و`sdkmanager` غير متوفرين.
  - مدير الحزم `pnpm`: غير متوفر.
  - أدوات تجميع C++ (MSVC Build Tools): غير متوفرة؛ الملف `vswhere.exe` غير موجود في المسار الافتراضي `%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe`.
- **متغيرات البيئة الخاصة بـ SDKs:**  
  - `ANDROID_HOME`: فارغ / غير معرّف (`null`).
  - `ANDROID_SDK_ROOT`: فارغ / غير معرّف (`null`).
  - `JAVA_HOME`: فارغ / غير معرّف (`null`).
- **حزمة WebView2 Runtime:**  
  - تم فحص مفاتيح الريجستري بوضع القراءة فقط:
    - المفتاح `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`: يحتوي على الإصدار المثبت `pv = 155.0.4283.62`.
    - المفتاح `HKLM\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`: غير موجود.
  - الخلاصة: بيئة تشغيل WebView2 متوفرة بالنظام بإصدار حديث يدعم تشغيل نوافذ Tauri 2.
- **ملاحظة تشغيلية حول TLS والشبكة في الصدفة:**  
  - واجه تنفيذ `Invoke-RestMethod` و`curl.exe` داخل سياق Sandbox الحالي فشل تفاوض أمان Schannel برمز الخطأ `SEC_E_NO_CREDENTIALS` (0x8009030e) نتيجة عزل الصلاحيات المحلية لـ Windows Security Package.
  - تعمل بيئات التشغيل المعتمدة على OpenSSL (مثل Python 3.12 و Node.js v26) باتصال TLS سليم ومباشر دون معوقات.

---

## 2. حالة المستودع و Git (Git Status & State)

- تم تشغيل أمر التحقق: `git status --porcelain=v1 2>&1`.
- المخرج الفعلي: `fatal: not a git repository (or any of the parent directories): .git`.
- رمز الخروج: `128` (محفوظان في `artifacts/BOOT-001/01-git-status.txt` و `01-git-status.exit.txt`).
- الالتزام بالتعليمات: **لا يوجد مستودع Git حاليًا في المجلد**. تم الامتناع تمامًا عن تنفيذ أي أمر من شأنه إنشاء مستودع أو تعديل حالته (`git init`, `add`, `commit`, `push`, `stash`, `clean`, `reset`).
- الأثر التعاقدي: عدم وجود مستودع Git يمنع استخراج `source_commit` و`source_tree` و`snapshot_clean`، مما يجعل تقرير المهمة بحالة `blocked` إلزامية إلى حين تهيئة المستودع رسميًا في المهمة التالية `BOOT-002`.

---

## 3. غياب الدراسة الفنية المحلية (Absence of Local Technical Study)

- لا يوجد في المجلد أو تفرعاته أي ملف يحمل اسم أو محتوى دراسة فنية محلية سابقة.
- التزامًا بمبدأ النزاهة الهندسية في `AGENTS.md` (§1 و §10): تم توثيق هذا الغياب نصًا، مع الحظر الصارم لاختلاق أي افتراضات فنية غير مسندة بوثائق الحزمة الحالية أو القياسات الحية.
- المرجع الفني المعتمد للتصميم والتنفيذ هو `docs/MASTER-PLAN.md` والعقود المرفقة في الحزمة.

---

## 4. فحص المكتبات: الفصل ثلاثي الحالات (Libraries Inventory & Classification)

تم الاستعلام المباشر عن البيانات الوصفية (Metadata GET فقط دون تنزيل حزم أو أرشيفات) من السجلات الرسمية `crates.io API` و `npm registry`.  
تُقسّم المكتبات والأدوات إلى **ثلاث حالات تعاقدية محددة** (تم توثيقها رسميًا مع قرارات التراخيص في `docs/dependencies.md`):

### أولاً: المكتبات المعتمدة للمرحلة الأولى (Approved for M1)
مكتبات تم التحقق من بياناتها الوصفية في السجلات وتوافق تراخيصها، وهي معتمدة لبناء النواة وواجهة المرحلة M1/M2:

| المكتبة / الحزمة | السجل الرسمي | الإصدار المقاس المستقر | الترخيص الرسمي | إصدار Rust الأدنى / المحرك | حالة الحظر (Yanked) | ملف الدليل |
|---|---|---|---|---|---|---|
| `tokio` | crates.io | `1.54.1` | MIT | `1.85` | لا (false) | `artifacts/BOOT-001/registry/tokio.json` |
| `reqwest` | crates.io | `0.13.5` | MIT OR Apache-2.0 | `1.85.0` | لا (false) | `artifacts/BOOT-001/registry/reqwest.json` |
| `serde` | crates.io | `1.0.229` | MIT OR Apache-2.0 | `1.56` | لا (false) | `artifacts/BOOT-001/registry/serde.json` |
| `serde_json` | crates.io | `1.0.154` | MIT OR Apache-2.0 | `1.71` | لا (false) | `artifacts/BOOT-001/registry/serde_json.json` |
| `rusqlite` | crates.io | `0.40.2` | MIT | غير محدد في السجل | لا (false) | `artifacts/BOOT-001/registry/rusqlite.json` |
| `sha2` | crates.io | `0.11.1` | MIT OR Apache-2.0 | `1.88` | لا (false) | `artifacts/BOOT-001/registry/sha2.json` |
| `tracing` | crates.io | `0.1.44` | MIT | `1.65.0` | لا (false) | `artifacts/BOOT-001/registry/tracing.json` |
| `thiserror` | crates.io | `2.0.21` | MIT OR Apache-2.0 | `1.77` | لا (false) | `artifacts/BOOT-001/registry/thiserror.json` |
| `clap` | crates.io | `4.6.7` | MIT OR Apache-2.0 | `1.85` | لا (false) | `artifacts/BOOT-001/registry/clap.json` |
| `tauri` | crates.io | `2.12.5` | Apache-2.0 OR MIT | `1.90` | لا (false) | `artifacts/BOOT-001/registry/tauri.json` |
| `@tauri-apps/api` | npm | `2.12.4` | Apache-2.0 OR MIT | غير محدد | لا (false) | `artifacts/BOOT-001/registry/@tauri-apps-api.json` |
| `@tauri-apps/cli` | npm | `2.12.4` | Apache-2.0 OR MIT | `node >= 10` | لا (false) | `artifacts/BOOT-001/registry/@tauri-apps-cli.json` |
| `svelte` | npm | `5.57.2` | MIT | `node >=18` | لا (false) | `artifacts/BOOT-001/registry/svelte.json` |
| `typescript` | npm | `7.0.2` | Apache-2.0 | `node >=16.20.0` | لا (false) | `artifacts/BOOT-001/registry/typescript.json` |
| `vite` | npm | `8.3.5` | MIT | `node ^20.19.0 \|\| >=22.12.0` | لا (false) | `artifacts/BOOT-001/registry/vite.json` |

### ثانياً: المكتبات المحجوبة بانتظار فحص تجريبي (Blocked pending spike)
مكتبات تم التحقق من بياناتها الوصفية لكنها مقيّدة تعاقديًا في `docs/MASTER-PLAN.md` (§4) بـ spike محدد قبل إقرار اعتمادها أو تجميعها في كود الإنتاج:

| المكتبة / الحزمة | السجل الرسمي | الإصدار المقاس المستقر | الترخيص الرسمي | إصدار Rust الأدنى | قيد الـ Spike المانع | الغرض وحالة القيد | ملف الدليل |
|---|---|---|---|---|---|---|---|
| `librqbit` | crates.io | `9.0.1` | Apache-2.0 | غير محدد في السجل | `TOR-001` | محجوبة عن كود M1؛ اعتماد محرك التورنت مشروط بـ spike في `TOR-001` | `artifacts/BOOT-001/registry/librqbit.json` |
| `uniffi` | crates.io | `0.32.2` | MPL-2.0 | غير محدد في السجل | `AND-001` | محجوبة عن كود M1؛ ربط أندرويد مشروط بـ device spike وتدقيق ترخيص MPL-2.0 في `AND-001` | `artifacts/BOOT-001/registry/uniffi.json` |
| `wasmtime` | crates.io | `50.0.0` | Apache-2.0 WITH LLVM-exception | `1.97.0` | `PLUG-001` | محجوبة ومستبعدة من M1؛ مقيدة بـ Threat Model & Sandbox spike لنظام الإضافات post-M6 في `PLUG-001` | `artifacts/BOOT-001/registry/wasmtime.json` |

### ثالثاً: الأدوات المقترحة غير المفحوصة في السجلات (Proposed / unverified)
أدوات خارجية مقترحة لم تُجلب عبر سجلات crates.io أو npm، وتم عزلها في مجلد `proposed/`:

| الأداة / الحزمة | الغرض المقترح | الحالة الحالية | الموعد التعاقدي للتثبيت والتحقق | ملف التوثيق المرجعي |
|---|---|---|---|---|
| `yt-dlp` | استخراج روابط الوسائط وقوائم التشغيل | Proposed, unverified | يتم تثبيتها واختبار توافقها حصريًا في spike مهمة `MEDIA-001` | `artifacts/BOOT-001/proposed/yt-dlp.json` |
| `FFmpeg` | معالجة الفيديو والدمج وتحويل الصيغ | Proposed, unverified | يتم تثبيتها واختبار توافقها حصريًا في spike مهمة `MEDIA-001` | `artifacts/BOOT-001/proposed/ffmpeg.json` |

---

## 5. خطة البناء للمنصات: Windows و Linux و Android (متاح مقابل مقترح)

### منصة Windows (Desktop M1 / M2):
- **المتاح محليًا:** بيئة Windows 11، Node.js v26.8.1، npm 11.19.0، Python 3.12.10، حزمة WebView2 v155.0.4283.62 جاهزة ومحدثة.
- **المفقود المطلوب:** حزمة Rust (rustup/cargo/rustc) مع استيفاء سقف المكتبات (1.88+ للنواة و 1.90+ لـ Tauri، ويُثبّت الإصدار النهائي في BOOT-002)، وأدوات تجميع MSVC Build Tools.
- **الخطة المقترحة:** تهيئة Rust عبر rustup وتثبيت C++ Build Tools محليًا في BOOT-002 لتمكين تجميع النواة وبناء تطبيق Tauri 2.

### منصة Linux (Desktop / Core M1):
- **المتاح محليًا:** غير متاح (البيئة الحالية Windows NT فقط).
- **الخطة المقترحة:** حظر أي ادعاء بنجاح اختبارات نظام Linux محليًا. تعتمد اختبارات Linux على بيئة CI مؤتمتة أو حاويات منفصلة عند الحاجة، مع تسجيل عدم الانطباق المحلي لأي منصة غير مشغلة.

### منصة Android (Mobile M8):
- **المتاح محليًا:** غير متاح إطلاقًا (لا يوجد Java، لا Android SDK، لا NDK، لا ADB).
- **الخطة المقترحة:** إرجاء كافة متطلبات Android إلى مرحلة M8 ومهمة `AND-001`، حيث يتم تنفيذ spike على جهاز فعلي أو بيئة محاكاة مستقلة، مع عدم تشغيل أي كود أو تكليف أندرويد في المراحل التأسيسية الحالية.

---

## 6. توجيه الموديلات والهوية والميزانية (Model Routing, Identity & Budget)

### توثيق مسارات التنفيذ الفعلية مقابل المخططة (Execution Routing):
- **الـ Builder (هذا الـ Attempt):**  
  تم التنفيذ على مسار (lane) `coder` بالموديل `ag/gemini-3.8-flash-high` عبر المزود `p21566ec00365e463`.
- **الـ Debater (المراجعة المعمارية والمناقشة الفعلية):**  
  تمت مراجعة المناقش واعتراضاته رسميًا على مسار (lane) `Debugger` بالموديل `cmc/deepseek/deepseek-v4.1-flash` عبر المزود `p21566ec00365e463` (وهو موديل مختلف تمامًا ومستقل عن الـ Builder).
- **المسار الأولي/المخطط للـ Debater:**  
  كان المسار المخطط في التكوين الأولي هو `debater` بـ `openrouter/thinkingmachines/inkling:free`، ولكنه **غير متاح** في بيئة الجلسة الحالية (فشل برمز 403 عند محاولة استدعاء lane debater). ولذلك جرى توجيه مراجعة الـ debater الفعلية عبر مسار `Debugger` بالموديل المستقل `cmc/deepseek/deepseek-v4.1-flash`.

### جدول توجيه الموديلات الفعلي والمعدل للجلسة:

| المسار (Lane) | المزود / معرّف الموديل | الدور والمسؤولية في هذه الجلسة | الحالة التشغيلية الفعلية |
|---|---|---|---|
| `coder` | `p21566ec00365e463` / `ag/gemini-3.8-flash-high` | الـ Builder (تنفيذ المهام وإصلاح المخرجات) | نشط (مُنفّذ التقرير الحالي) |
| `Debugger` | `p21566ec00365e463` / `cmc/deepseek/deepseek-v4.1-flash` | الـ Debater الفعلي (المراجعة المستقلة عبر موديل مختلف) | نشط (أجرى مراجعة BOOT-001 واعتراضات D-001 إلى D-005) |
| `debater` | `p21566ec00365e463` / `openrouter/thinkingmachines/inkling:free` | مسار المناقش المخطط أوليًا | غير متاح (403 Forbidden في الجلسة) |
| `PM and planner` | `p21566ec00365e463` / `cmc/moonshotai/Kimi-K2.7-Code` | إدارة وتخطيط المشروع | متاح في Roster |
| `coder helper` | `p21566ec00365e463` / `cmc/Qwen/Qwen3.8-Max` | مساعد البرمجة الإضافي | متاح في Roster |

### إثبات الهوية والاستقلالية وحدود بوابة `preflight`:
- **حقيقة الهوية (`identity_verified = false`):**
  - مجرد تسمية الـ lane أو توجيه الجلسة لا يعد دليلاً قاطعاً على هوية الموديل.
  - نحدد صراحة في التقارير: `identity_source = "runtime_metadata"` و `identity_verified = false` لغياب توقيع مشفّر أو attestation مستقل من المزود.
- **حدود فحص `workflow.py preflight` (فحص تهيئة لا توثيق هوية):**
  - فحص `preflight` في `tools/workflow.py` يقارن فقط سلسلتي النصوص الممررتين عبر متغيرات البيئة (`SAYL_PRIMARY_MODEL` مقابل `SAYL_CHALLENGER_MODEL`).
  - يقبل الفحص أي سلسلتين مختلفتين حتى لو كانت سلسلة وهمية غير موجودة (تم اختباره تجريبيًا من قِبل الـ Debater بـ `fabricated/not-a-real-model:free` وتجاوزه بـ exit code 0).
  - بالتالي فإن `preflight` هو فحص تهيئة وضبط متغيرات (Configuration check) وليس إثباتًا أو تصديقًا لهوية الموديل (Attestation).
- **تحقق استقلالية الموديلين في هذه الجولة:**
  - على الرغم من حدود التحقق التشفيري، فإن المراجعة الفعلية تمت بين موديلين من عائلتين مختلفتين تمامًا (`ag/gemini-3.8-flash-high` للـ Builder و `cmc/deepseek/deepseek-v4.1-flash` للـ Debater على lane الـ Debugger)، مما حقق الغرض التعاقدي من المراجعة المتقاطعة (Cross-model debate).

### التكلفة والميزانية:
- الميزانية المحددة في `agents.config.json`: سقف شهري 20 دولار، واحتياطي 8 دولار، وسقف المهمة الواحدة 0.5 دولار.
- لا توجد أي استدعاءات مدفوعة أو مكالمات LLM تم توليدها من أدوات سير العمل المحلية في هذه المهمة (`tools/workflow.py` يعمل محليًا بالكامل بدون اتصال خارجي).

---

## 7. المهارات المتاحة فعليًا في الجلسة (Skills Inventory)

قائمة الـ Skills المتوفرة في سياق الجلسة الحالي:
1. `diagnose-windows-sandbox-acl`
2. `neon`
3. `neon-ai-gateway`
4. `neon-auth`
5. `neon-functions`
6. `neon-object-storage`
7. `neon-postgres`
8. `neon-postgres-branches`
9. `neon-postgres-egress-optimizer`
10. `neon-realtime`
11. `office-docx`
12. `office-pptx`
13. `office-xlsx`

**توثيق صريح للغياب:**
- لا توجد أي مهارات متخصصة في: `App Design` أو `UI-UX` أو `Accessibility` أو `RTL-Arabic` أو `Security-Testing`.
- لم يتم تثبيت أي مهارة عن بُعد (Remote Skills) التزامًا صارمًا بالبند 4 و 13 في `AGENTS.md`.

---

## 8. أداة Strix غير مثبتة وسبب عدم الانطباق (Strix Non-Applicability)

- أداة Strix غير متوفرة محليًا على الجهاز.
- **التبرير التعاقدي لعدم الانطباق في BOOT-001:**
  - مهمة `BOOT-001` تقتصر على الحصر المعماري، وتوثيق البيئة، وتجهيز عقود المهام وسير العمل.
  - لم يتم إنشاء أو تعديل أي سطر من كود التطبيق، ولا يوجد أي سطح هجومي شبكي أو تطبيقي (attack surface) يمكن فحصه.
  - تم تسجيل ذلك في تقرير المهمة: `strix.required = false` و `strix.executed = false` مع التبرير المذكور.

---

## 9. قرارات معمارية مصغّرة (ADR-lite Decisions)

1. **ADR-001: حظر إنشاء Git Repo في BOOT-001:**  
   - القرار: عدم تشغيل `git init` أو إجراء أي commit في هذه المهمة، والالتزام بحالة `blocked` الصريحة للتقرير حتى يتم تفويض مهمة تأسيس البيئة `BOOT-002`.
2. **ADR-002: حساب سقف متطلبات Rust واعتماد المكتبات الرسمية:**  
   - القرار: استبعاد مكتبة `wasmtime` (التي تتطلب `1.97.0`) بالكامل من حسابات المرحلة M1، لكونها محجوبة تعاقديًا ومؤجلة إلى spike الإضافات ونموذج التهديد (`PLUG-001`) اللاحق لـ M6.
   - الحساب الفعلي للسقف الأدنى (`effective_rust_floor`) لمكتبات النواة المعتمدة لـ M1 فقط (`tokio` 1.85، `reqwest` 1.85.0، `serde` 1.56، `serde_json` 1.71، `rusqlite` غير محدد، `sha2` 1.88، `tracing` 1.65.0، `thiserror` 1.77، `clap` 1.85): الناتج الصريح هو **`1.88`** (أقصى `rust_version` بينها تفرضه مكتبة `sha2`). وإذا أُضيف إطار واجهة سطح المكتب `tauri` v2.12.5 فإن متطلبه هو `1.90`.
   - **ملاحظة إلزامية:** إصدار Rust النهائي المعتمد يُثبَّت رسميًا في ملف `rust-toolchain.toml` وتكوين البيئة خلال مهمة `BOOT-002`.
3. **ADR-003: إرجاء فحص حزم الوسائط (`yt-dlp` و `FFmpeg`):**  
   - القرار: تصنيف أدوات الوسائط كـ "مقترحة وغير مفحوصة"، وعدم الاستعلام عن أي واجهات برمجية خارجية أو GitHub، وتأجيل تثبيتها إلى مهمة `MEDIA-001` المخصصة.
4. **ADR-004: التزام القياس الفعلي وعدم اختلاق أدوات:**  
   - القرار: تسجيل غياب `pwsh` و `rust` و `android` كما تم قياسه بالضبط في النظام، وتوثيق طبيعة مشكلة Schannel الأمنية بنزاهة.

---

## 10. ما لم يتم اختباره صراحة (Explicitly Not Tested)

- لم يتم تجميع أو تشغيل أي كود برمجي لتطبيق SAYL (لا يوجد هيكل كود تطبيقي بعد).
- لم يتم تشغيل أي بيئة ترجمة Rust أو حزم npm (لم يتم تثبيت أي dependencies).
- لم يتم اختبار تشغيل أو استدعاء `yt-dlp` أو `FFmpeg` أو مكتبة التورنت `librqbit`.
- لم يتم إجراء أي اختبار على منصات Linux أو Android أو بيئة المتصفحات الحقيقية.
- لم يتم إجراء فحص أمني بواسطة Strix أو اختبارات اختراق خارجية.
- لم يتم إجراء أي مكالمة LLM عبر واجهات برمجية مدفوعة من أدوات workflow المرفقة.
