<img src="assets/brand/logo-full.svg" alt="SAYL" width="220">

# سيل (SAYL)

[![CI](https://github.com/athar-studios/sayl/actions/workflows/ci.yml/badge.svg)](https://github.com/athar-studios/sayl/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

<!-- [Screenshot placeholder: صورة واجهة برنامج سيل ستوضع هنا عند توفر لقطات الشاشة الرسمية في docs/screenshots/v0.1.0/] -->

**سيل (SAYL)** هو مدير تنزيل مستقل وموثوق لنظام Windows، صُمم ليمنح المستخدم تجربة تنزيل سريعة وآمنة مع الحفاظ التام على الخصوصية. يركز البرنامج على دقة استئناف التنزيلات، وتسريع نقل الملفات عبر الاتصالات المتزامنة، وحماية البيانات دون أي إعلانات أو أدوات تتبع.

---

## جدول المحتويات
- [ما هو سيل؟](#ما-هو-سيل)
- [التحميل والتثبيت](#التحميل-والتثبيت)
- [بدء أول تنزيل والميزات الأساسية](#بدء-أول-تنزيل-والميزات-الأساسية)
- [إضافة المتصفح والتكامل](#إضافة-المتصفح-والتكامل)
- [الإعدادات](#الإعدادات)
- [الأسئلة الشائعة (FAQ)](#الأسئلة-الشائعة-faq)
- [متطلبات التشغيل](#متطلبات-التشغيل)
- [الترخيص والموقع الرسمي](#الترخيص-والموقع-الرسمي)
- [English Documentation](#english)

---

## ما هو سيل؟
سيل هو تطبيق لإدارة وتنزيل الملفات عبر بروتوكولات الويب القياسية (HTTP و HTTPS)، يجمع بين الأداء العالي وواجهة عصرية سهلة الاستخدام تدعم اللغة العربية بالكامل. يقوم البرنامج بتقسيم التنزيلات إلى أجزاء متعددة وتنزيلها بالتوازي لرفع سرعة التحميل إلى أقصى حد يتيحه اتصالك، مع التحقق المستمر من صحة الأجزاء المكتملة وتأكيد عدم تلفها.

أهم ما يميز سيل:
- **استئناف موثوق ومحقق:** فحص دقيق لدعم الخوادم لنطاقات الاستئناف ووسوم التحقق (`ETag`) لمنع أي تداخل أو تلف في الملفات (عند توفر دعم الاستئناف من الخادم وعدم تغير الملف).
- **أمان ومقاومة للتعطل:** حفظ تلقائي لبيانات التنزيل يضمن استعادة المهام المعلقة فوراً في حال إعادة تشغيل الجهاز أو انقطاع الطاقة.
- **خصوصية مطلقة:** لا يجمع التطبيق أي إحصائيات، ولا يحتوي على برمجيات تتبع، وتظل جميع روابط التنزيل وسجلاتها محفوظة على جهازك فقط.

---

## التحميل والتثبيت
يمكنك تنزيل أحدث إصدار من البرنامج من صفحة الإصدارات الرسمية على GitHub:  
👉 **[تحميل سيل — أحدث إصدار](https://github.com/athar-studios/sayl/releases/latest)**

### حزم التثبيت المتوفرة
- **مثبت Windows التنفيذي (موصى به):** `SAYL-Setup-x64-0.1.0.exe`  
  قم بتنزيل الملف وتشغيله لاتباع خطوات التثبيت القياسية على جهازك.
- **حزمة التثبيت الموسعة (MSI):** `SAYL-Setup-x64-0.1.0.msi`  
  خيار بديل ملائم للتثبيت الهادئ أو لمديري الأنظمة في البيئات المؤسسية.

### التحقق من سلامة الملفات
يتم نشر ملف البصمات الرسمية `SHA256SUMS.txt` إلى جانب حزم التثبيت. للتحقق من سلامة الملف المحمل في Windows PowerShell:
```powershell
Get-FileHash .\SAYL-Setup-x64-0.1.0.exe -Algorithm SHA256
```
قارن الناتج بالبصمة المنشورة في ملف `SHA256SUMS.txt` لضمان صحة الملف وعدم تعرضه لأي تعديل.

---

## بدء أول تنزيل والميزات الأساسية

### بدء تنزيل جديد
1. انقر على زر **«إضافة تنزيل»** في شريط الأدوات، أو اضغط على الاختصار `Ctrl+N`.
2. أدخل رابط الملف (URL) المطلوب تنزيله في الحقل المخصص.
3. اضغط على زر **«بدء التنزيل»** للشروع في التحميل فوراً.

### التعامل مع الروابط المكررة
عند إدخال رابط تنزيل موجود مسبقاً في قائمة التنزيلات، يعرض التطبيق نافذة حوار تتيح ثلاثة خيارات واضحة:
- **فتح الموجود:** الانتقال فوراً إلى مهمة التنزيل الحالية واستعراض تفاصيلها.
- **تحميله مرة أخرى:** بدء مهمة تنزيل جديدة ومستقلة للرابط نفسه.
- **إلغاء:** إغلاق نافذة الحوار والتراجع دون إضافة.

### السحب والإفلات (Drag and Drop)
يمكنك سحب أي رابط تنزيل مباشرة من المتصفح أو أي تطبيق آخر وإفلاته داخل نافذة برنامج سيل، وسيتعرف البرنامج على الرابط تلقائياً لتبدأ عملية التنزيل بلمسة واحدة.

### تصنيف الملفات والشريط الجانبي
يوفر سيل شريطاً جانبياً قابلاً للطي والتوسيع لتنظيم الملفات وفق تصنيفاتها مع عداد فوري لعدد الملفات في كل تصنيف:
- **التصنيفات المتاحة:** الكل، فيديو، صوت، مستندات، برامج، ملفات مضغوطة، أخرى.
- **آلية التصنيف:** تُصنف الملفات تلقائياً وفق امتداد اسم الملف أولاً، مع اعتماد نوع المحتوى المستلم عبر ترويسات HTTP (`Content-Type` / MIME) كخيار بديل عند غياب الامتداد أو عدم وضوحه.

### البحث وتصفية المهام
- **البحث السريع:** حقل بحث فوري يتيح تصفية المهام بمطابقة اسم الملف أو جزء من رابط التنزيل (URL).
- **تصفية الحالة:** أزرار سريعة لتصفية المهام حسب حالتها: الكل، يعمل، متوقف، مكتمل، فشل.

### طرق العرض: البطاقات والجدول
يتيح سيل نمطين لعرض قائمة التنزيلات:
- **عرض البطاقات (افتراضي):** واجهة بصرية تعرض كل مهمة كبطاقة تفصيلية توضح نسبة التقدم والحجم وسرعة التحميل.
- **عرض الجدول المضغوط:** جدول مدمج منظم يعرض (الاسم، الحجم، نسبة التقدم، السرعة، الوقت المتبقي، الحالة، وأزرار الإجراءات السريعة).
- يتم حفظ نمط العرض المختار تلقائياً في الإعدادات ويظل ثابتاً عبر جلسات الاستخدام.

### نافذة التحميل العائمة
نافذة مستقلة وصغيرة تطفو على الشاشة لكل تنزيل نشط، تعرض اسم الملف، وحجمه، وشريط تقدم متدرج، والسرعة اللحظية، والوقت المتبقي، وعدد الأجزاء (`Parts`)، مع أزرار الإيقاف المؤقت والاستئناف والإلغاء؛ وعند اكتمال التنزيل تتيح خياري **«فتح الملف»** و**«فتح المجلد»**. وتخضع النافذة لإعدادين في شاشة الإعدادات: «إظهار نافذة التحميل عند بدء كل تحميل» (مفعّل افتراضياً) و«إغلاق النافذة تلقائيًا عند انتهاء التحميل» (معطّل افتراضياً).

### منع نوم الجهاز أثناء التنزيلات
طالما توجد مهام تنزيل نشطة، يطلب التطبيق من نظام التشغيل Windows البقاء في وضع النشاط ومنع الانتقال إلى وضع السكون (`SetThreadExecutionState`)، ويتم تحرير هذا القيد تلقائياً بمجرد اكتمال كافة المهام أو توقفها. تخضع هذه الميزة لخيار **«منع نوم الجهاز أثناء التنزيلات»** في شاشة الإعدادات وهي مفعّلة افتراضياً.

### إيقاف تشغيل الجهاز عند انتهاء التحميلات
خيار اختياري لإيقاف تشغيل الكمبيوتر تلقائياً فور انتهاء كافة التنزيلات، ويمكن تفعيله من داخل نافذة التحميل العائمة أو عبر القائمة الرئيسية لشريط الأدوات (⋮):
- **إعداد مؤقت:** لا يُحفظ هذا الخيار في الإعدادات الدائمة على القرص، ويتم إلغاؤه وإعادة ضبطه تلقائياً عند إعادة تشغيل البرنامج منعاً لأي إغلاق غير مقصود لاحقاً.
- **العد التنازلي للحماية:** عند اكتمال كافة التحميلات، يبدأ عد تنازلي مرئي لمدة 60 ثانية مع زر واضح لـ **«إلغاء»** الإيقاف في أي وقت قبل إغلاق النظام.
- **قواعد الرفض الصارمة:** يمتنع البرنامج تماماً عن إيقاف تشغيل الجهاز إذا كانت هناك أي مهمة فاشلة، أو متوقفة مؤقتاً، أو لا تزال قيد التحميل، أو في حال عدم وجود أي مهام في القائمة.

### اختيار مجلد الحفظ
- تُحفظ التنزيلات افتراضياً في مجلد التنزيلات القياسي الخاص بالنظام (`Downloads`).
- يتم تحديد مجلد الحفظ لكافة التنزيلات من خلال شاشة الإعدادات، حيث يمكنك اختيار المسار المناسب عبر مستعرض المجلدات المدمج.

### الإيقاف المؤقت والاستئناف
- **الإيقاف المؤقت:** اضغط على زر الإيقاف (`Pause`) لأي مهمة قيد التحميل في أي وقت. يتم إنهاء الاتصالات بأمان وتخزين آخر بايت تم التحقق منه على القرص.
- **الاستئناف:** اضغط على زر الاستئناف (`Resume`) لمتابعة التنزيل من النقطة التي توقف عندها، شريطة أن يدعم خادم الملف طلبات النطاقات الجزئية (`Accept-Ranges`) وألا يكون الملف قد تغير على الخادم أثناء التوقف.

---

## إضافة المتصفح والتكامل
يتكامل سيل مع متصفحات الويب المستندة إلى Chromium (مثل Google Chrome وMicrosoft Edge وBrave) عبر الإضافة الرسمية الملحقة:
- **قائمة الزر الأيمن:** عند النقر بزر الفأرة الأيمن على أي رابط داخل المتصفح، اختر **«التحميل باستخدام سيل»** لإرسال الرابط مباشرة إلى تطبيق سيل لسطح المكتب.
- **نافذة الإضافة في شريط الأدوات (Popup):** تفحص وتعرض حالة الاتصال بالمضيف المحلي (`com.sayl.downloader`) وتُظهر إصدار التطبيق عند الاتصال.
- **السلوك التلقائي عند عدم التشغيل:** إذا كان تطبيق سيل مغلقاً أو غير مثبت، تعرض الإضافة أن المضيف غير متصل، ويترك المتصفح الرابط دون تعديل ليتمكن المستخدم من تنزيل الملف عبر المتصفح كالمعتاد.

---

## الإعدادات
يمكنك الوصول إلى لوحة الإعدادات عبر النقر على **أيقونة الترس في شريط الأدوات**:
- **مجلد التنزيل الافتراضي:** تحديد المسار الذي تُحفظ فيه كافة الملفات المكتملة عبر مستعرض المجلدات.
- **عدد الاتصالات المتزامنة:** تخصيص عدد الاتصالات لكل مهمة (الافتراضي 4 اتصالات متزامنة) لموازنة استهلاك عرض النطاق الترددي.
- **طريقة العرض الافتراضية:** التبديل بين نمط البطاقات ونمط الجدول المضغوط، مع حفظ اختيارك تلقائياً.
- **منع نوم الجهاز أثناء التنزيلات:** إبقاء نظام Windows في وضع النشاط طالما هناك مهام قيد التحميل (مفعّل افتراضياً).
- **إظهار نافذة التحميل عند بدء كل تحميل:** فتح نافذة التحميل العائمة تلقائياً عند بدء أي تنزيل جديد (مفعّل افتراضياً).
- **إغلاق النافذة تلقائيًا عند انتهاء التحميل:** إغلاق نافذة التحميل العائمة تلقائياً فور اكتمال الملف (معطّل افتراضياً).
- **اللغة والمظهر:** التبديل الفوري بين اللغة العربية (مع اتجاه RTL) واللغة الإنجليزية، واختيار المظهر المناسب (فاتح أو داكن).

---

## الأسئلة الشائعة (FAQ)

### س: هل يقوم سيل بجمع أي بيانات عن نشاطي أو تتبع ملفاتي؟
**ج:** لا على الإطلاق. يلتزم سيل بسياسة خصوصية صارمة؛ البرنامج خالٍ تماماً من أدوات التتبع (No Telemetry) أو التحليلات الإحصائية أو الإعلانات. كافة بيانات الروابط وسجلات التنزيل تبقى على جهازك الشخصي حصراً.

### س: لماذا يعيد البرنامج تحميل الملف من البداية أحياناً بدلاً من الاستئناف؟
**ج:** يرجع ذلك لضمان عدم تلف الملف الناتج في الحالات التالية:
1. إذا كان الخادم البعيد لا يدعم طلبات النطاقات الجزئية (`Accept-Ranges`).
2. إذا تم تعديل الملف على الخادم أثناء فترة التوقف وتغير وسم التحقق (`ETag`).

### س: هل تضيع الأجزاء المنزلة إذا أُغلق الجهاز فجأة أو انقطع التيار الكهربائي؟
**ج:** لا. يعتمد سيل على آلية تسجيل متينة مع تفريغ دوري للقرص، مما يتيح له استعادة حالة المهام تلقائياً وجعلها جاهزة للاستئناف فور إعادة فتح البرنامج.

---

## متطلبات التشغيل
- **نظام التشغيل:** Windows 10 (الإصدار 1903 أو أحدث) أو Windows 11، بمعمارية 64 بت (x64).
- **بيئة التشغيل:** Microsoft Edge WebView2 Runtime (مثبت افتراضياً في معظم أنظمة Windows الحديثة).
- **المساحة التخزينية:** مساحة كافية على القرص في مجلد الحفظ المختار لاستيعاب الملفات المحملة.

---

## الترخيص والموقع الرسمي
- **الترخيص:** برنامج سيل منشور كبرمجية مفتوحة المصدر بموجب رخصة **Apache License, Version 2.0**. راجع ملف [`LICENSE`](LICENSE) للاطلاع على البنود والشروط.
- **الموقع الرسمي:** [https://sayl.site](https://sayl.site)
- **مستودع الكود والمشاريع:** [https://github.com/athar-studios/sayl](https://github.com/athar-studios/sayl)

---

## English

# SAYL Download Manager

[![CI](https://github.com/athar-studios/sayl/actions/workflows/ci.yml/badge.svg)](https://github.com/athar-studios/sayl/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

<!-- [Screenshot placeholder: Application UI screenshot will be added here once official screenshots are published to docs/screenshots/v0.1.0/] -->

**SAYL** is an independent, high-performance download manager for Windows designed for speed, reliable resuming, and complete user privacy. It delivers fast multi-part downloads, resilient state persistence across crashes, and a modern clean interface with zero telemetry and zero ads.

---

### What is SAYL?
SAYL manages and accelerates file downloads over standard HTTP and HTTPS protocols. It splits files into parallel byte ranges to maximize connection bandwidth, continuously validates received chunks, and ensures that interrupted transfers resume accurately without data corruption.

Key highlights:
- **Verified Resuming:** Strict byte-range validation and strong ETag checking prevent corrupted resumes when servers or files change (provided the remote server supports range requests and the file has not been modified).
- **Crash Resilience:** Durable transaction-safe journaling ensures unfinished tasks recover automatically after sudden reboots or power loss.
- **Absolute Privacy:** No tracking, no analytics SDKs, no ads, and no external telemetry calls. Your download history and URLs stay local on your machine.

---

### Download & Installation
Download the latest official release for Windows directly from GitHub:  
👉 **[Download SAYL — Latest Release](https://github.com/athar-studios/sayl/releases/latest)**

#### Available Packages
- **Standard NSIS Installer (Recommended):** `SAYL-Setup-x64-0.1.0.exe`  
  Download and run the setup executable to install SAYL on your PC.
- **Windows Installer Package (MSI):** `SAYL-Setup-x64-0.1.0.msi`  
  Alternative MSI package suitable for silent installation or enterprise deployment.

#### Verifying Package Integrity
Each release publishes an accompanying `SHA256SUMS.txt` file. You can verify your installer using PowerShell:
```powershell
Get-FileHash .\SAYL-Setup-x64-0.1.0.exe -Algorithm SHA256
```
Compare the output hash against the one published in `SHA256SUMS.txt`.

---

### Starting Your First Download

#### Adding a Download
1. Click **"Add Download"** in the toolbar or press `Ctrl+N`.
2. Paste the target file URL into the dialog.
3. Click **"Start Download"** to initiate the transfer.

#### Handling Duplicate URLs
When adding a URL that already exists in the downloads list, SAYL prompts with a dialog offering three clear actions:
- **Open existing:** Switch directly to the existing download task and view its details.
- **Download again:** Start a new, independent download task for the same URL.
- **Cancel:** Close the dialog and dismiss the addition.

#### Drag and Drop
Drag any download link directly from your browser or desktop application and drop it into the SAYL window to begin downloading immediately.

#### Categories & Collapsible Sidebar
SAYL features a collapsible sidebar that organizes your downloads by file category with live task counts:
- **Available Categories:** All, Video, Audio, Documents, Programs, Compressed, and Other.
- **Automatic Classification:** Files are categorized based on their file extension first, falling back to the HTTP MIME content type (`Content-Type`) when extensions are absent or ambiguous.

#### Search & State Filtering
- **Quick Search:** Real-time search box filtering downloads by matching file name or URL.
- **State Filter:** Quick filter chips to view tasks by status: All, Active (Running), Paused, Completed, and Failed.

#### View Modes: Cards & Compact Table
SAYL offers two view layouts to match your preference:
- **Card View (Default):** Rich visual cards displaying progress, size, and download speed.
- **Compact Table View:** Streamlined tabular layout displaying Name, Size, Progress, Speed, Remaining Time, State, and quick Action buttons.
- Your preferred view mode is saved automatically in Settings and restored across sessions.

#### Floating Download Window
A small floating window for each active download allows monitoring progress outside the main application window:
- **Metrics Displayed:** File name, size, gradient progress bar, real-time speed, estimated remaining time, and active segment count (`Parts`).
- **Direct Controls:** Pause, resume, and cancel buttons.
- **Completion Actions:** Offers **"Open file"** and **"Show in folder"** buttons once the transfer completes.
- **Configuration:** Governed by two preferences in Settings: "Show download window when download starts" (on by default) and "Close window automatically when finished" (off by default).

#### Sleep Prevention During Downloads
To ensure large downloads finish uninterrupted, SAYL requests Windows to remain awake (`SetThreadExecutionState`) while downloads are actively running, and releases this lock automatically once all downloads stop or finish. This behavior is governed by the **"Prevent sleep during downloads"** setting and is enabled by default.

#### Shutdown Computer on Completion
An optional feature to shut down the machine automatically once all downloads complete, accessible from the floating download window and the toolbar's main menu (⋮):
- **Transient Preference:** Never saved to disk; automatically reset upon application restart to prevent unintended future shutdowns.
- **Safety Countdown:** Initiates a visible 60-second countdown prompt with an explicit **"Cancel"** action before initiating system shutdown.
- **Refusal Safeguards:** Strictly refuses to shut down if any task has failed, is paused, is still running, or if there are no download tasks in the list.

#### Save Folder Selection
- By default, files are saved to your system's `Downloads` folder.
- The destination directory for all downloads is configured in Settings using the native folder picker.

#### Pause and Resume
- **Pause:** Click the `Pause` button on any active download at any time. The engine gracefully stops workers and commits written bytes to disk.
- **Resume:** Click `Resume` to continue transferring from where you left off, provided the remote server supports range requests (`Accept-Ranges`) and the remote file validator has not changed.

---

### Browser Extension & Context Menu
SAYL integrates with Chromium-based browsers (Google Chrome, Microsoft Edge, Brave) via its companion extension:
- **Context Menu Integration:** Right-click any link on the web and select **"التحميل باستخدام سيل" / "Download with Sayl"** to send the URL directly to the SAYL desktop application.
- **Toolbar Popup:** Checks and displays whether the native messaging host (`com.sayl.downloader`) is connected, reporting the running app version.
- **Fallback Behavior:** If the SAYL application is closed or not installed, the popup indicates that the host is not connected and the browser leaves the link untouched, allowing the user to download through the browser as normal.

---

### Settings
Open Settings via the **gear button in the toolbar**:
- **Default Download Directory:** Configure where all downloaded files are saved using the native directory picker.
- **Concurrency & Connections:** Adjust maximum concurrent connections per download (default is 4).
- **Default View Mode:** Toggle between Card view and compact Table view, saved automatically.
- **Prevent Sleep During Downloads:** Keep Windows awake while downloads are active (enabled by default).
- **Show Download Window on Start:** Automatically display the floating window when a download begins (enabled by default).
- **Close Window on Finish:** Automatically close the floating download window when complete (disabled by default).
- **Appearance & Language:** Toggle between Arabic (with full RTL support) and English, and select Dark or Light theme modes.

---

### Frequently Asked Questions (FAQ)

#### Q: Does SAYL collect any data about my downloads or usage?
**A:** No. SAYL is built with a zero-telemetry policy. There are no tracking scripts, no third-party analytics, and no remote telemetry servers. All download records and URLs remain strictly on your local device.

#### Q: Why does a download occasionally restart from 0% instead of resuming?
**A:** This safeguards against file corruption if:
1. The remote web server does not support HTTP Range requests (`Accept-Ranges`).
2. The file on the remote server changed while paused (validator or `ETag` mismatch).

#### Q: Are downloaded parts lost if my computer crashes or loses power?
**A:** No. SAYL maintains a write-ahead journal and regularly flushes state to disk. When reopened, any incomplete downloads are restored and ready for resumption.

---

### System Requirements
- **Operating System:** Windows 10 (version 1903 or later) or Windows 11, 64-bit (x64).
- **Runtime:** Microsoft Edge WebView2 Runtime (pre-installed on modern Windows systems).
- **Storage:** Adequate free disk space and write permissions on the designated download directory.

---

### License & Website
- **License:** Licensed under the **Apache License, Version 2.0**. See [`LICENSE`](LICENSE) for details.
- **Official Website:** [https://sayl.site](https://sayl.site)
- **Repository & Issues:** [https://github.com/athar-studios/sayl](https://github.com/athar-studios/sayl)
