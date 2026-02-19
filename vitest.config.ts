import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';
import path from 'path';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  resolve: {
    conditions: ['browser'],
  },
  test: {
    environment: 'happy-dom',
    setupFiles: ['./src/test-setup.ts'],
    include: ['src/**/*.test.ts'],
    globals: true,
    alias: {
      '$lib': path.resolve('./src/lib'),
      '$app': path.resolve('./.svelte-kit/runtime/app'),
    },
  },
});
