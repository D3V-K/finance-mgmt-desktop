import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Offline-first desktop build: bundled local assets only, zero remote asset dependencies.
export default defineConfig(({ mode }) => ({
  plugins: [svelte()],
  resolve: mode === 'test' ? { conditions: ['browser'] } : undefined,
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: 'es2022',
    assetsInlineLimit: 0,
    modulePreload: { polyfill: false },
    outDir: 'dist',
  },
  test: {
    environment: 'jsdom',
    globals: true,
    include: ['src/**/*.test.ts'],
    setupFiles: ['src/test-setup.ts'],
  },
}));
