// Extract per-element layout geometry from Chrome using getBoundingClientRect().
// Walks the DOM depth-first, outputs JSON with path, tag, bounding rect, and key computed styles.
// Usage: node compare/extract-chrome.js test-emails/foo.html
// Produces: test-emails/foo_chrome.json
const puppeteer = require('/home/folk/.npm-global/lib/node_modules/puppeteer');
const path = require('path');
const fs = require('fs');

const htmlPath = process.argv[2];
if (!htmlPath) {
  console.error('Usage: node compare/extract-chrome.js <html-file>');
  process.exit(1);
}
const absPath = path.resolve(htmlPath);

(async () => {
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });
  const page = await browser.newPage();
  await page.setViewport({ width: 800, height: 600 });
  await page.goto('file://' + absPath, { waitUntil: 'networkidle0', timeout: 10000 });

  const elements = await page.evaluate(() => {
    const results = [];

    function walk(node, parentPath) {
      if (node.nodeType !== Node.ELEMENT_NODE) return;

      const cs = window.getComputedStyle(node);
      const tag = node.tagName.toLowerCase();

      if (cs.display === 'none') return;

      // Sibling index among same-tag siblings
      let sibIdx = 0;
      let sib = node.previousElementSibling;
      while (sib) {
        if (sib.tagName === node.tagName) sibIdx++;
        sib = sib.previousElementSibling;
      }
      const nodePath = parentPath ? `${parentPath}>${tag}[${sibIdx}]` : tag;

      // Use getBoundingClientRect for sub-pixel precision
      const rect = node.getBoundingClientRect();

      results.push({
        path: nodePath,
        tag,
        id: node.id || undefined,
        classes: node.className ? String(node.className).trim() : undefined,
        // Layout geometry (0.1px precision)
        x: Math.round(rect.left * 10) / 10,
        y: Math.round(rect.top * 10) / 10,
        w: Math.round(rect.width * 10) / 10,
        h: Math.round(rect.height * 10) / 10,
        // Computed styles
        display: cs.display,
        bg: cs.backgroundColor,
        color: cs.color,
        fontSize: Math.round(parseFloat(cs.fontSize) * 10) / 10,
        fontWeight: cs.fontWeight,
        lineHeight: cs.lineHeight === 'normal' ? null : Math.round(parseFloat(cs.lineHeight) * 10) / 10,
        paddingTop: Math.round(parseFloat(cs.paddingTop) * 10) / 10,
        paddingRight: Math.round(parseFloat(cs.paddingRight) * 10) / 10,
        paddingBottom: Math.round(parseFloat(cs.paddingBottom) * 10) / 10,
        paddingLeft: Math.round(parseFloat(cs.paddingLeft) * 10) / 10,
        marginTop: Math.round(parseFloat(cs.marginTop) * 10) / 10,
        marginRight: Math.round(parseFloat(cs.marginRight) * 10) / 10,
        marginBottom: Math.round(parseFloat(cs.marginBottom) * 10) / 10,
        marginLeft: Math.round(parseFloat(cs.marginLeft) * 10) / 10,
        textAlign: cs.textAlign,
        maxWidth: cs.maxWidth === 'none' ? null : Math.round(parseFloat(cs.maxWidth) * 10) / 10,
        // Overflow info
        overflowX: cs.overflowX,
        overflowY: cs.overflowY,
        // Content size for scroll containers
        scrollWidth: node.scrollWidth !== node.clientWidth ? node.scrollWidth : undefined,
        scrollHeight: node.scrollHeight !== node.clientHeight ? node.scrollHeight : undefined,
      });

      for (const child of node.children) {
        walk(child, nodePath);
      }
    }

    walk(document.documentElement, '');
    return results;
  });

  const outPath = absPath.replace('.html', '_chrome.json');
  fs.writeFileSync(outPath, JSON.stringify(elements, null, 2));
  console.error(`Extracted ${elements.length} elements to ${outPath}`);

  // Also take a screenshot for visual reference
  const bodyHeight = await page.evaluate(() => document.body.scrollHeight);
  const height = Math.min(bodyHeight, 10000);
  await page.setViewport({ width: 800, height });
  const screenshotPath = absPath.replace('.html', '_chrome.png');
  await page.screenshot({ path: screenshotPath, fullPage: true });
  console.error(`Screenshot: ${screenshotPath} (${height}px)`);

  await browser.close();
})();
