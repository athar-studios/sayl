# SAYL Chromium Extension (Manifest V3)

امتداد المتصفح لتكامل سيل مع Chromium / Google Chrome / Microsoft Edge.

## هيكل الملفات / File Structure
- `manifest.json`: Manifest V3 definition.
- `background.js`: Service worker handling context menu link downloads and native messaging communications.
- `popup.html` & `popup.js`: Minimal bilingual (Arabic RTL + English LTR) status popup showing bridge host connection health.
- `icons/`: Extension icons.

## ملاحظة حول الأيقونات / Icons Note
- `16x16.png`, `32x32.png`, `48x48.png` and `128x128.png` are copied from the project brand kit at `assets/brand/` (`icon-16.png`, `icon-32.png`, `icon-48.png`, `icon-128.png`), so each slot gets a real asset at its own size instead of a downscaled copy.
- No new image assets were artificially generated.

## التثبيت اليدوي / Manual Installation
1. Open Chrome/Edge and navigate to `chrome://extensions` or `edge://extensions`.
2. Enable "Developer mode" (وضع مطوّر البرامج).
3. Click "Load unpacked" (تحميل حزمة غير مضغوطة) and select this directory (`extensions/browser/chromium`).
4. Note the Extension ID assigned by the browser.
5. Register the native host using `tools/register-native-host.ps1` with the assigned Extension ID.
