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

### السحب والإفلات (Drag and Drop)
يمكنك سحب أي رابط تنزيل مباشرة من المتصفح أو أي تطبيق آخر وإفلاته داخل نافذة برنامج سيل، وسيتعرف البرنامج على الرابط تلقائياً لتبدأ عملية التنزيل بلمسة واحدة.

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

#### Drag and Drop
Drag any download link directly from your browser or desktop application and drop it into the SAYL window to begin downloading immediately.

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
