# SAYL Firefox Extension (Manifest V3)

امتداد المتصفح لتكامل سيل مع Mozilla Firefox.

## هيكل الملفات / File Structure
- `manifest.json`: Manifest V3 definition with Gecko ID `sayl@atharstudios.com`.
- `background.js`: Background event page handling context menu link downloads and native messaging communications via `browser.*` Promise API.
- `popup.html` & `popup.js`: Bilingual (Arabic RTL + English LTR) status popup showing bridge host connection health.
- `_locales/`: Localized messages for Arabic (`ar`) and English (`en`).
- `icons/`: Extension icons (16px, 32px, 48px, 128px).

## التثبيت اليدوي ومُعرّف الإضافة / Manual Installation & Add-on ID
- **Add-on ID:** `sayl@atharstudios.com`
- **Native Host Allowed Extension:** `sayl@atharstudios.com`
- **Native Host Name:** `com.sayl.downloader`

### خطوات التحميل كإضافة مؤقتة للتطوير / Temporary Add-on Installation Steps:
1. Open Firefox and navigate to `about:debugging#/runtime/this-firefox`.
2. Click "Load Temporary Add-on..." (تحميل إضافة مؤقتة...).
3. Select `extensions/browser/firefox/manifest.json`.
4. Verify that the Extension ID shows `sayl@atharstudios.com`.
