# SAYL Chromium Extension (Manifest V3)

امتداد المتصفح لتكامل سيل مع Chromium / Google Chrome / Microsoft Edge.

## هيكل الملفات / File Structure
- `manifest.json`: Manifest V3 definition.
- `background.js`: Service worker handling automatic download interception, context menu downloads, and native messaging communication.
- `rules.mjs`: Pure decision logic for download interception, file extension matching, size verification, and Alt-click bypass.
- `rules.test.mjs`: Unit tests for interception rules using `node:test` and `node:assert`.
- `content.js`: Lightweight content script recording Alt-click events for bypass in session storage.
- `popup.html` & `popup.js`: Bilingual status and settings popup showing end-to-end connection health and configuration.
- `_locales/`: Localization catalogs (Arabic `ar` default, English `en`).
- `icons/`: Extension icons.

## ملاحظة حول الأيقونات / Icons Note
- `16x16.png`, `32x32.png`, `48x48.png` and `128x128.png` are copied from the project brand kit at `assets/brand/` (`icon-16.png`, `icon-32.png`, `icon-48.png`, `icon-128.png`), so each slot gets a real asset at its own size instead of a downscaled copy.
- No new image assets were artificially generated.

## التثبيت اليدوي ومُعرّف الامتداد / Manual Installation & Extension ID
- **Pinned Public Key:** An RSA-2048 public key is embedded in `manifest.json` under `"key"`, ensuring the extension retains a deterministic extension ID across all installations and browsers.
- **Pinned Extension ID:** `cpigidmkbhcfkgljofmmpnfdhdaoldoj`
- **Native Host Allowed Origin:** `chrome-extension://cpigidmkbhcfkgljofmmpnfdhdaoldoj/`

### خطوات التثبيت اليدوي / Manual Installation Steps:
1. Open Chrome/Edge and navigate to `chrome://extensions` or `edge://extensions`.
2. Enable "Developer mode" (وضع مطوّر البرامج).
3. Click "Load unpacked" (تحميل حزمة غير مضغوطة) and select this directory (`extensions/browser/chromium`) or the installed copy under `<install_dir>\extensions\browser\chromium`.
4. Verify that the assigned Extension ID matches `cpigidmkbhcfkgljofmmpnfdhdaoldoj`.
5. The installer automatically registers the native host for this extension ID.
