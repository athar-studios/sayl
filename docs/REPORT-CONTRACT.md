# عقد التقارير والأدلة

كل تقرير JSON لrole واحد وtask واحد وsource snapshot واحد. القالب في templates/report.template.json غير صالح كنجاح جاهز؛ حقوله null/blocked مقصودة.

الحقول:
- task_id، role، status: passed/blocked/failed.
- source_commit، source_tree: Git hashes فعلية (40 أو64 hex)، snapshot_clean=true.
- acceptance_revision: hash task spec؛ لا تتغير المواصفات صامتًا.
- model: provider/id، identity_source: runtime_metadata أو self_declared أو unknown. identity_verified=true فقط عند وجود سجل أداة فعلية يؤيدها، لا ثقة عمياء في كلام الموديل.
- evidence: array من {path, sha256} لملفات داخل المشروع، موجودة ولا تحتوي أسرارًا.
- findings: {id,severity,blocking,state,...}. blocker مفتوح يمنع passed.
- checks: الأوامر وexit codes وlog paths؛ builder/tester/security/release يرفقون تشغيلًا ملائمًا للدور.
- strix: required، executed، reason، command/evidence حسب scope. required=true وexecuted=false يمنع gate.
- human_deployment_approval=false افتراضيًا؛ gate لا يُصدر موافقة نيابة عن المالك.

workflow helper يتحقق من البنية والأدلة والhashes والاتساق. لا يثبت أن مزودًا خارجيًا صدّق الهوية، أو أن تقريرًا مخترعًا أصبح حقيقة. CI وأذونات فصل الأدوار ومراجعة المالك لازمة. التحقق من artifact SHA النهائي منفصل عن source snapshot.

## snapshot
قبل المراجعة النهائية اصنع commit محليًا نظيفًا إذا مصرحًا؛ لا تغييرات untracked مؤثرة. استبعد reports/artifacts/runtime عبر gitignore، لا ملفات app. سجل git rev-parse HEAD وHEAD^{tree}. لا use commit لشجرة بينما اختبرت شجرة dirty.

## task vs release
gate task يتطلب reports للأدوار الأساسية؛ release gate يزيد release_engineer. BOOT-001 لا يملك سطح تطبيق بعد: يمكن security reviewer تبرير عدم انطباق Strix داخل التقرير؛ لا يدّعي executed. لا تعتبر BOOT-001 مكتملة بالكامل بدون موديل ثانٍ والمراجعات إذا كانت سياسة الأدوار إلزامية؛ يمكن تسليم inventory مع status blocked لحين توفرهما.

Release verification: بني binary من snapshot نفسه، hash artifact، اختبر الحزمة لا source فقط، سجل OS/arch/signature/install-upgrade-uninstall وsite links. نشر public يظل موافقة منفصلة خارج gate.
