import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Assets are bundled locally; the prototype makes no network requests at runtime.
export default defineConfig(({ mode }) => ({
  plugins: [svelte()],
  resolve: mode === 'test' ? { conditions: ['browser'] } : undefined,
  build: { target: 'es2022', assetsInlineLimit: 0, modulePreload: { polyfill: false } },
  test: {
    environment: 'jsdom',
    globals: true,
    include: ['src/**/*.test.ts'],
    setupFiles: ['src/test-setup.ts']
  }
}));
