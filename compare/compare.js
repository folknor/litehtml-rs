// Compare Chrome and pipeline layout extractions.
// Matches elements by DOM path, reports geometry and style differences.
//
// Usage: node compare/compare.js test-emails/foo_chrome.json test-emails/foo_pipeline.json
//
// Exit code 0 = pass (all matched elements within tolerance)
// Exit code 1 = fail (differences found)

const fs = require('fs');

const chromePath = process.argv[2];
const pipelinePath = process.argv[3];
if (!chromePath || !pipelinePath) {
  console.error('Usage: node compare/compare.js <chrome.json> <pipeline.json>');
  process.exit(2);
}

const chrome = JSON.parse(fs.readFileSync(chromePath, 'utf8'));
const pipeline = JSON.parse(fs.readFileSync(pipelinePath, 'utf8'));

// Index by path
const chromeByPath = new Map();
for (const el of chrome) chromeByPath.set(el.path, el);
const pipelineByPath = new Map();
for (const el of pipeline) pipelineByPath.set(el.path, el);

// --- Tolerances ---
const TOL = {
  pos: 2,       // px: x, y position
  size: 5,      // px: width, height
  padding: 2,   // px: padding
  margin: 2,    // px: margin
  fontSize: 1,  // px: font-size
  color: 5,     // per-channel RGB
};

// --- Helpers ---
function isTransparent(str) {
  if (!str || str === 'null') return true;
  const m = str.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?/);
  if (!m) return true;
  if (m[4] !== undefined && parseFloat(m[4]) === 0) return true;
  return false;
}

function parseRgb(str) {
  if (!str || str === 'null') return null;
  const m = str.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
  return m ? [+m[1], +m[2], +m[3]] : null;
}

function colorsMatch(a, b) {
  if (isTransparent(a) && isTransparent(b)) return true;
  if (isTransparent(a) || isTransparent(b)) return false;
  const ca = parseRgb(a), cb = parseRgb(b);
  if (!ca || !cb) return false;
  return Math.abs(ca[0]-cb[0]) <= TOL.color && Math.abs(ca[1]-cb[1]) <= TOL.color && Math.abs(ca[2]-cb[2]) <= TOL.color;
}

function numDiff(a, b, tol) {
  if (a == null && b == null) return null;
  if (a == null || b == null) return { chrome: a, pipeline: b, delta: null };
  const d = Math.abs(a - b);
  return d > tol ? { chrome: a, pipeline: b, delta: Math.round(d * 10) / 10 } : null;
}

// --- Match and compare ---
const allPaths = new Set([...chromeByPath.keys(), ...pipelineByPath.keys()]);
const matched = [];
const chromeOnly = [];
const pipelineOnly = [];

for (const p of allPaths) {
  const inC = chromeByPath.has(p);
  const inP = pipelineByPath.has(p);
  if (inC && inP) matched.push(p);
  else if (inC) chromeOnly.push(p);
  else pipelineOnly.push(p);
}

// Filter pipeline-only to skip <head> and its children (Chrome hides display:none)
const significantPipelineOnly = pipelineOnly.filter(p => !p.includes('head['));

const diffs = [];
let exactCount = 0;

for (const path of matched) {
  const c = chromeByPath.get(path);
  const p = pipelineByPath.get(path);
  const issues = {};

  // Geometry
  const dx = numDiff(c.x, p.x, TOL.pos);
  const dy = numDiff(c.y, p.y, TOL.pos);
  const dw = numDiff(c.w, p.w, TOL.size);
  const dh = numDiff(c.h, p.h, TOL.size);
  if (dx) issues.x = dx;
  if (dy) issues.y = dy;
  if (dw) issues.w = dw;
  if (dh) issues.h = dh;

  // Colors
  if (!colorsMatch(c.bg, p.bg)) issues.bg = { chrome: c.bg, pipeline: p.bg };
  if (!colorsMatch(c.color, p.color)) issues.color = { chrome: c.color, pipeline: p.color };

  // Font
  const dfs = numDiff(c.fontSize, p.fontSize, TOL.fontSize);
  if (dfs) issues.fontSize = dfs;

  // Padding
  const dpt = numDiff(c.paddingTop, p.paddingTop, TOL.padding);
  const dpr = numDiff(c.paddingRight, p.paddingRight, TOL.padding);
  const dpb = numDiff(c.paddingBottom, p.paddingBottom, TOL.padding);
  const dpl = numDiff(c.paddingLeft, p.paddingLeft, TOL.padding);
  if (dpt || dpr || dpb || dpl) {
    issues.padding = {};
    if (dpt) issues.padding.top = dpt;
    if (dpr) issues.padding.right = dpr;
    if (dpb) issues.padding.bottom = dpb;
    if (dpl) issues.padding.left = dpl;
  }

  if (Object.keys(issues).length === 0) {
    exactCount++;
  } else {
    diffs.push({ path, tag: c.tag, classes: c.classes, issues });
  }
}

// --- Report ---
const total = matched.length;
const passRate = total > 0 ? Math.round(exactCount / total * 100) : 0;

console.log(`\n╔══════════════════════════════════════╗`);
console.log(`║        LAYOUT COMPARISON REPORT      ║`);
console.log(`╚══════════════════════════════════════╝`);
console.log(`  Chrome elements:     ${chrome.length}`);
console.log(`  Pipeline elements:   ${pipeline.length}`);
console.log(`  Matched by path:     ${total}`);
console.log(`  Exact matches:       ${exactCount} / ${total} (${passRate}%)`);
console.log(`  With differences:    ${diffs.length}`);
if (significantPipelineOnly.length > 0) {
  console.log(`  Pipeline-only:       ${significantPipelineOnly.length}`);
}

// Categorize diffs
const categories = {};
for (const d of diffs) {
  for (const key of Object.keys(d.issues)) {
    categories[key] = (categories[key] || 0) + 1;
  }
}

if (Object.keys(categories).length > 0) {
  console.log(`\n── Diff Categories ──`);
  for (const [cat, count] of Object.entries(categories).sort((a, b) => b[1] - a[1])) {
    const pct = Math.round(count / total * 100);
    const bar = '█'.repeat(Math.ceil(pct / 3));
    console.log(`  ${cat.padEnd(12)} ${String(count).padStart(4)} (${String(pct).padStart(2)}%) ${bar}`);
  }
}

// Largest geometry diffs
const geoDiffs = diffs
  .map(d => {
    let maxDelta = 0;
    for (const key of ['w', 'h', 'x', 'y']) {
      if (d.issues[key] && d.issues[key].delta) {
        maxDelta = Math.max(maxDelta, d.issues[key].delta);
      }
    }
    return { ...d, maxDelta };
  })
  .filter(d => d.maxDelta > 0)
  .sort((a, b) => b.maxDelta - a.maxDelta);

if (geoDiffs.length > 0) {
  console.log(`\n── Top Geometry Diffs (chrome → pipeline) ──`);
  const shown = geoDiffs.slice(0, 30);
  for (const d of shown) {
    const label = d.classes
      ? `<${d.tag}.${d.classes.split(' ')[0]}>`
      : `<${d.tag}>`;
    const shortPath = d.path.split('>').slice(-3).join(' > ');
    const parts = [];
    for (const key of ['x', 'y', 'w', 'h']) {
      if (d.issues[key]) {
        parts.push(`${key}: ${d.issues[key].chrome} → ${d.issues[key].pipeline} (Δ${d.issues[key].delta})`);
      }
    }
    console.log(`  ${shortPath}`);
    console.log(`    ${label} ${parts.join(', ')}`);
  }
}

// Color diffs
const colorDiffs = diffs.filter(d => d.issues.color || d.issues.bg);
if (colorDiffs.length > 0) {
  console.log(`\n── Color Diffs ──`);
  for (const d of colorDiffs.slice(0, 15)) {
    const shortPath = d.path.split('>').slice(-3).join(' > ');
    if (d.issues.color) {
      console.log(`  ${shortPath}: color ${d.issues.color.chrome} → ${d.issues.color.pipeline}`);
    }
    if (d.issues.bg) {
      console.log(`  ${shortPath}: bg ${d.issues.bg.chrome} → ${d.issues.bg.pipeline}`);
    }
  }
}

// Padding diffs
const padDiffs = diffs.filter(d => d.issues.padding);
if (padDiffs.length > 0) {
  console.log(`\n── Padding Diffs ──`);
  for (const d of padDiffs.slice(0, 15)) {
    const shortPath = d.path.split('>').slice(-3).join(' > ');
    const parts = [];
    for (const side of ['top', 'right', 'bottom', 'left']) {
      const pd = d.issues.padding[side];
      if (pd) parts.push(`${side}: ${pd.chrome} → ${pd.pipeline}`);
    }
    console.log(`  ${shortPath} [${d.classes || d.tag}]: ${parts.join(', ')}`);
  }
}

// Exit code
const pass = diffs.length === 0;
if (pass) {
  console.log(`\n✓ ALL ELEMENTS MATCH within tolerance.`);
} else {
  console.log(`\n✗ ${diffs.length} elements differ.`);
}
process.exit(pass ? 0 : 1);
