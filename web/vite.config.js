import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// The wasm core is served from public/downbeat_core.wasm (npm run core).
export default defineConfig({
  plugins: [
    svelte({
      onwarn(warning, handler) {
        // Internal tool: the a11y pass is noise for canvas/SVG drag handles.
        if (warning.code.startsWith('a11y')) return;
        handler(warning);
      },
    }),
  ],
  base: './',
  build: { target: 'es2022' },
});
