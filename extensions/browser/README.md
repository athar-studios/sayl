# SAYL Browser Extensions (Manifest V3)

امتدادات متصفحات الويب لربط المتصفحات بمدير التنزيلات سيل (SAYL).

## حزم الامتدادات / Extension Packages

### 1. Chromium (`extensions/browser/chromium/`)
- متوافق مع: Google Chrome, Microsoft Edge, Brave, وغيرها من متصفحات Chromium.
- **Manifest Version:** 3 (Service Worker).
- **Pinned Key:** RSA-2048 public key ينتج دائمًا المعرّف: `cpigidmkbhcfkgljofmmpnfdhdaoldoj`.
- **Allowed Origin:** `chrome-extension://cpigidmkbhcfkgljofmmpnfdhdaoldoj/`.

### 2. Firefox (`extensions/browser/firefox/`)
- متوافق مع: Mozilla Firefox (109.0+).
- **Manifest Version:** 3 (Background event scripts).
- **Gecko Add-on ID:** `sayl@atharstudios.com`.
- **Allowed Extension:** `sayl@atharstudios.com`.

---

## التطوير والتحميل المباشر / Development & Unpacked Loading

كلا الامتدادين مكتمل وجاهز للتحميل المباشر دون الحاجة لأي خطوة بناء مسبقة:

- **Chrome / Edge:**
  1. افتح `chrome://extensions` أو `edge://extensions`.
  2. فعّل "Developer mode".
  3. انقر "Load unpacked" واختر مجلد `extensions/browser/chromium`.
- **Firefox:**
  1. افتح `about:debugging#/runtime/this-firefox`.
  2. انقر "Load Temporary Add-on...".
  3. اختر ملف `extensions/browser/firefox/manifest.json`.

---

## بناء الحزم المضغوطة / Building ZIP Packages

لتجميع الحزم المضغوطة الجاهزة للتوزيع وفحص صحة الملفات وقواعد JSON والتحقق من صيغة JavaScript:

```powershell
pwsh -NoProfile -File tools/build-browser-extensions.ps1
```

المخرجات تُحفظ في `extensions/browser/dist/`:
- `sayl-chromium-0.1.0.zip`
- `sayl-firefox-0.1.0.zip`

---

## الاتصال بالمضيف الأصلي / Native Messaging Bridge
- **Host Name:** `com.sayl.downloader`
- **Host Binary:** `sayl-bridge.exe`
- **Messages:** `Ping` -> `Pong`, `Download` -> `Accepted` / `Error`.
