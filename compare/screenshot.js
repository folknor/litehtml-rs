// Usage: node compare/screenshot.js test-emails/foo.html
// Produces: test-emails/foo_chrome.png (same naming convention as pipeline)
const puppeteer = require('/home/folk/.npm-global/lib/node_modules/puppeteer');
const path = require('path');
const fs = require('fs');

const htmlPath = process.argv[2];
if (!htmlPath) {
  console.error('Usage: node compare/screenshot.js <html-file>');
  process.exit(1);
}

const absPath = path.resolve(htmlPath);
if (!fs.existsSync(absPath)) {
  console.error(`File not found: ${absPath}`);
  process.exit(1);
}

const outPath = absPath.replace('.html', '_chrome.png');

(async () => {
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });
  const page = await browser.newPage();
  await page.setViewport({ width: 800, height: 600 });
  await page.goto('file://' + absPath, { waitUntil: 'networkidle0', timeout: 10000 });

  // Get full page height
  const bodyHeight = await page.evaluate(() => document.body.scrollHeight);
  // Cap at 10000 to match pipeline
  const height = Math.min(bodyHeight, 10000);

  await page.setViewport({ width: 800, height });
  await page.screenshot({ path: outPath, fullPage: true });
  console.log(`Saved ${outPath} (${height}px tall)`);

  await browser.close();
})();
