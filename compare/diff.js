// Compare Chrome and pipeline layout dumps by DOM path.
// Usage: node compare/diff.js test-emails/foo_chrome.json test-emails/foo_pipeline.json
const fs = require('fs');

const chromePath = process.argv[2];
const pipelinePath = process.argv[3];
if (!chromePath || !pipelinePath) {
  console.error('Usage: node compare/diff.js <chrome.json> <pipeline.json>');
  process.exit(1);
}

const chrome = JSON.parse(fs.readFileSync(chromePath, 'utf8'));
const pipeline = JSON.parse(fs.readFileSync(pipelinePath, 'utf8'));

// Index by path
const chromeByPath = {};
for (const el of chrome) chromeByPath[el.path] = el;
const pipelineByPath = {};
for (const el of pipeline) pipelineByPath[el.path] = el;

const TOLERANCE = {
  position: 5,
  size: 10,
  fontSize: 2,
  padding: 3,
};

function isTransparent(str) {
  if (!str || str === 'null') return true;
  const m = str.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*(\d+(?:\.\d+)?))?/);
  if (!m) return true;
  if (m[4] !== undefined && parseFloat(m[4]) === 0) return true;
  return false;
}

function parseRgba(str) {
  if (!str || str === 'null') return null;
  const m = str.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
  if (m) return [parseInt(m[1]), parseInt(m[2]), parseInt(m[3])];
  return null;
}

function colorsMatch(a, b) {
  const ta = isTransparent(a);
  const tb = isTransparent(b);
  if (ta && tb) return true;
  if (ta || tb) return false;
  const ca = parseRgba(a);
  const cb = parseRgba(b);
  if (!ca || !cb) return false;
  return Math.abs(ca[0] - cb[0]) <= 5 &&
         Math.abs(ca[1] - cb[1]) <= 5 &&
         Math.abs(ca[2] - cb[2]) <= 5;
}

// Find all paths present in both
const allPaths = new Set([...Object.keys(chromeByPath), ...Object.keys(pipelineByPath)]);
const matched = [];
const chromeOnly = [];
const pipelineOnly = [];

for (const p of allPaths) {
  if (chromeByPath[p] && pipelineByPath[p]) {
    matched.push(p);
  } else if (chromeByPath[p]) {
    chromeOnly.push(p);
  } else {
    pipelineOnly.push(p);
  }
}

// Compare matched elements
const issues = [];
let exact = 0;

for (const p of matched) {
  const c = chromeByPath[p];
  const pp = pipelineByPath[p];
  const diffs = [];

  if (Math.abs(c.x - pp.x) > TOLERANCE.position) diffs.push(`x: ${c.x} → ${pp.x}`);
  if (Math.abs(c.y - pp.y) > TOLERANCE.position) diffs.push(`y: ${c.y} → ${pp.y}`);
  if (Math.abs(c.w - pp.w) > TOLERANCE.size) diffs.push(`w: ${c.w} → ${pp.w}`);
  if (Math.abs(c.h - pp.h) > TOLERANCE.size) diffs.push(`h: ${c.h} → ${pp.h}`);
  if (!colorsMatch(c.bg, pp.bg)) diffs.push(`bg: ${c.bg} → ${pp.bg}`);
  if (!colorsMatch(c.color, pp.color)) diffs.push(`color: ${c.color} → ${pp.color}`);
  if (Math.abs(c.fontSize - pp.fontSize) > TOLERANCE.fontSize) diffs.push(`fontSize: ${c.fontSize} → ${pp.fontSize}`);
  if (Math.abs(c.paddingTop - pp.paddingTop) > TOLERANCE.padding) diffs.push(`padTop: ${c.paddingTop} → ${pp.paddingTop}`);
  if (Math.abs(c.paddingLeft - pp.paddingLeft) > TOLERANCE.padding) diffs.push(`padLeft: ${c.paddingLeft} → ${pp.paddingLeft}`);
  if (Math.abs(c.paddingRight - pp.paddingRight) > TOLERANCE.padding) diffs.push(`padRight: ${c.paddingRight} → ${pp.paddingRight}`);
  if (Math.abs(c.paddingBottom - pp.paddingBottom) > TOLERANCE.padding) diffs.push(`padBot: ${c.paddingBottom} → ${pp.paddingBottom}`);

  if (diffs.length > 0) {
    issues.push({ path: p, tag: c.tag, diffs });
  } else {
    exact++;
  }
}

// Output
console.log(`\n=== Layout Comparison ===`);
console.log(`Chrome: ${chrome.length} elements`);
console.log(`Pipeline: ${pipeline.length} elements`);
console.log(`Matched by path: ${matched.length}`);
console.log(`Exact matches: ${exact} / ${matched.length} (${matched.length > 0 ? Math.round(exact/matched.length*100) : 0}%)`);
console.log(`Chrome-only paths: ${chromeOnly.length}`);
console.log(`Pipeline-only paths: ${pipelineOnly.length}`);

if (chromeOnly.length > 0 && chromeOnly.length <= 20) {
  console.log(`\n--- Chrome-only ---`);
  for (const p of chromeOnly) console.log(`  ${p}`);
}
if (pipelineOnly.length > 0 && pipelineOnly.length <= 20) {
  console.log(`\n--- Pipeline-only ---`);
  for (const p of pipelineOnly) console.log(`  ${p}`);
}

// Diff categories
const diffCounts = {};
for (const d of issues) {
  for (const diff of d.diffs) {
    const key = diff.split(':')[0];
    diffCounts[key] = (diffCounts[key] || 0) + 1;
  }
}

if (Object.keys(diffCounts).length > 0) {
  console.log(`\n--- Diff Categories ---`);
  for (const [key, count] of Object.entries(diffCounts).sort((a, b) => b[1] - a[1])) {
    console.log(`  ${key}: ${count} elements`);
  }
}

// Largest diffs by size delta
if (issues.length > 0) {
  const sizeDiffs = issues
    .map(d => {
      let maxDelta = 0;
      for (const diff of d.diffs) {
        const m = diff.match(/^[wh]: ([\d.]+) → ([\d.]+)$/);
        if (m) maxDelta = Math.max(maxDelta, Math.abs(parseFloat(m[1]) - parseFloat(m[2])));
      }
      return { ...d, maxDelta };
    })
    .sort((a, b) => b.maxDelta - a.maxDelta)
    .slice(0, 25);

  console.log(`\n--- Top 25 Diffs (by size delta) ---`);
  for (const d of sizeDiffs) {
    // Shorten path for display: just show last 3 segments
    const shortPath = d.path.split('>').slice(-3).join('>');
    console.log(`  ${shortPath}: ${d.diffs.join(', ')}`);
  }
}
