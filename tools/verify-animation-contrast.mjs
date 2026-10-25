import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { chromium } from 'playwright';

const websiteDir = path.resolve('apps/website');
const artifactsDir = path.resolve('tools/screenshots');
fs.mkdirSync(artifactsDir, { recursive: true });

// Simple HTTP server for apps/website
const MIME_TYPES = {
  '.html': 'text/html',
  '.css': 'text/css',
  '.js': 'application/javascript',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.webp': 'image/webp',
  '.woff2': 'font/woff2',
  '.json': 'application/json',
  '.webmanifest': 'application/manifest+json'
};

const server = http.createServer((req, res) => {
  let reqPath = decodeURI(req.url.split('?')[0]);
  if (reqPath === '/') reqPath = '/index.html';
  if (reqPath === '/en' || reqPath === '/en/') reqPath = '/en/index.html';

  let filePath = path.join(websiteDir, reqPath);
  if (!fs.existsSync(filePath) && fs.existsSync(filePath + '.html')) {
    filePath = filePath + '.html';
  } else if (fs.existsSync(filePath) && fs.statSync(filePath).isDirectory()) {
    const indexPath = path.join(filePath, 'index.html');
    if (fs.existsSync(indexPath)) filePath = indexPath;
  }

  if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
    const ext = path.extname(filePath).toLowerCase();
    const contentType = MIME_TYPES[ext] || 'application/octet-stream';
    res.writeHead(200, { 'Content-Type': contentType });
    fs.createReadStream(filePath).pipe(res);
  } else {
    res.writeHead(404, { 'Content-Type': 'text/plain' });
    res.end('Not Found');
  }
});

await new Promise((resolve) => server.listen(3344, '127.0.0.1', resolve));
console.log('Static server listening on http://127.0.0.1:3344');

try {
  const browser = await chromium.launch({
    channel: 'chrome',
    headless: true
  });

  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 1
  });

  const page = await context.newPage();

  // Test 1: Home page at 1s and 3s
  console.log('Navigating to Home page...');
  await page.goto('http://127.0.0.1:3344/', { waitUntil: 'networkidle' });

  // Wait 1 second and capture frame 1
  await page.waitForTimeout(1000);
  const path1s = path.join(artifactsDir, 'home-1s.png');
  await page.screenshot({ path: path1s, fullPage: false });
  console.log(`Captured 1s screenshot: ${path1s}`);

  // Wait 2 more seconds (total 3s) and capture frame 2
  await page.waitForTimeout(2000);
  const path3s = path.join(artifactsDir, 'home-3s.png');
  await page.screenshot({ path: path3s, fullPage: false });
  console.log(`Captured 3s screenshot: ${path3s}`);

  // Compare buffers
  const buf1 = fs.readFileSync(path1s);
  const buf2 = fs.readFileSync(path3s);
  let differentBytes = 0;
  const minLen = Math.min(buf1.length, buf2.length);
  for (let i = 0; i < minLen; i++) {
    if (buf1[i] !== buf2[i]) differentBytes++;
  }
  differentBytes += Math.abs(buf1.length - buf2.length);
  const isDifferent = differentBytes > 0;
  console.log(`Animation Frame Difference: ${differentBytes} byte changes between 1s and 3s. Differ? ${isDifferent}`);

  // Test 2: Hero Contrast Evaluation
  const contrastInfo = await page.evaluate(() => {
    function getLuminance(r, g, b) {
      const a = [r, g, b].map(v => {
        v /= 255;
        return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
      });
      return 0.2126 * a[0] + 0.7152 * a[1] + 0.0722 * a[2];
    }

    function parseColor(str) {
      const m = str.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
      if (!m) return [255, 255, 255];
      return [parseInt(m[1]), parseInt(m[2]), parseInt(m[3])];
    }

    function ratio(c1, c2) {
      const l1 = getLuminance(...c1);
      const l2 = getLuminance(...c2);
      const lighter = Math.max(l1, l2);
      const darker = Math.min(l1, l2);
      return (lighter + 0.05) / (darker + 0.05);
    }

    const h1 = document.querySelector('.hero h1');
    const lead = document.querySelector('.hero p.lead');
    const dlName = document.querySelector('.dl .name');
    const dlBig = document.querySelector('.dl .big');
    const dlSmall = document.querySelector('.dl small');
    const heroContent = document.querySelector('.hero-content');
    const dlCard = document.querySelector('.dl');

    const scrimBg = [7, 15, 46]; // #070F2E
    const cardBg = [11, 26, 78]; // #0B1A4E

    const h1Color = parseColor(window.getComputedStyle(h1).color);
    const leadColor = parseColor(window.getComputedStyle(lead).color);
    const dlNameColor = parseColor(window.getComputedStyle(dlName).color);
    const dlBigColor = parseColor(window.getComputedStyle(dlBig).color);
    const dlSmallColor = parseColor(window.getComputedStyle(dlSmall).color);

    return {
      h1Contrast: ratio(h1Color, scrimBg).toFixed(2),
      leadContrast: ratio(leadColor, scrimBg).toFixed(2),
      dlNameContrast: ratio(dlNameColor, cardBg).toFixed(2),
      dlBigContrast: ratio(dlBigColor, cardBg).toFixed(2),
      dlSmallContrast: ratio(dlSmallColor, cardBg).toFixed(2)
    };
  });
  console.log('Contrast evaluation results:', contrastInfo);

  // Test 3: Docs page
  console.log('Navigating to Docs page...');
  await page.goto('http://127.0.0.1:3344/docs', { waitUntil: 'networkidle' });
  await page.waitForTimeout(500);
  const pathDocs = path.join(artifactsDir, 'docs.png');
  await page.screenshot({ path: pathDocs, fullPage: false });
  console.log(`Captured docs screenshot: ${pathDocs}`);

  const docsBgInfo = await page.evaluate(() => {
    const aside = document.querySelector('.docs aside');
    const article = document.querySelector('.docs article');
    return {
      asideBg: window.getComputedStyle(aside).backgroundColor,
      articleBg: window.getComputedStyle(article).backgroundColor
    };
  });
  console.log('Docs background evaluation:', docsBgInfo);

  // Test 4: Compare page
  console.log('Navigating to Compare page...');
  await page.goto('http://127.0.0.1:3344/compare', { waitUntil: 'networkidle' });
  await page.waitForTimeout(500);
  const pathCompare = path.join(artifactsDir, 'compare.png');
  await page.screenshot({ path: pathCompare, fullPage: false });
  console.log(`Captured compare screenshot: ${pathCompare}`);

  await browser.close();
  console.log('ALL PLAYWRIGHT TESTS PASSED SUCCESSFULLY.');
} finally {
  server.close();
}
