import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig, type Plugin } from 'vite';

const host = process.env.TAURI_DEV_HOST;

// WSL2: project lives on /mnt/c/ (Windows 9P mount, ~100x slower I/O).
// Redirect Vite's cache + preprocessed deps to the native Linux filesystem.
const isWSL = process.platform === 'linux' && import.meta.dirname?.startsWith('/mnt/');
const cacheDir = isWSL ? '/tmp/coheara-vite-cache' : undefined;

/**
 * WSL2 workaround: Vite's SSR module runner has a hardcoded 60s transport
 * timeout. On the 9P filesystem (/mnt/c/), module fetching exceeds this
 * because each file read has ~10-100ms latency. This plugin raises the
 * timeout so the first request can complete while modules are loaded.
 */
function wsl2Timeout(): Plugin {
  return {
    name: 'coheara:wsl2-timeout',
    configureServer(server) {
      if (!isWSL) return;
      const ssr = server.environments?.ssr;
      const transport = (ssr as any)?.runner?.transport;
      if (transport) {
        transport.timeout = 300_000; // 5 minutes
      }
    },
  };
}

export default defineConfig(({ command }) => {
  const isDev = command === 'serve';

  return {
    plugins: [sveltekit(), tailwindcss(), ...(isWSL && isDev ? [wsl2Timeout()] : [])],
    clearScreen: false,
    ...(cacheDir ? { cacheDir } : {}),

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
            // WSL2: warm up critical entry points to pre-transform them
            warmup: {
              clientFiles: [
                './src/routes/+layout.svelte',
                './src/routes/+page.svelte',
                './src/lib/components/profile/ProfileGuard.svelte',
              ],
            },
          },
        }
      : {}),
  };
});
