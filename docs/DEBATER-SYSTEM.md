# Cross-model Debater System — v1

## الهدف
اكتشاف أخطاء التصميم قبل دمج الكود، لا الوصول إلى اتفاق شكلي. Builder=A وDebater=B بهوية موديل مختلفة فعليًا. Final reviewer جلسة مستقلة على B أو موديل ثالث، لا يحتاج اشتراكًا ثالثًا تلقائيًا. تحقق من التكلفة والخصوصية قبل إرسال الشفرة.

## الحزمة التي يراها المناقش
- task + acceptance revision + scope/dependencies.
- base commit، tested commit، git tree id، changed paths، clean snapshot.
- diff والأجزاء اللازمة من العقود، لا كل history.
- أسباب مختصرة من Builder: المشكلة/القيود/الاختيار/بديلين إن وُجدا/سبب الرفض/المخاطر.
- أوامر الاختبارات وexit codes وlogs وإشارات ما لم يختبر.
- threat-model delta وبيانات الأداء إن ادعي تحسن.

## فصل الاستقلال
يكتب debater تقييمًا أوليًا مستقلًا للمواصفات والdiff قبل قراءة خاتمة builder النهائية عند الإمكان. يرى بعدها أسباب الاختيار ليفحصها. final reviewer لا ينسخ قرار debater؛ يتحقق من final snapshot والأدلة بنفسه.

الهوية: provider + model id/revision من تشغيل فعلي عند توفره. عبور provider لنفس الموديل ليس بالضرورة اختلاف موديل. تغيير label أو system prompt لا يحقق شرط المستخدم. إذا أخفت المنصة الهوية أو تعذر إثباتها، سجل قيدًا واطلب قرارًا؛ لا self-attestation مبالغًا فيها.

## الجولة الأولى
Debater يسجل حتى خمسة اعتراضات ذات قيمة؛ لا حد للأخطاء الخطيرة لمجرد بلوغ خمسة. لكل اعتراض:
```json
{
  "id": "D-001",
  "severity": "high",
  "blocking": true,
  "claim": "سبب محدد قابل للتحقق",
  "location": "path:line or design contract",
  "failure_scenario": "مدخلات/توقيت يفشل فيه التصميم",
  "proposed_test": "اختبار يعزل المشكلة",
  "alternative": "بديل مع tradeoff مختصر",
  "state": "open"
}
```

Builder يرد لكل ID: accept-and-fix أو evidence-based-rebuttal أو request-owner-decision. لا رأي مجردًا مثل «أفضل performance» دون قياس. تغيير acceptance لإسكات الاعتراض ممنوع دون موافقة مالك المتطلب.

## الجولة الثانية
Debater يتحقق من fixes ونتائج tests لنفس snapshot. يكتب closed/further-evidence/owner-decision. objection يمكن إغلاقه بدليل صحيح على عدم انطباقه، لا يلزم تغيير الكود لمجرد الخلاف. disagreement مهني لا تصويت.

بعد جولتين: blocker => blocked ويُرفع للمالك. لا endless debate. الخلاف التجميلي غير blocker إن لا ينتهك acceptance.

## ما يوجب الرفض
range overlap/gap، crash data loss، false complete، stale write، secret leak، privilege escalation، broad browser permissions، telemetry خفية، unsupported locale claim، fake benchmark، undeclared outbound calls، update trust gap، أو اختبارات مفقودة ضمن scope.

## القرار والتوثيق
debaters لا يكتبون الكود. builder فقط ينفذ fixes ضمن task scope. تقرير debater النهائي يشمل findings وdecision وresidual risks وأسباب مختصرة لا hidden chain-of-thought. حدث ADR عند تعديل تصميم جوهري، واطلب إعادة الاختبارات والمراجعات المتأثرة.

## بناء الثقة دون ادعاء زائد
موديلان قد يشتركان في نفس الخطأ. لذلك الاتفاق لا يغني عن deterministic tests وruntime وstatic/security checks. ممنوع الإبلاغ عن «تطبيق بلا مشاكل»؛ اذكر نطاق التحقق وحدوده.
