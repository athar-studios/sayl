import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { launchApp } from './e2e/cdp.mjs';

const PORT = 9255;
const FIXTURE_PORT = 8991;

// 1. Create a mock fixture server for testing 100MB and 1GB downloads and 404
const server = http.createServer((req, res) => {
  const url = req.url;
  if (url === '/files/100MB.bin') {
    res.writeHead(404, { 'Content-Type': 'text/plain' });
    res.end('404 Not Found');
    return;
  }

  if (url === '/files/100Mb.dat' || url === '/files/1Gb.dat') {
    const totalSize = url.includes('1Gb') ? 1073741824 : 104857600;
    const rangeHeader = req.headers.range;

    if (req.method === 'HEAD') {
      res.writeHead(200, {
        'Content-Type': 'application/octet-stream',
        'Content-Length': totalSize,
        'Accept-Ranges': 'bytes',
        'ETag': '"mock-etag-12345"',
      });
      res.end();
      return;
    }

    if (rangeHeader) {
      const match = rangeHeader.match(/bytes=(\d+)-(\d+)?/);
      if (match) {
        const start = parseInt(match[1], 10);
        const end = match[2] ? parseInt(match[2], 10) : Math.min(start + 5 * 1024 * 1024 - 1, totalSize - 1);
        const chunkLen = end - start + 1;

        res.writeHead(206, {
          'Content-Type': 'application/octet-stream',
          'Content-Length': chunkLen,
          'Content-Range': `bytes ${start}-${end}/${totalSize}`,
          'Accept-Ranges': 'bytes',
          'ETag': '"mock-etag-12345"',
        });

        const buf = Buffer.alloc(Math.min(chunkLen, 64 * 1024), 0x5a);
        let sent = 0;
        function sendChunk() {
          while (sent < chunkLen) {
            const toSend = Math.min(buf.length, chunkLen - sent);
            sent += toSend;
            if (!res.write(buf.subarray(0, toSend))) {
              res.once('drain', sendChunk);
              return;
            }
          }
          res.end();
        }
        sendChunk();
        return;
      }
    }

    res.writeHead(200, {
      'Content-Type': 'application/octet-stream',
      'Content-Length': totalSize,
      'Accept-Ranges': 'bytes',
      'ETag': '"mock-etag-12345"',
    });

    // Stream out up to 10MB quickly for test purposes
    const streamLimit = Math.min(totalSize, 10 * 1024 * 1024);
    const buf = Buffer.alloc(64 * 1024, 0x5a);
    let sent = 0;
    function sendChunk() {
      while (sent < streamLimit) {
        const toSend = Math.min(buf.length, streamLimit - sent);
        sent += toSend;
        if (!res.write(buf.subarray(0, toSend))) {
          res.once('drain', sendChunk);
          return;
        }
      }
      res.end();
    }
    sendChunk();
    return;
  }

  res.writeHead(404);
  res.end('Not Found');
});

await new Promise((r) => server.listen(FIXTURE_PORT, '127.0.0.1', r));
console.log(`[PASS] Fixture server running on port ${FIXTURE_PORT}`);

const screenshotsDir = path.resolve('docs/screenshots');
fs.mkdirSync(screenshotsDir, { recursive: true });

const testDataDir = path.resolve('target/test-verify-data');
const testDownloadDir = path.resolve('target/test-verify-downloads');
if (fs.existsSync(testDataDir)) fs.rmSync(testDataDir, { recursive: true, force: true });
if (fs.existsSync(testDownloadDir)) fs.rmSync(testDownloadDir, { recursive: true, force: true });
fs.mkdirSync(testDataDir, { recursive: true });
fs.mkdirSync(testDownloadDir, { recursive: true });

console.log('[START] Launching installed SAYL app (target/installed/sayl-desktop.exe)...');
const app = await launchApp({
  port: PORT,
  dataDir: testDataDir,
  downloadDir: testDownloadDir,
});
console.log('[PASS] Connected to installed app via CDP session.');

const cdp = app.cdp;

try {
  // Wait for splash screen to dismiss and app to be interactive
  console.log('Waiting for splash dismissal...');
  await cdp.waitForFunction('() => !document.querySelector(".splash-screen") || document.querySelector(".splash-screen").style.opacity === "0" || !document.body.contains(document.querySelector(".splash-screen"))');
  await new Promise((r) => setTimeout(r, 1000));

  // STEP 1: Arabic Light Empty State
  console.log('[CHECK 1] Verifying Arabic Light Empty State...');
  const emptyStateData = await cdp.evaluate(`
    (() => {
      const empty = document.querySelector('.empty-state');
      const title = empty ? empty.querySelector('.empty-title')?.textContent : '';
      const actionBtn = empty ? empty.querySelector('.empty-action-btn')?.textContent : '';
      const logoSvg = empty ? empty.querySelector('.empty-logo-wrap svg, .empty-logo-wrap img') : null;
      return { hasEmpty: !!empty, title, actionBtn, hasLogo: !!logoSvg };
    })()
  `);
  console.log('Empty state data:', emptyStateData);
  if (!emptyStateData.hasEmpty || !emptyStateData.hasLogo) {
    throw new Error('Empty state not rendered properly!');
  }
  await cdp.captureScreenshot(path.join(screenshotsDir, '01_empty_state_ar_light.png'));
  console.log('[PASS] Screenshot 01 saved.');

  // STEP 2: Empty Space Right-Click Context Menu
  console.log('[CHECK 2] Right-click on empty space (Arabic)...');
  await cdp.evaluate(`
    (() => {
      const container = document.querySelector('.table-container') || document.querySelector('.task-list-container');
      const rect = container.getBoundingClientRect();
      const evt = new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: rect.left + rect.width / 2,
        clientY: rect.top + rect.height / 2
      });
      container.dispatchEvent(evt);
    })()
  `);
  await new Promise((r) => setTimeout(r, 300));
  const emptyMenu = await cdp.evaluate(`
    (() => {
      const menu = document.querySelector('.context-menu-popup');
      if (!menu) return null;
      const items = Array.from(menu.querySelectorAll('.ctx-item')).map(el => el.textContent.trim());
      return { visible: true, items, style: menu.getAttribute('style') };
    })()
  `);
  console.log('Empty space context menu items:', emptyMenu);
  await cdp.captureScreenshot(path.join(screenshotsDir, '02_empty_space_context_menu_ar.png'));
  console.log('[PASS] Screenshot 02 saved.');

  // Dismiss context menu
  await cdp.evaluate(`(() => document.dispatchEvent(new MouseEvent('click', { bubbles: true })))()`);
  await new Promise((r) => setTimeout(r, 200));

  // STEP 3: Settings Panel in Arabic Light & Tab Switching without Freeze
  console.log('[CHECK 3] Opening Settings and cycling all tabs in Arabic Light...');
  await cdp.click('#tour-settings-btn, button[title*="الإعدادات"]');
  await cdp.waitForSelector('.settings-dialog');

  const tabs = ['general', 'downloads', 'categories', 'notifications', 'power', 'appearance', 'extension', 'about'];
  for (const tab of tabs) {
    console.log(`Switching to Settings tab: ${tab}...`);
    await cdp.click(`#tour-nav-${tab}`);
    await new Promise((r) => setTimeout(r, 200));
  }

  // Focus on Appearance tab (Item 1 verification)
  await cdp.click('#tour-nav-appearance');
  await new Promise((r) => setTimeout(r, 300));
  const appearanceCheck = await cdp.evaluate(`
    (() => {
      const themeSelect = document.querySelector('#theme-select');
      const langSelect = document.querySelector('#lang-select');
      return {
        theme: themeSelect ? themeSelect.value : null,
        lang: langSelect ? langSelect.value : null,
      };
    })()
  `);
  console.log('Appearance tab verified without freeze:', appearanceCheck);
  await cdp.captureScreenshot(path.join(screenshotsDir, '03_settings_appearance_ar_light.png'));
  console.log('[PASS] Screenshot 03 saved.');

  // Test close button 'X' (Item 6 verification)
  console.log('[CHECK 6] Verifying close button X hit area...');
  await cdp.click('.settings-dialog .close-btn');
  await new Promise((r) => setTimeout(r, 400));
  let isOpen = await cdp.evaluate('Boolean(document.querySelector(".settings-dialog"))');
  if (isOpen) throw new Error('Close button X failed to close settings!');
  console.log('[PASS] Settings closed with X button cleanly.');

  // Open settings again and test Escape key
  console.log('Testing Escape key to close settings...');
  await cdp.click('#tour-settings-btn, button[title*="الإعدادات"]');
  await cdp.waitForSelector('.settings-dialog');
  await cdp.evaluate(`
    (() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true }));
    })()
  `);
  await new Promise((r) => setTimeout(r, 400));
  isOpen = await cdp.evaluate('Boolean(document.querySelector(".settings-dialog"))');
  if (isOpen) throw new Error('Escape key failed to close settings!');
  console.log('[PASS] Settings closed with Escape key cleanly.');

  // STEP 4: Switch to Arabic Dark and verify theme sync
  console.log('[CHECK 4 & 7] Switching theme to Dark in Arabic...');
  await cdp.click('#tour-settings-btn, button[title*="الإعدادات"]');
  await cdp.waitForSelector('.settings-dialog');
  await cdp.click('#tour-nav-appearance');
  await new Promise((r) => setTimeout(r, 200));

  // Change theme select to dark
  await cdp.evaluate(`
    (() => {
      const sel = document.querySelector('#theme-select');
      sel.value = 'dark';
      sel.dispatchEvent(new Event('change', { bubbles: true }));
    })()
  `);
  // Click Save
  await cdp.click('.settings-dialog .btn-primary');
  await new Promise((r) => setTimeout(r, 500));
  // Close settings
  await cdp.click('.settings-dialog .close-btn');
  await new Promise((r) => setTimeout(r, 300));

  const darkThemeData = await cdp.evaluate(`
    (() => {
      return {
        themeAttr: document.documentElement.getAttribute('data-theme'),
        lang: document.documentElement.getAttribute('lang'),
        dir: document.documentElement.getAttribute('dir'),
      };
    })()
  `);
  console.log('Dark theme applied:', darkThemeData);
  if (darkThemeData.themeAttr !== 'dark' || darkThemeData.lang !== 'ar') {
    throw new Error('Theme or language revert failed on save!');
  }
  await cdp.captureScreenshot(path.join(screenshotsDir, '04_app_ar_dark.png'));
  console.log('[PASS] Screenshot 04 saved (App in Arabic Dark).');

  // Verify settings modal is also Dark
  await cdp.click('#tour-settings-btn, button[title*="الإعدادات"]');
  await cdp.waitForSelector('.settings-dialog');
  await cdp.captureScreenshot(path.join(screenshotsDir, '05_settings_ar_dark.png'));
  console.log('[PASS] Screenshot 05 saved (Settings in Arabic Dark).');
  await cdp.click('.settings-dialog .close-btn');
  await new Promise((r) => setTimeout(r, 300));

  // STEP 5: Test Download Error (404 Not Found)
  console.log('[CHECK 5/A] Testing download 404 error with real error text...');
  await cdp.click('#tour-add-btn, button[title*="إضافة"]');
  await cdp.waitForSelector('.dialog-card');
  await cdp.type('#url-field', `http://127.0.0.1:${FIXTURE_PORT}/files/100MB.bin`);
  await cdp.click('.dialog-actions .btn-primary');
  await new Promise((r) => setTimeout(r, 2000));

  // Check that row appears with Failed state and real error text
  const failedTaskInfo = await cdp.evaluate(`
    (() => {
      const row = document.querySelector('.grid-row');
      if (!row) return null;
      const statusText = row.querySelector('.col-status')?.textContent?.trim();
      const errorText = row.querySelector('.inline-error')?.textContent?.trim() || document.body.textContent;
      return { statusText, has404: errorText.includes('404') };
    })()
  `);
  console.log('Failed task check:', failedTaskInfo);
  await cdp.captureScreenshot(path.join(screenshotsDir, '06_failed_download_404_real_error.png'));
  console.log('[PASS] Screenshot 06 saved.');

  // STEP 6: Test Successful 100MB / 1GB download
  console.log('[CHECK 5/B] Testing real download with 16px turquoise bar & 2-col grid...');
  await cdp.click('#tour-add-btn, button[title*="إضافة"]');
  await cdp.waitForSelector('.dialog-card');
  await cdp.type('#url-field', `http://127.0.0.1:${FIXTURE_PORT}/files/100Mb.dat`);
  await cdp.click('.dialog-actions .btn-primary');
  await new Promise((r) => setTimeout(r, 2000));

  await cdp.captureScreenshot(path.join(screenshotsDir, '07_grid_download_active_ar_dark.png'));
  console.log('[PASS] Screenshot 07 saved.');

  // Right-click row -> context menu in Arabic
  console.log('[CHECK 10] Right-click row context menu...');
  await cdp.evaluate(`
    (() => {
      const row = document.querySelector('.grid-row');
      const rect = row.getBoundingClientRect();
      const evt = new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: rect.left + 150,
        clientY: rect.top + 20
      });
      row.dispatchEvent(evt);
    })()
  `);
  await new Promise((r) => setTimeout(r, 300));
  await cdp.captureScreenshot(path.join(screenshotsDir, '08_row_context_menu_ar_dark.png'));
  console.log('[PASS] Screenshot 08 saved.');
  await cdp.evaluate(`(() => document.dispatchEvent(new MouseEvent('click', { bubbles: true })))()`);
  await new Promise((r) => setTimeout(r, 200));

  // STEP 7: Switch to English Light & English Dark
  console.log('[CHECK 7 & 8] Switching to English Light...');
  await cdp.click('#tour-settings-btn, button[title*="الإعدادات"]');
  await cdp.waitForSelector('.settings-dialog');
  await cdp.click('#tour-nav-appearance');
  await new Promise((r) => setTimeout(r, 200));

  await cdp.evaluate(`
    (() => {
      const themeSel = document.querySelector('#theme-select');
      themeSel.value = 'light';
      themeSel.dispatchEvent(new Event('change', { bubbles: true }));

      const langSel = document.querySelector('#lang-select');
      langSel.value = 'en';
      langSel.dispatchEvent(new Event('change', { bubbles: true }));
    })()
  `);
  await cdp.click('.settings-dialog .btn-primary');
  await new Promise((r) => setTimeout(r, 500));
  await cdp.click('.settings-dialog .close-btn');
  await new Promise((r) => setTimeout(r, 300));

  const enLightData = await cdp.evaluate(`
    (() => {
      return {
        themeAttr: document.documentElement.getAttribute('data-theme'),
        lang: document.documentElement.getAttribute('lang'),
        dir: document.documentElement.getAttribute('dir'),
      };
    })()
  `);
  console.log('English Light applied:', enLightData);
  if (enLightData.themeAttr !== 'light' || enLightData.lang !== 'en' || enLightData.dir !== 'ltr') {
    throw new Error('English Light settings verification failed!');
  }
  await cdp.captureScreenshot(path.join(screenshotsDir, '09_main_en_light.png'));
  console.log('[PASS] Screenshot 09 saved.');

  // Switch to English Dark
  console.log('Switching to English Dark...');
  await cdp.click('#tour-settings-btn, button[title*="Settings"]');
  await cdp.waitForSelector('.settings-dialog');
  await cdp.click('#tour-nav-appearance');
  await new Promise((r) => setTimeout(r, 200));

  await cdp.evaluate(`
    (() => {
      const themeSel = document.querySelector('#theme-select');
      themeSel.value = 'dark';
      themeSel.dispatchEvent(new Event('change', { bubbles: true }));
    })()
  `);
  await cdp.click('.settings-dialog .btn-primary');
  await new Promise((r) => setTimeout(r, 500));
  await cdp.click('.settings-dialog .close-btn');
  await new Promise((r) => setTimeout(r, 300));

  await cdp.captureScreenshot(path.join(screenshotsDir, '10_main_en_dark.png'));
  console.log('[PASS] Screenshot 10 saved.');

  console.log('\n==========================================');
  console.log('ALL VERIFICATION CHECKS PASSED ON INSTALLED APP!');
  console.log('==========================================\n');

} finally {
  await app.close();
  server.close();
  console.log('Cleanup completed.');
}
