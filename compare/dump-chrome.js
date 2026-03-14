// Dump per-element bounding rects and computed styles from Chrome.
// Usage: node compare/dump-chrome.js test-emails/foo.html
// Produces: test-emails/foo_chrome.json
const puppeteer = require('/home/folk/.npm-global/lib/node_modules/puppeteer');
const path = require('path');
const fs = require('fs');

const htmlPath = process.argv[2];
if (!htmlPath) {
  console.error('Usage: node compare/dump-chrome.js <html-file>');
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
    // Track sibling index per parent for path generation
    function walk(node, parentPath) {
      if (node.nodeType === Node.ELEMENT_NODE) {
        const cs = window.getComputedStyle(node);
        const tag = node.tagName.toLowerCase();

        // Skip invisible elements
        if (cs.display === 'none') {
          return;
        }

        // Compute sibling index: count preceding siblings with same tag
        let sibIdx = 0;
        let sib = node.previousElementSibling;
        while (sib) {
          if (sib.tagName === node.tagName) sibIdx++;
          sib = sib.previousElementSibling;
        }

        const nodePath = parentPath ? `${parentPath}>${tag}[${sibIdx}]` : tag;

        const rect = node.getBoundingClientRect();
        results.push({
          path: nodePath,
          tag,
          id: node.id || undefined,
          classes: node.className || undefined,
          x: Math.round(rect.x * 10) / 10,
          y: Math.round(rect.y * 10) / 10,
          w: Math.round(rect.width * 10) / 10,
          h: Math.round(rect.height * 10) / 10,
          display: cs.display,
          bg: cs.backgroundColor,
          color: cs.color,
          fontSize: parseFloat(cs.fontSize),
          fontWeight: cs.fontWeight,
          paddingTop: parseFloat(cs.paddingTop),
          paddingRight: parseFloat(cs.paddingRight),
          paddingBottom: parseFloat(cs.paddingBottom),
          paddingLeft: parseFloat(cs.paddingLeft),
          marginTop: parseFloat(cs.marginTop),
          marginRight: parseFloat(cs.marginRight),
          marginBottom: parseFloat(cs.marginBottom),
          marginLeft: parseFloat(cs.marginLeft),
          textAlign: cs.textAlign,
          maxWidth: cs.maxWidth === 'none' ? undefined : parseFloat(cs.maxWidth),
        });

        for (const child of node.children) {
          walk(child, nodePath);
        }
      }
    }
    walk(document.documentElement, '');
    return results;
  });

  const outPath = absPath.replace('.html', '_chrome.json');
  fs.writeFileSync(outPath, JSON.stringify(elements, null, 2));
  console.error(`Dumped ${elements.length} elements to ${outPath}`);

  await browser.close();
})();
