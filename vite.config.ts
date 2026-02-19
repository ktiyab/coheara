import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(({ command }) => {
  const isDev = command === 'serve';

  return {
    plugins: [sveltekit(), tailwindcss()],
    clearScreen: false,

    // Source maps: dev only (SEC-03)
    build: {
      sourcemap: isDev,
    },

    // Dev server: only applied in development mode
    ...(isDev
      ? {
          server: {
            port: 1420,
            strictPort: true,
            host: host || false,
            hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
          },
        }
      : {}),
  };
});
