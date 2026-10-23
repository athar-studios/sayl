# SAYL — حزمة تسليم المشروع إلى AI Coding Agent

هذه حزمة مواصفات وكونفيج وعقود مراجعة، وليست التطبيق النهائي. الاسم «سيل / SAYL» اقتراح لم يُفحص تجاريًا.

## البداية
1. فك الحزمة داخل مجلد مشروع جديد، أو ادمج الملفات بعد مراجعة التعارضات. لا تستبدل تغييرات مستخدم قائمة.
2. اقرأ docs/MASTER-PLAN.md ثم AGENTS.md.
3. سلّم START-HERE-PROMPT.md إلى Agent البرمجة مع صلاحية قراءة المجلد.
4. شغّل `python tools/workflow.py validate` للتحقق من الحزمة محليًا. Python 3.10+.
5. انسخ `.env.example` إلى ملف أسرار محلي أو استخدم متغيرات بيئة آمنة؛ الأداة المرفقة لا تقرأ .env تلقائيًا.
6. عيّن هوية موديل البناء وهوية موديل المناقشة المختلف. `python tools/workflow.py preflight` يتحقق من الإعدادات، ولا يختبر اتصال المزود.
7. يبدأ الـAgent بـBOOT-001 فقط، ثم يحدد ربط أدواتك الفعلية بالأدوار. لا اتصال مدفوع ولا تنزيل dependency لمجرد فك الحزمة.

## مهم: معنى «كونفيج جاهز» هنا
`agents.config.json` ملف إعدادات خاص بهذه الحزمة، يقرؤه الـAgent وأداة workflow المرفقة. ليس صيغة native مضمونة لـCursor أو Claude Code أو Codex أو CrewAI. لا تختلق دعمًا تلقائيًا لأي منصة.

الحزمة تعمل كـfile-based workflow: يقرأ كل دور packet المهمة ويودع تقرير JSON مطابقًا للعقد. الأداة المحلية تتحقق من الإعدادات والتقارير وبوابات القبول. لا تستدعي LLMs ولا تنفذ تطبيقات أو Strix بنفسها. Agent التنفيذ يستخدم أدواته الفعلية، أو ينشئ adapter مع اختبارات عند BOOT-003. هذا فصل مقصود حتى لا ندعي تشغيل فريق بمزود لم يحدده المستخدم.

الوضع الافتراضي `file_handoff`: يمكن تشغيل الجلسات يدويًا بموديل A وموديل B وتسليم packets بينهما. الوضع الآلي لا يُعلن جاهزًا إلا بعد اختبارات adapter الفعلية. تبديل اسم الدور أو session لا يثبت اختلاف الموديل.

## الملفات
- docs/MASTER-PLAN.md: الخطة الموحّدة للمنتج والموقع والإضافات واللغات والخصوصية.
- agents.config.json: الأدوار والصلاحيات والموديلات والميزانية وبوابات الجودة.
- docs/DEBATER-SYSTEM.md: بروتوكول الاعتراض والرد والفصل في الخلاف.
- prompts/roles/: تعليمات كل دور.
- docs/REPORT-CONTRACT.md وtemplates/: عقود وقوالب الأدلة والتقارير.
- tools/workflow.py: validate/preflight/packet/check-report/gate، دون اتصالات خارجية.
- docs/tasks/BOOT-001.md: أول مهمة فقط.
- STATUS.json: حالة أولية؛ لا نتائج اختبارات مزعومة.
- SAYL-FULL-EXECUTION-PROMPT.md: برومبت تشغيل كل المراحل.
- CONTINUE-AFTER-PHASE-1.md: برومبت الاستمرار بعد BOOT-001.
- STAGE-GATE.md: قاعدة التوقف بعد كل مرحلة وطلب موافقة.

## أوامر محلية
```bash
python tools/workflow.py validate
python tools/workflow.py preflight
python tools/workflow.py packet BOOT-001 builder
python tools/workflow.py check-report reports/BOOT-001/builder.json
python tools/workflow.py gate BOOT-001
```

`gate` يفشل ما لم توجد كل التقارير المطلوبة لنفس commit وهوية شجرة الملفات ونفس acceptance revision. نجاح الأداة دليل اتساق ملفات، وليس برهانًا على خلو التطبيق من الثغرات أو أن محتوى تقرير كاذب أصبح حقيقة. تحتاج مراجعة مستقلة وCI وأدلة تشغيل.

## نطاق التفويض
إنشاء/تعديل محلي وcommits محلية فقط عندما تسمح بيئة المستخدم. لا push، لا نشر، لا شراء نطاق، لا متجر، لا تثبيت Skills أو فحص خارجي أو إرسال كود خاص لمزود جديد دون الموافقة المناسبة. أسرار التوقيع وAPI خارج Git.
