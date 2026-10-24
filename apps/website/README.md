# SAYL Website — Static Download Site

موقع تنزيل ثابت (Static) لمشروع **سيل (SAYL)** — مدير تحميل Windows مستقل.

## الهيكل

```
apps/website/
├── index.html      # الصفحة الرئيسية (Arabic RTL + English toggle)
├── styles.css      # تنسيقات RTL-first، قابلة للوصول، متجاوبة
├── app.js          # تبديل اللغة، فحص توفر الملفات، FAQ accordion
├── favicon.svg     # أيقونة الموقع
├── og-image.svg    # صورة Open Graph للمشاركة
└── README.md       # هذا الملف
```

## المتطلبات

- **لا بناء (no build step)** — ملفات HTML/CSS/JS نقية
- **لا npm / لا Node.js** — يعمل مباشرة في المتصفح
- **لا طلبات CDN خارجية** — كل شيء مستضاف ذاتيًا (self-hosted)
- **Python 3.7+** (للمعاينة المحلية فقط)

## المعاينة المحلية

```bash
cd apps/website
python -m http.server 8000
```

ثم افتح `http://localhost:8000` في المتصفح.

> **ملاحظة:** فحص توفر الملفات (`HEAD` requests) يعمل فقط عبر HTTP(S). عند فتح `file://` مباشرة، ستظهر حالة "تعذر التحقق". استخدم الخادم المحلي أعلاه.

## نشر الملفات (Artifacts)

منطقة التنزيل في `index.html` تشير إلى مسارات نسبية يتوقع أن ينتجها خط أنابيب الإصدار (release pipeline):

| زر | مسار الملف النسبي (`data-artifact`) |
|------|--------------------------------------|
| NSIS Installer (.exe) | `releases/SAYL-Setup-x64-<VERSION>.exe` |
| MSI Installer (.msi) | `releases/SAYL-Setup-x64-<VERSION>.msi` |
| Source Archive (.zip) | `releases/SAYL-<VERSION>-source.zip` |
| SHA256SUMS.txt | `releases/SHA256SUMS.txt` |

**أين توضع الملفات فعليًا:**
- في GitHub Releases: تُرفع كملفات مرفقة (assets) للإصدار.
- في GitHub Pages: توضع تحت مجلد `releases/` في فرع النشر (مثلاً `gh-pages` أو `docs/`).
- المسار النسبي في `data-artifact` يفترض أن الموقع يُخدم من جذر المستودع أو من جذر فرع النشر.

**سلوك JS:**
- عند التحميل، يرسل `HEAD` request لكل `data-artifact`.
- **200 OK** → الزر مفعل، حالة "متاح / Available".
- **404 Not Found** → الزر معطل، حالة "لم يُصدر هذا الإصدار بعد / This version has not been released yet".
- **خطأ شبكة / CORS** → الزر معطل، حالة "تعذر التحقق / Failed to check".
- يعيد الفحص تلقائيًا كل 30 ثانية.

## الوصولية (Accessibility)

- `lang="ar" dir="rtl"` افتراضيًا، يتبدل عند تغيير اللغة.
- تباين ألوان WCAG AA.
- حلقات تركيز مرئية (`:focus-visible`).
- نصوص بديلة (`alt`) للصور، تسميات (`aria-label`) للأزرار.
- `prefers-reduced-motion` محترم — لا انتقالات/أنيميشن للمستخدمين الذين يفضلون تقليل الحركة.
- متجاوب حتى 360px عرضًا.
- لا `innerHTML` مع بيانات غير موثوقة — النصوص تُدخل عبر `textContent`.

## الخصوصية

- لا تحليلات (analytics)، لا تتبع (tracking)، لا خطوط خارجية.
- لا `fetch` إلا لفحص توفر الملفات (نفس النطاق).
- لا `localStorage` / `sessionStorage` / Cookies.

## تطوير

للتعديل:
1. عدّل `index.html`، `styles.css`، `app.js` مباشرة.
2. اختبر محليًا عبر `python -m http.server`.
3. تحقق من Console — يجب أن يكون نظيفًا (لا أخطاء).
4. اختبر التبديل للإنجليزية: اضغط زر `EN` في الهيدر.
5. اختبر وضع RTL/LTR في أدوات المطور.

## الترخيص

جزء من مشروع SAYL. راجع `LICENSE` في جذر المستودع (لم يُحسم بعد).