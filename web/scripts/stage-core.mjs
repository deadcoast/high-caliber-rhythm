// Copy the wasm core from the cargo target dir into public/ for Vite.
import fs from 'node:fs';
import path from 'node:path';
const src = path.resolve('../core/target/wasm32-unknown-unknown/release/downbeat_core.wasm');
const dst = path.resolve('public/downbeat_core.wasm');
fs.copyFileSync(src, dst);
console.log(`staged ${dst} (${(fs.statSync(dst).size / 1024).toFixed(0)} KB)`);
