# جسر المتصفح (Browser Bridge) — عقد الأمان والبروتوكول

يوثّق هذا المستند الحزمة `crates/bridge` التي تسمح لامتداد متصفح بطلب تنزيل رابط من SAYL عبر `native messaging`.

## الحالة الفعلية (اقرأ هذا أولاً)

البروتوكول والتحقق مُنفَّذان ومُختبَران محلياً. **لم يُبنَ أي امتداد متصفح، ولم تُختبر الحزمة داخل Chrome أو Firefox أو Edge في هذه الجلسة**، ولم يُسجَّل host manifest على أي متصفح على هذا الجهاز. ما يثبته الاختبار هو سلوك البروتوكول على تدفق `stdio` في الذاكرة، لا التكامل الحقيقي مع المتصفح.

## الإطار (Framing)

الرسالة = بادئة طول 4 بايت little-endian `u32` ثم جسم JSON بترميز UTF-8، وهي صيغة `native messaging` المعتمدة في Chromium وFirefox.

- سقف صلب: `MAX_MESSAGE_BYTES = 1 MiB`.
- الطول المُعلن يُفحص **قبل** أي تخصيص ذاكرة، فمُرسِل خبيث لا يستطيع إجبار المضيف على حجز غيغابايتات بإعلان طول كبير.
- الطول 0، أو الطول فوق السقف، أو جسم أقصر من المُعلن، كلها أخطاء مصنّفة (`EmptyFrame`, `FrameTooLarge`, `Io`) ولا تُسبب panic.
- إطار غير قابل للتأطير ⇒ تُكتب استجابة `Error` واحدة ثم تتوقف الحلقة، لأن التدفق لم يبقَ متزامناً. إطار مؤطَّر لكن غير قابل للتحليل ⇒ استجابة `Error` وتستمر الحلقة.

## المفردات (Protocol v1)

الطلبات: `Ping { version }`، `Download { url, suggested_filename?, referrer? }`، `Status { task_id }`.
الاستجابات: `Pong { version, app_version }`، `Accepted { task_id }`، `Status { state, bytes }`، `Error { code, message }`.

الطلبات تُحلَّل إلى `enum` مغلق، فأي `type` غير معروف يُنتج `Error` ولا يُفسَّر كأمر. لا يوجد أي أمر ينفّذ برنامجاً أو يقرأ ملفاً من المستخدم.

## ما يثق به المضيف وما لا يثق به

المضيف **لا يثق بشيء يصل على stdin**. حقيقة أن المتصفح هو من أطلق العملية لا تثبت هوية المتصل تشفيرياً: أي عملية محلية تستطيع تمرير نفس الوسيط `chrome-extension://...` في سطر الأوامر. لذلك:

- وسيط الأصل (`argv[1]`) يُسجَّل للتشخيص فقط، وليس رمز تصريح (`CommandLine`).
- كل حقل يُتحقق منه بشكل مستقل: لا قرار مبني على "المتصفح أرسله إذاً هو موثوق".

## سياسة الروابط

`validate_request_url` تفرض:

- المخطط `http` أو `https` فقط. `file:`، `ftp:`، `javascript:`، `data:` كلها مرفوضة.
- رفض الروابط التي تحمل `user:password@`، وكذكاء دفاعي لا تُدرج كلمة المرور في أي رسالة خطأ.
- رفض المحارف التحكمية (حماية من حقن الترويسات) ورفض أي رابط أطول من 8 KiB.
- رفض العناوين المحلية والشبكات الخاصة (`localhost`، `127.0.0.0/8`، `10/8`، `172.16/12`، `192.168/16`، `169.254/16`، `::1`، ومدى IPv6 المحلي) **إلا** إذا فعّل المالك `allow_private_hosts` صراحةً — لأن التنزيل من NAS على الشبكة المحلية حالة مشروعة، لكنها ليست الافتراضي.

## سياسة أسماء الملفات

`sanitize_filename` من `sayl-http` تُدار على الاسم المقترح، ثم:

- أي اسم ينتج فارغاً أو `.` أو `..` يُرفض.
- أي اسم يتجاوز 255 بايت يُرفض.
- `resolve_within` يبني المسار داخل مجلد التنزيل ويرفض أي نتيجة لا تكون ابناً مباشراً له، فلا هروب بـ `..\..\` ولا مسار مطلق.
- الأسماء العربية المشروعة تبقى كما هي دون تغيير.

## ما لا يُسجَّل

لا تُكتب في السجلات: الكوكيز، ترويسات الطلب، أو الروابط الكاملة التي قد تحمل رموز جلسة. رسائل الخطأ تحمل رمزاً مستقراً ووصفاً عاماً.

## التثبيت على Windows (مرجعي، غير مُنفَّذ هنا)

يسجّل Chromium مضيف native messaging عبر مفتاح في الريجستري:

```text
HKCU\Software\Google\Chrome\NativeMessagingHosts\com.sayl.download
```

قيمته مسار ملف manifest بصيغة JSON يحدد `path` إلى ملف `.exe` و`allowed_origins` بقائمة أصول الامتداد المسموح. Firefox يستخدم ملف manifest في مسار `NativeMessagingHosts` داخل بروفايل المستخدم.

**تحذير:** أي `.exe` يُسجَّل هكذا يعمل بصلاحيات المستخدم، ويجب أن يكون موقّعاً في الإصدار النهائي. لا يوجد في هذه الجلسة توقيع ولا مثبّت، و`allowed_origins` يجب أن يُقيَّد بمعرّف الامتداد الحقيقي لا بـ `*`.

## اختبارات

`cargo +stable-x86_64-pc-windows-gnu test -p sayl-bridge` يغطي: الإطار والطول الصفري وفوق السقف والقراءة المقتطعة، JSON تالف، نوع مجهول، جلسة كاملة Ping→Download→Status، رفض الشبكة المحلية والتمكين الصريح لها، حجب بيانات الاعتماد وعدم تسريبها في الخطأ، وتحييد أسماء الملفات المتسللة.

## دليل التشغيل والتثبيت (v0.1.0)

### 1. بناء ثنائية مضيف الرسائل الأصلية (`sayl-bridge`)

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cargo +stable-x86_64-pc-windows-gnu build -p sayl-bridge
```

المسار الناتج للملف التنفيذي:
`target\debug\sayl-bridge.exe` (أو `target\release\sayl-bridge.exe` عند البناء بعلم `--release`).

### 2. تحميل الامتداد في متصفح Chromium / Edge

1. افتح صفحة الامتدادات في المتصفح:
   - Chrome: `chrome://extensions`
   - Edge: `edge://extensions`
2. فعّل خيار **وضع مطوّر البرامج** (Developer mode).
3. انقر على **تحميل حزمة غير مضغوطة** (Load unpacked) واختر المجلد:
   `extensions/browser/chromium`
4. انسخ **معرّف الامتداد** (Extension ID) الظاهر في بطاقة الامتداد (مثل `abcdefghijklmnopqrstuvwxyzabcdef`).

### 3. تسجيل المضيف الأصلي عبر سكربت PowerShell

استخدم `tools/register-native-host.ps1` لتسجيل المضيف في الريجستري تحت المستخدم الحالي (`HKCU`):

```powershell
.\tools\register-native-host.ps1 -HostExe "C:\full\path\to\target\debug\sayl-bridge.exe" -ManifestPath "extensions\browser\host\com.sayl.downloader.json" -ExtensionId "<YOUR_EXTENSION_ID>"
```

يقوم السكربت بالتحقق من وجود الملف التنفيذي على القرص قبل التسجيل، ثم يربط كلا المتصفحين (Google Chrome و Microsoft Edge) بمفتاح:
- `HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader`
- `HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\com.sayl.downloader`

ولإلغاء التسجيل وحذف المفاتيح:
```powershell
.\tools\register-native-host.ps1 -Unregister
```

### 4. حدود الاختبار الفعلية (Honest Limitations)

- تم اختبار البروتوكول ومحرك التوزيع ووظائف التحقق واختبارات الربط في الذاكرة بالكامل وتمر بنجاح عبر:
  `cargo +stable-x86_64-pc-windows-gnu test -p sayl-bridge`
- تم التحقق من سلامة ملفات JavaScript وmanifest عبر `node --check` ومحلل JSON.
- التكامل مع المتصفح واعتراض التنزيلات من داخل بيئة تشغيل متصفح حقيقي لم يُختبر تفاعلياً ضد متصفح مفتوح أثناء هذه الجلسة، وصلاحية التثبيت مقيدة بما تثبته اختبارات الوحدة والسكربتات أعلاه.

