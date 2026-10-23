# سيل | SAYL Download Manager — الخطة الموحّدة

نسخة الخطة: 3.0 — 23 أكتوبر 2026
المالك: محمد عشري
حالة المنتج: مواصفات، وليس تطبيقًا مبنيًا أو مختبرًا.
الاسم: اقتراح إبداعي مؤقت؛ افحص العلامة والنطاق والمتاجر قبل النشر.

## 1. الهدف وحدود الصدق
مدير تحميل Rust مستقل بواجهة Tauri 2 + Svelte + TypeScript، Windows أولًا، بآلية تنزيل ديناميكية مستوحاة من وصف IDM الرسمي، وTimeline حقيقية. ثم Linux وAndroid والوسائط والتورنت والإضافات.

لا نسخ لشفرات أو واجهة IDM، ولا فك DRM أو تجاوز وصول، ولا ادعاء معرفة خوارزميته الداخلية بالكامل. لا ادعاء انعدام الأخطاء أو التسريع الثابت أو ترجمة كل لغات البشر من اليوم الأول. الاختبارات والمراجعات تقلل المخاطر وتوثق ما اختبر فقط.

## 2. أساس تصميم المحرك
وصف IDM الرسمي في Dynamic Segmentation and Performance يبين التقسيم الديناميكي بالنصف، ومساعدة أكبر segment مناسب عند توفر اتصال، وإعادة استخدام الاتصالات، وعدم الاستمرار في تقسيم الأجزاء الصغيرة، وحفظ مواضع التنزيل للاستكمال. هذا وصف سلوك منشور وليس كودًا داخليًا.

قرارنا: أكبر متبقٍ مؤهل هو أساس split، لا «أبطأ اتصال فقط». pending work أولًا، ثم split. سرعة العامل وETA مدخلات لمنع split غير المفيد، لا ضمان مماثلة خوارزمية IDM.

## 3. النطاق والإصدارات
| المرحلة | الناتج | بوابة الاعتماد |
|---|---|---|
| M0 | بيئة وقرارات واختبارات fixtures وربط أدوار | validation + adapter smoke test إن فُعل |
| M1 | CLI + HTTP صحيح + resume + parallel + recovery | hashes + crash campaign + invariant tests |
| M2 | Windows UI + Timeline + queue + themes + i18n | real runtime journey + RTL + accessibility |
| M3 | yt-dlp/FFmpeg + playlists | metadata/progress/finalize/cancel + live dated smoke tests |
| M4 | torrent + selection + streaming/seek | controlled swarm + reference playback |
| M5 | browser extensions + schedule + refresh URL | installed-browser matrix + security review |
| M6 | WASM plugins + catalog pages | external plugin + permission denial tests |
| M7 | download website + docs + privacy + distribution | clean-machine installers + link checks |
| M8 | Android HTTP Preview + SAF + background | real-device lifecycle matrix |
| v1.0 | نطاق منصات ومزايا معلن اجتاز المراجعة | owner release approval |

الموقع والتدويل والخصوصية تبدأ معماريًا مبكرًا؛ لا تؤجل كل تصميمها إلى M7. لا وعد 16 أسبوعًا للمنتج الكامل؛ أعد تقدير الوقت بعد M1. لا تحذف ميزات من backlog صامتًا لتثبيت تاريخ.

## 4. هيكل التطبيق المقترح
```text
crates/core/          models, scheduler contracts, state machine
crates/storage/       SQLite, migrations, checkpoints
crates/http/          HTTP transport, range validation
crates/app/           app services, commands/events contracts
crates/cli/           CLI and later optional daemon
crates/media/         yt-dlp and FFmpeg adapters
crates/torrent/       librqbit adapter
crates/plugin-host/   WASM and host permission broker
apps/desktop/         Tauri + Svelte + TypeScript
apps/android/         Kotlin + UniFFI adapter
apps/website/         static download/docs/catalog site
extensions/browser/  Chromium and Firefox packages
catalog/plugins/     reviewed metadata, not arbitrary remote execution
catalog/themes/      theme metadata and assets
locales/             versioned translation resources
```
أنشئ المكونات عند الحاجة. core مستقل عن UI والمنصة. storage وحده يملك SQL، وfrontend لا ينفذ shell ولا يصل إلى DB. استخدم Tauri commands/events لا daemon مفتوح افتراضيًا. إذا أضيف daemon لاحقًا: loopback + token + validation + limits + origin policy، وليس localhost بلا حماية.

المكتبات المرشحة: Tokio، reqwest، serde، SQLite عبر rusqlite على actor/thread مخصص للـblocking، SHA-256، tracing؛ Tauri 2 وSvelte وTypeScript؛ yt-dlp/FFmpeg؛ librqbit بعد spike؛ UniFFI بعد device spike؛ WASM runtime بعد threat-model spike. تثبت النسخ والرخص في docs/dependencies.md وlockfiles. لا Lua ولا runtime مزدوج للإضافات في البداية. لا تفسر ملف دراسة مفقودًا من الذاكرة.

## 5. سلامة HTTP
- http/https فقط، TLS validation، redirects محدودة، filename sanitization، no overwrite صامت.
- HEAD مساعد لا شرط. GET Range bytes=0-0 probe محدود؛ Accept-Ranges وحده ليس إثباتًا. 206 وContent-Range الصحيحان أساس القبول.
- 200 أثناء probe => sequential آمن دون تصريف الملف كله داخل probe أو append عند offset قديم.
- حجم مجهول أو validator غير موثوق => sequential fresh افتراضيًا؛ لا safe resume مزعوم.
- strong ETag + If-Range للاستكمال والتوازي الآمن. weak ETag لا يستخدم في If-Range. Last-Modified فقط بشروط validator الملائمة وفق HTTP Semantics.
- تغيير المورد أو استجابة غير متوافقة من worker => أوقف cohort واسحب leases، ثم restart آمن أو Failed.
- 416 ليس Completed تلقائيًا؛ re-probe وتحقق total والتغطية.
- اطلب identity encoding وأوقف auto-decompression في range client؛ إن لم يتسق التمثيل فلا parallel ranges.
- تحقق body length وبداية/نهاية range والـtotal قبل اعتماد bytes. truncated body لا يكتمل صامتًا.
- لا تسرب Authorization/cookies عبر redirect لأصل مختلف؛ cookie jar بالنطاقات، لا نسخ headers عشوائيًا.
- رفض CR/LF وframing headers غير المسموحة. logs تحجب tokens وquery secrets.
- proxy وUser-Agent وReferer وcookies بإعدادات واضحة، مع أسرار خارج قاعدة progress العامة.

## 6. Scheduler والكتابة والتقسيم
النطاقات داخليًا [start, end_exclusive)، u64 وchecked arithmetic. تحويل HTTP إلى end_exclusive-1. ملف فارغ مسار خاص. لا gaps/overlaps أو نجاح اعتمادًا على file length فقط.

Scheduler actor واحد يملك segment table وleases. writer actor واحد يملك الملف وbounded queue. كل رسالة كتابة تحمل segment id + lease generation. لا worker يملك seek/write حرًا.

Baseline: اتصالان ثم 4 ثم 8 للاختبار. default سقف 8 عمال للمهمة، 16 عام، 3 مهمات نشطة؛ أرقام مقترحة تُراجع بالقياس. limits لكل origin = scheme/host/port.

Split آمن أولًا عبر bounded subrequests داخل segment منطقي:
1. طلب quiesce من صاحب النطاق.
2. عدم بدء subrequest جديد؛ إكمال الحالي أو إلغاؤه وفق timeout.
3. writer barrier يؤكد حد الكتابة السابق.
4. إبطال generation القديمة ورفض الرسائل المتأخرة.
5. تقسيم الذيل غير المنقول بالنصف عند midpoint معلوم.
6. إعادة إسناد نصف للعامل المتاح وتسجيل SegmentSplit.

لا تعديل Range أُرسل بالفعل وكأن الخادم تلقى نهاية جديدة. أرقام مبدئية: request quantum 8MiB، split متبقٍ >=16MiB، child >=8MiB، cooldown 2s؛ قياس overhead وsplit latency قبل تثبيت أي رقم.

shared client + pool للـreuse، لكن request لا يساوي TCP connection، وHTTP/2 streams قد تتشارك اتصالًا. fixture يقيس socket ids/handshakes والطلبات. لا ضمان reuse لمجرد وجود client واحد.

Retry: 2/4/8 ثوانٍ + jitter، cap 60s و5 محاولات ابتدائيًا؛ Retry-After حيث ينطبق. لا retry أعمى لـ401/403. connect/idle timeouts مستقلة. cancellation تقاطع backoff. 429/503 تخفض الضغط. learning per-origin يأتي بعد baseline، بTTL وسياق proxy/protocol، لا «أفضل سرعة ثابتة للدومين».

## 7. الحالة والتخزين والاستكمال
```text
Queued -> Probing -> Downloading -> Verifying -> Finalizing -> Completed
Downloading -> Pausing -> Paused
Paused -> Probing -> Downloading
nonterminal -> Cancelling -> Cancelled
nonterminal -> Failed
```
الأوامر idempotent. حذف المهمة مختلف عن حذف الملف. phases فرعية للـmerge/export لا progress وهمي. events sequence numbers مع snapshot عند فقد أحداث.

written coverage مختلف عن durable coverage. checkpoint:
write_all -> writer barrier -> file sync ناجح -> SQLite transaction للـsnapshot نفسه -> commit.

SQLite synchronous=FULL مبدئيًا مع journal mode موثق. sync خارج async executor. لا DB write لكل packet. interval مبدئي 2s أو 8MiB مع coalescing، لا fsync storm.

Recovery: session lock، تحقق هوية partial وتغطية DB، re-probe للمورد، ignore uncommitted tails، لا استكمال ملف truncated كأنه كامل. crash بين data sync وDB commit قد يعيد تنزيل bytes غير معتمدة؛ مقبول. العكس غير مقبول.

Finalize قابل للاستعادة: coverage كاملة، hash عند توفر قيمة متوقعة موثوقة، close ثم rename في نفس filesystem بسياسة عدم الاستبدال. hash محلي بلا expected hash بصمة لا إثبات أصالة. MD5 للتوافق فقط إن طُلب لاحقًا.

## 8. مصفوفة الاختبارات والأداء
خادم fixture محلي يولد بيانات deterministic، لا تنزيلات الإنترنت شرطًا لنجاح core CI. السيناريوهات الملزمة:
206 صحيح، لا Accept-Ranges، تجاهل Range/200، Content-Range خاطئ، قطع body/reset، ETag تغير، weak/no validator، 416، HEAD مرفوض، unknown size، empty file، encoding غير متوقع، 429/Retry-After، redirects وأسرار، no space، permission denied، filename malicious/Arabic، pause/cancel أثناء probe/write/retry/split، worker بطيء، old lease، recovery وقت finalize، proxy/TLS، limits، file truncation، pooling.

Scheduler property tests ببذور محفوظة: حدود صالحة، مالك نشط واحد لكل byte، durable لا يتجاوز written، progress غير مضاعف، final coverage كاملة.

Crash campaign: 20 دورة CI صغيرة، 100 دورة محلية قبل M1. أدخل kill عند failpoints محددة وسجل seed والأوقات والhash. process kill ليس power-loss test.

Benchmark release: نفس الملف والخادم والاتصالات وprotocol وproxy، خمس جولات على الأقل وترتيب متبادل. aria2 baseline وIDM اختياري بترخيص مشروع إن توفر. قياس كامل elapsed ووقت النقل منفصلًا، median والتشتت. حالات local/high-RTT/capped/slow-worker/no-Range/429. هدف أولي <=10% أبطأ من aria2 في الحالات المعتمدة، لا ضمان دائم.

هدف engine HTTP واحد <50MiB مع تعريف metric؛ UI والعمليات التابعة مقاسة منفصلة وإجمالًا. هدف CPU idle <1% على جهاز مرجعي خلال 60s مستقرة. الأرقام بوابات مقترحة لا نتائج فعلية.

## 9. الواجهة والثيمات
رحلة حقيقية أولًا: add -> download -> pause/resume -> complete/open. بعدها Timeline ranges/leases/checkpoints/splits/retries من المحرك لا animations مزيفة. Cards وdetails وqueue وDrop Zone وSpotlight. global shortcut بعد اختبار فشل registration. clipboard opt-in بلا network fetch أو تنزيل تلقائي.

progress 5Hz مبدئيًا وتجميع updates، تقليل العمل بالخلفية وreduce motion. keyboard/screen readers، عزل LTR للروابط ضمن RTL. قائمة بديلة للTimeline. 1000 historical items اختبار أداء وvirtualization عند الحاجة.

Theme JSON بإصدار schema: colors/type/density/radii، أربع presets عربي/محايد light/dark. fonts محلية مرخصة. live apply/import/export/reset؛ no remote resources أو raw executable CSS في مسار الثيم الآمن. advanced CSS لاحقًا local opt-in بقيود ومراجعة؛ لا ادعاء sandbox كامل.

## 10. التدويل
البنية تستهدف لغات العالم، لا ادعاء ترجمات مكتملة لكل اللغات. Unicode، keys لكل النصوص بما فيها errors/notifications، locale-aware plural/date/number، no sentence concatenation، RTL/LTR، system locale محليًا وخيار يدوي، fallback معلن.

Arabic/English fully reviewed أولًا، ثم جدول coverage للغات الإضافية. ICU/CLDR-capable library بعد توافق Svelte spike؛ لا ترجمة شبكية وقت التشغيل. pseudo-localization ونص طويل وعربي/عبري/CJK/هندي في الاختبارات. الموقع والتوثيق لهما لغات ونسب اكتمال مستقلة. لا تظهر «دعم كامل» للغة غير مراجعة.

## 11. الفيديو والتورنت
Media spike: pinned yt-dlp/FFmpeg، metadata، structured progress، final filepath بعد post-processing، cancellation يوقف process tree. argv array لا shell، drain stdout/stderr وحدود الحجم/redaction. --print/--progress-template وafter_move تستخدم بعد إثبات CLI المثبت، لا تحليل مخرجات بشرية.

YouTube/TikTok/Instagram/X سيناريوهات يدوية مؤرخة بروابط مصرح بها، لا وعد كل الروابط. quality من metadata، audio-only/MP3 حيث يلزم conversion، phases extract/download/merge/convert. playlists preview/select/index names، serial default لترتيب الإكمال، retry failed فقط. cookies اختيارية صريحة؛ dependencies مثل JS runtime تتحقق وقت spike ولا تُفترض bundled.

لا تجبر yt-dlp على HTTP engine الخاص بنا في v1. updates موقعة/موثقة المصدر مع rollback؛ checksum من مصدر غير موثوق نفسه ليس chain of trust.

Torrent: pinned librqbit spike يثبت magnet/.torrent/DHT/PEX/selection/persistence والـAPIs الفعلية. controlled swarm في CI. seeding settings واضحة. piece handling للمكتبة لا HTTP scheduler. streaming/seek على reference media وcodec معروف، protected loopback stream عند الحاجة وexternal player fallback؛ لا ضمان تشغيل كل codec أو بدء بزمن ثابت.

## 12. امتدادات المتصفح وتحليل الروابط
مصفوفة مستهدفة: Chrome/Edge/Brave/Opera/Vivaldi + Firefox؛ Safari مرحلة macOS لها تطبيق وتوزيع واختبارات منفصلة. الاشتراك في engine لا يعفي اختبار كل browser. لا ادعاء «كل المتصفحات تعمل» قبل evidence matrix.

داخل التطبيق صفحة Browser Integration: اكتشاف متصفحات محليًا، اختيار browser، زر فتح المتجر/مسار التثبيت الرسمي، إرشادات الموافقة، test connection، status/version، uninstall/help. لا silent install أو تجاوز سياسات المتصفح. المتاجر لديها مسارات نشر وسياسات وصلاحيات يجب مراجعتها وقت التنفيذ.

Native Messaging bridge محدود للـextension IDs، schema/size validation، OS registration واختبارات install/uninstall. لا localhost API متاح لصفحات الإنترنت. لا injection في المتصفح أو اعتراض TLS.

Parsing تدريجي: context-menu لرابط مباشر، ثم inspect الصفحة بعد أمر المستخدم وبصلاحية محددة، normalization/dedup، validation، preview قبل الدفعات. لا كل الروابط ملفات تنزيل ولا extension دليل نوع المحتوى. لا scan دائم، لا browsing history، لا cookies عامة. auth context بأقل صلاحية وبموافقة. download takeover لاحقًا بمنع loops والدبل وتنفيذ rollback.

Refresh URL لا يعتمد على اسم وحجم فقط؛ تحقق strong identity أو verification strategy موثوقة، وإلا restart واضح.

## 13. Plugins وThemes catalog
WASM فقط أولًا. hooks: extract، headers مقيدة، post-process عبر host safe operations، ui-tile بيانات declarative لا JS/HTML حر. package = manifest + wasm + README، api_version وpermissions وnetwork allowlist وlimits.

لا FS/network/env افتراضيًا. broker يفحص DNS/redirect/private/loopback/link-local مع DNS rebinding defenses. قيود plugin لا تمنع تنزيل LAN صريح للمستخدم. memory/fuel/wall timeout/output caps، trap لا يسقط app، no arbitrary shell. صلاحيات كل install/update قابلة للرفض، disable/uninstall، source/license/checksum/signature وفق trust model.

المثال الأول extractor خارجي لـfixture مملوك للمشروع، بلا تعديل core؛ ثم مثال موقع حقيقي مسموح. local package أولًا، repo artifact مثبت لاحقًا؛ لا clone+build+execute تلقائي.

صفحتان داخل التطبيق والموقع: Plugins وThemes، تعرضان الاسم/المطور/الإصدار/المصدر/الرخصة/التوافق/الصلاحيات/المراجعة/التثبيت/الإزالة. مراجعة الكتالوج لا ضمان أمان مطلق. الكتالوج static بلا حسابات أو counters أو auto-install؛ remote refresh واضح opt-in، thumbnails محلية أو ضمن نفس نشر الموقع.

## 14. الخصوصية وإثباتها
العبارة المعتمدة: «لا نرسل سجل تنزيلاتك أو روابطك أو ملفاتك إلى خوادمنا. لا يحتوي التطبيق على تحليلات استخدام أو إعلانات أو تتبع. بيانات التنزيل تعالج محليًا، والاتصالات الخارجية اللازمة للتنزيل أو الميزات الاختيارية موضحة.»

لا analytics/ads/fingerprinting/session recording/device IDs. لا crash upload تلقائي. لا clipboard/history upload. no remote fonts/embeds/chat widgets. logs محلية محدودة قابلة للحذف والتصدير بعد redaction. updates/catalog opt-in افتراضيًا، وmanual check يوضح الاتصال. لا telemetry داخل plugin APIs الرسمية؛ plugins الخارجية تعرض نطاقاتها وصلاحياتها ولا نستطيع ضمان نوايا كل كود خارجي.

التطبيق يعالج الرابط والملف ليعمل، فلا نقول «لا نستخدم أي بيانات مطلقًا». خادم الملف يرى اتصالًا، ومستضيف الموقع/GitHub والمتاجر قد يعالجون metadata وفق سياساتهم. راجع hosting logs/retention، قللها حيث نتحكم بها، وافصح عن الحدود. لا تعد بإخفاء IP أو zero logs عند طرف آخر.

Chrome يطلب الإفصاح عن التعامل مع بيانات المستخدم حتى إن بقيت المعالجة محلية.

Network audit من clean profile: idle/add/download/pause/clipboard/theme/plugin/update/site/catalog، destinations + purpose + trigger + payload class. لا user data في الأدلة. unexpected destination blocker. ينتج data-flow.md وnetwork-inventory.md وreport. claim differentiation بلا «نحن الوحيدون» أو مقارنة غير موثقة.

## 15. الموقع والتوثيق والنشر
Static site مقترح لتقليل البيانات والتشغيل؛ GitHub Pages/GitHub Releases أو بديل معتمد بعد تقييم الخصوصية. لا fake repo URL/domain. deployment approval قبل push/publish/DNS/payment.

الصفحات: Home، Download، Documentation/How-to، Browser Integration، Plugins، Themes، Privacy، Security، Releases، GitHub/Issues. Download يعرض OS/arch/version/size/date/min requirements/hash/signature/known issues. لا auto-download دون click، واختيار OS قابل للتغيير.

How-to: install/first-run/HTTP/pause-resume/Timeline/queue/schedule/speed/destination/browser/video/playlists/torrent/seeding/plugins/themes/language/privacy/reset/troubleshooting. screenshots من build فعلية، كل مقال مربوط بالإصدار؛ لا ملفات خاصة في الصور.

كل رابط download يشير artifact موجودًا من نفس release؛ checksums/tags/versions متسقة. GitHub/Docs ظاهر في app والموقع. locale routing/keyboard/mobile/link checker وno-tracker scan.

Tauri يوفر قنوات توزيع وتحديثات يجب ضبطها واختبارها؛ التوقيع وإدارة المفاتيح والـrollback ليست مجرد زر UI.

Windows MSI/NSIS قرار موثق، install/upgrade/uninstall clean machine. Linux build وتشغيل حقيقي لا cross-build فقط. Android APK signing/versionCode. OS installer signing منفصل عن updater signature؛ إن غاب signing سجل limitation ولا تدعِ عدم وجود warnings. secrets خارج Git، CI least privilege، dependencies/license notices/SBOM، migrations وrollback tests.

## 16. Android
Kotlin + UniFFI spike على جهاز فعلي، feature separation يمنع سحب desktop crates. temp app-managed ثم export SAF أولًا؛ لا افتراض random access لدى كل provider، تحقق المساحة وexport recovery.

UIDT على Android 14+ للمهام المناسبة التي يبدأها المستخدم، fallback مدروس للأقدم، WorkManager للأعمال المناسبة له لا ضمان حياة دائمة. notifications وcancel، حفظ حالة دوري. اختبر background/screen off/recent swipe/process kill/reboot/force stop/network switch/permissions/storage منفصلة. لا وعد استمرار بعد force stop.

HTTP Preview أولًا، media/torrent/plugins Android بعد spikes وقرارات توزيع مستقلة.

## 17. سلسلة جودة الـAgents
Specification -> implementation -> self review -> real runtime -> cross-model debate -> fixes+retest -> independent final review -> Strix/security -> fixes+affected reviews -> release verification -> owner approval.

اقرأ docs/DEBATER-SYSTEM.md وagents.config.json. لا يقبل builder عمله وحده. reviewer سياق مستقل ودور read-only؛ يمكنه مشاركة موديل debater لكن ليس builder. أي تغییر للشجرة/commit/acceptance revision يبطل الأدلة والموافقات المتأثرة.

لكل task أو مرحلة تقارير مستقلة. Strix الفعلي عند أسطح attackable وبوابات milestones؛ مراجعة surface لكل مهمة. إذا لم يتوفر binary أو skill أو access، سجل blocked، لا «فحص Strix ناجح» بمعرفة نظرية. Strix Skills تضيف للأدوات المتوافقة مسارات فحص كود/تطبيق/API وCI؛ تثبيتها وتشغيلها يتطلبان مراجعة المصدر والنسخة والتصريح.

النطاق أصولنا المحلية/staging المصرح بها فقط، لا فحص مواقع التنزيل أو متاجر أو GitHub infrastructure. fixtures وأسرار وهمية، no destructive tests/exfiltration. لا كود خاص يُرسل إلى cloud scanner جديد دون إذن. تقرير findings مع confidence/repro/impact/fix/retest. no critical/high unresolved، data corruption أو secret leak دائمًا blocker. المتوسطة حسب المخاطر وموافقة مكتوبة.

تشغيل binary النهائي بعد packaging منفصل عن اختبارات source. لا «zero vulnerabilities»؛ قل النطاق والنسخة وما لم يُختبر. لا نشر تلقائي حتى لو كل المراجعات passed.

## 18. ترتيب المهام
```text
BOOT-001 inventory/local study/constraints/model routing plan
BOOT-002 workspace/CI/state files/test harness
BOOT-003 adapter binding and two-model handoff smoke test
CORE-001 states/errors/events/idempotence
TEST-001 deterministic HTTP server and failure modes
HTTP-001 sequential streaming/safe filenames
HTTP-002 probe/range validation/redirect policy
STORE-001 SQLite migrations/writer/checkpoints
HTTP-003 cancellation/pause/retry
HTTP-004 validator-safe resume
REC-001 sequential crash campaign
HTTP-005 fixed parallel
SCHED-001 lease-safe dynamic splitting
HTTP-006 pool/proxy/headers security
HTTP-007 limits/origin budgets/queue
REC-002 parallel crash/finalize recovery
PERF-001 benchmark and resource baseline
UI-001 real desktop vertical slice
I18N-001 locale architecture and Arabic/English
UI-002 cards/errors/accessibility/RTL
UI-003 actual Timeline
UI-004 drop/Spotlight/clipboard
THEME-001 tokens/presets/live import-export
QUEUE-001 scheduling/restart/timezone
MEDIA-001 pinned tools spike
MEDIA-002 formats/conversion/cancel
MEDIA-003 playlists/failure isolation
TOR-001 compatibility/controlled swarm spike
TOR-002 selection/persistence/limits
TOR-003 reference streaming/seek
WEB-001 native messaging security contract
WEB-002 Chromium browser matrix and install paths
WEB-003 Firefox package and tests
WEB-004 Safari feasibility separate from unsupported claims
WEB-005 authorized link parsing/preview
PLUG-001 runtime/permissions/limits
PLUG-002 external plugin and SDK
PLUG-003 scoped additional hooks
CATALOG-001 in-app and site plugin/theme catalogs
PRIV-001 data map/network audit/privacy copy
SITE-001 static download website and real links
DOCS-001 tested how-to documentation
AND-001 actual-device FFI/background/storage spike
AND-002 HTTP UI/notifications/SAF
SEC-STRIX-001 pinned skills/authorized scans/retest
REL-001 clean-machine installer/upgrade/uninstall
REL-002 updater signatures/tamper/rollback
REL-003 source-to-artifact/site link integrity
REL-004 approved public release
```

التبعيات الفعلية تُكتب في task files قبل التنفيذ، دون فقد ترتيب سلامة HTTP: sequential -> validated resume -> crash safety -> parallel -> split -> UI. site/i18n/privacy design مبكر. لا parallel edits في scheduler/storage أو migrations المشتركة. spikes مستقلة ممكنة بworktrees منفصلة.

## 19. أدلة الإنجاز
حالات: todo/in_progress/review/blocked/done. لا done دون قبول مثبت. فشل SDK أو موديل ثانٍ أو جهاز اختبار => blocked. استخدام fallback نفس موديل builder للـdebate ممنوع.

commands/checks الفعلية محفوظة مع exit codes، timestamps، environment، logs hashes، source commit/tree، acceptance revision. screenshots لا تثبت hash أو أمان. results لا تكتب قبل التنفيذ. review findings لا تحذف عند الإصلاح؛ status/retest يضافان.

أساس Rust بعد وجود workspace:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```
frontend scripts وfeature matrix تنشأ من المشروع الفعلي. لا أوامر packages أو SDK مفترضة. budget يومي/للمهمة، retry caps، لا مجموعات agents تعمل بلا توقف. local validation لا تعني محاسبة مزود مضمونة.

## 20. قرار الإطلاق
لا data-corruption/secret-leak blockers؛ لا critical/high unresolved؛ جميع التقارير لنفس source snapshot؛ artifact hashes مرفقة؛ manifests والروابط حقيقية؛ credentials/signing keys خارج repo؛ privacy/i18n/docs متوافقة مع الواقع؛ مالك المشروع يوافق على targets والمحتوى والتكلفة. النشر وتحديث DNS والمتاجر إجراءات منفصلة تتطلب موافقة.

ابدأ BOOT-001 فقط. لا تبنِ منتجًا وهميًا ولا تعلن نجاح اختبار غير منفذ.
