// Extract the Ahem woff2 font from taffy's CSS and write it as a file
const fs = require('fs');
const css = fs.readFileSync('/home/folk/Programs/taffy/scripts/gentest/test_base_style.css', 'utf8');
const m = css.match(/base64,([A-Za-z0-9+/=]+)\)/);
if (!m) { console.error('Could not find base64 font data'); process.exit(1); }
const buf = Buffer.from(m[1], 'base64');
fs.writeFileSync('/home/folk/Programs/litehtml-rs/compare/ahem.woff2', buf);
console.log(`Wrote ahem.woff2 (${buf.length} bytes)`);
