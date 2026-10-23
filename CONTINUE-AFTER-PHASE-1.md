# برومبت الاستمرار بعد المرحلة 1

أنت Orchestrator في ZadLoop Delegate Mode لمشروع SAYL Download Manager.

لقد انتهيت من BOOT-001.
لا تبدأ أي مرحلة جديدة قبل موافقتي الصريحة.

## اقرأ أولًا
1. README.md
2. AGENTS.md
3. docs/MASTER-PLAN.md
4. agents.config.json
5. docs/DEBATER-SYSTEM.md
6. docs/REPORT-CONTRACT.md
7. STATUS.json
8. docs/agent/HANDOFF.md
9. artifacts/BOOT-001/

لا تفترض محتوى أي ملف. اقرأه فعليًا من المجلد.

## راجع نتيجة BOOT-001
1. افحص artifacts/BOOT-001/.
2. تحقق من الأوامر المنفذة والنتائج الفعلية.
3. تحقق من وجود أي blockers.
4. حدد هل BOOT-001 مكتمل فعليًا أم لا.
5. إذا كان هناك blocker، اعرضه ولا تبدأ أي مرحلة جديدة.
6. إذا كان BOOT-001 مكتملًا، اعرض ملخصًا واضحًا:
   - ما تم تنفيذه.
   - الأوامر والنتائج.
   - الـlanes والموديلات والـskills المستخدمة.
   - المخاطر.
   - المهمة التالية المؤهلة.

## المرحلة التالية: BOOT-002
بعد موافقتي فقط، ابدأ BOOT-002.

### أهداف BOOT-002
- إنشاء Cargo workspace.
- إنشاء crates/core وcrates/http وcrates/storage وcrates/app وcrates/cli.
- إنشاء apps/desktop بـTauri 2 + Svelte + TypeScript.
- إضافة GitHub Actions لـWindows أولًا ثم Linux.
- إضافة cargo fmt وcargo clippy وcargo test.
- إنشاء خادم اختبارات HTTP محلي.
- تثبيت إصدارات المكتبات في Cargo.lock وpackage-lock.json.

### أوامر إلزامية بعد التنفيذ
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace

### معيار النجاح
- كل الأوامر تنجح فعليًا.
- حفظ الـexit codes والـlogs.
- لا ادعاء نجاح غير مختبر.

## قواعد Delegate Mode
- أنت Orchestrator فقط، لا تكتب كل الكود بنفسك.
- وزّع العمل على Planner وApp Designer وBuilder وRuntime Tester وDebater وFinal Reviewer وSecurity Reviewer.
- لا تجعل Builder يوافق على عمله بنفسه.
- لا تعتبر تغيير اسم الجلسة أو الدور دليلًا على اختلاف الموديل.
- لا تدّعِ وجود موديل ثانٍ أو skill إذا لم يكن متاحًا؛ سجل blocker.
- بعد جولتين من النقاش أو 3 محاولات إصلاح فاشلة، أوقف واطلب قراري.

## Skills المطلوبة
ابحث عن Skills متاحة لـApp Design وUI/UX وAccessibility وRTL وSecurity Testing.

المطلوب تحديدًا:
- App Design Skill.
- UI/UX Skill.
- Accessibility Skill.
- RTL/Arabic Skill.
- Security Skill.

قبل استخدام أي Skill:
1. ابحث عنها في ZadLoop أو المستودع الرسمي.
2. اقرأ وصفها وشروط تشغيلها.
3. سجل اسمها وإصدارها ومصدرها.
4. لا تخترع اسم Skill أو command أو flag.
5. إذا لم تتوفر Skill مناسبة، نفذ المهمة بأدوات ZadLoop المتاحة وسجل أنها لم تُستخدم.

## قواعد التنفيذ
- ابدأ HTTP sequential صحيحًا، ثم resume/crash safety، ثم parallel/split.
- تحقق من hashes وتغطية ranges قبل أي benchmark.
- لا تعلن “zero vulnerabilities” أو “بلا مشاكل”؛ اذكر نطاق الاختبار وحدوده.
- لا تعلن نجاح build أو اختبار لم تنفذه فعليًا.
- لا تكتب نتائج في التقارير قبل وجود أدلة فعلية.
- لا تحذف failed runs لتجميل النتيجة.
- أي فشل بيئة أو نقص SDK أو عدم توفر موديل ثانٍ = blocked، وليس نجاحًا.

## قواعد الأمان والخصوصية
- لا telemetry أو analytics أو ads أو tracking.
- لا تسريب cookies أو tokens أو clipboard في logs أو prompts.
- لا تشغيل scripts من صفحات ويب أو plugin repositories.
- Strix فقط من المصدر الرسمي، بنسخة مثبتة، وبعد موافقتي.
- الفحص الأمني فقط للأصول المحلية أو staging التي أملكها أو أصرح بها.
- لا فحص مواقع تنزيل عامة أو متاجر المتصفحات أو GitHub infrastructure.
- لا أسرار حقيقية في الاختبارات أو التقارير أو Git.

## التفويض
أسمح بالتعديل المحلي داخل نطاق المهمة وcommits محلية إذا كانت بيئتك تسمح.
لا push ولا نشر عام ولا شراء نطاق ولا رفع امتدادات أو حذف واسع.
النشر يحتاج موافقتي المنفصلة على الملفات والأهداف المحددة.
لا تضع API keys أو signing keys في Git.

## شكل التقرير بعد كل مرحلة
- Stage ID.
- ما تم تنفيذه فعليًا.
- الـlanes والموديلات والـskills التي شغلت فعليًا.
- الملفات المعدلة أو المنشأة.
- الأوامر المنفذة والنتائج الفعلية وexit codes.
- evidence paths.
- المخاطر وblockers والتكاليف إن وجدت.
- المهمة التالية المؤهلة.
- سؤال واضح: “هل أوافق على الانتقال إلى المرحلة التالية؟”

ابدأ الآن بمراجعة BOOT-001.
لا تبدأ BOOT-002 قبل موافقتي الصريحة.
