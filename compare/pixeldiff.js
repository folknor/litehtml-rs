// Pixel-level diff between Chrome and pipeline PNGs.
// Outputs a diff image highlighting differences and a summary.
//
// Usage: node compare/pixeldiff.js fixtures/foo_chrome.png fixtures/foo_pipeline.png
//
// Produces: fixtures/foo_diff.png (red = different pixels)

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const chromePath = process.argv[2];
const pipelinePath = process.argv[3];
if (!chromePath || !pipelinePath) {
  console.error('Usage: node compare/pixeldiff.js <chrome.png> <pipeline.png>');
  process.exit(2);
}

// Use Node's built-in image decoding if available, otherwise fall back to external tool
// We'll use the 'sharp' module if available, otherwise ImageMagick compare

try {
  // Try ImageMagick compare
  const diffPath = chromePath.replace('_chrome.png', '_diff.png');

  // Get image dimensions
  const chromeInfo = execSync(`identify -format "%wx%h" "${chromePath}"`, { encoding: 'utf8' }).trim();
  const pipelineInfo = execSync(`identify -format "%wx%h" "${pipelinePath}"`, { encoding: 'utf8' }).trim();
  console.log(`Chrome:   ${chromeInfo}`);
  console.log(`Pipeline: ${pipelineInfo}`);

  const [cw, ch] = chromeInfo.split('x').map(Number);
  const [pw, ph] = pipelineInfo.split('x').map(Number);

  // Pad shorter image to match heights for comparison
  const maxH = Math.max(ch, ph);
  const maxW = Math.max(cw, pw);

  // Create padded versions if sizes differ
  let chromeCompare = chromePath;
  let pipelineCompare = pipelinePath;

  if (cw !== pw || ch !== ph) {
    chromeCompare = chromePath.replace('.png', '_padded.png');
    pipelineCompare = pipelinePath.replace('.png', '_padded.png');
    execSync(`convert "${chromePath}" -background white -extent ${maxW}x${maxH} "${chromeCompare}"`);
    execSync(`convert "${pipelinePath}" -background white -extent ${maxW}x${maxH} "${pipelineCompare}"`);
  }

  // Run ImageMagick compare
  // AE = absolute error count (number of different pixels)
  try {
    const result = execSync(
      `compare -metric AE -fuzz 5% "${chromeCompare}" "${pipelineCompare}" "${diffPath}" 2>&1`,
      { encoding: 'utf8' }
    );
    const diffPixels = parseInt(result.trim());
    const totalPixels = maxW * maxH;
    const pct = ((diffPixels / totalPixels) * 100).toFixed(1);
    console.log(`Diff pixels: ${diffPixels} / ${totalPixels} (${pct}%)`);
    console.log(`Diff image:  ${diffPath}`);
  } catch (e) {
    // compare returns exit code 1 when images differ, but still produces output
    if (e.stdout) {
      const diffPixels = parseInt(e.stdout.trim()) || parseInt(e.stderr?.trim()) || 'unknown';
      const totalPixels = maxW * maxH;
      if (typeof diffPixels === 'number') {
        const pct = ((diffPixels / totalPixels) * 100).toFixed(1);
        console.log(`Diff pixels: ${diffPixels} / ${totalPixels} (${pct}%)`);
      }
    }
    console.log(`Diff image:  ${diffPath}`);
  }

  // Clean up padded files
  if (chromeCompare !== chromePath) {
    try { fs.unlinkSync(chromeCompare); } catch {}
    try { fs.unlinkSync(pipelineCompare); } catch {}
  }

} catch (e) {
  console.error('ImageMagick not available. Install with: sudo apt install imagemagick');
  console.error(e.message);
  process.exit(1);
}
