/// <reference types="@sveltejs/kit" />
/// <reference lib="webworker" />

/**
 * ADS: Service worker for PWA support.
 *
 * Strategy:
 * - App shell (HTML, JS, CSS): Cache-first — works fully offline
 * - API calls (/api/**): Network-first with no cache fallback
 * - Other requests: Network-first
 *
 * The service worker version is tied to the build — any new build
 * produces new file hashes, triggering a SW update.
 */

declare const self: ServiceWorkerGlobalScope;

import { build, files, version } from '$service-worker';

const CACHE_NAME = `coheara-${version}`;

// Assets to pre-cache: build output (JS/CSS) + static files
const PRECACHE_ASSETS = [...build, ...files];

// Install: pre-cache all app shell assets
self.addEventListener('install', (event: ExtendableEvent) => {
	event.waitUntil(
		caches
			.open(CACHE_NAME)
			.then((cache) => cache.addAll(PRECACHE_ASSETS))
			.then(() => self.skipWaiting())
	);
});

// Activate: clean up old caches
self.addEventListener('activate', (event: ExtendableEvent) => {
	event.waitUntil(
		caches
			.keys()
			.then((keys) =>
				Promise.all(
					keys
						.filter((key) => key !== CACHE_NAME)
						.map((key) => caches.delete(key))
				)
			)
			.then(() => self.clients.claim())
	);
});

// Fetch: cache-first for app shell, network-first for API
self.addEventListener('fetch', (event: FetchEvent) => {
	const url = new URL(event.request.url);

	// Skip non-GET requests
	if (event.request.method !== 'GET') return;

	// Skip API and WebSocket requests — always go to network
	if (url.pathname.startsWith('/api/') || url.pathname.startsWith('/ws/')) return;

	// For app shell assets: cache-first
	if (PRECACHE_ASSETS.includes(url.pathname)) {
		event.respondWith(
			caches.match(event.request).then((cached) => {
				return cached || fetch(event.request);
			})
		);
		return;
	}

	// For everything else: network-first with cache fallback
	event.respondWith(
		fetch(event.request)
			.then((response) => {
				// Cache successful responses for offline use
				if (response.ok) {
					const clone = response.clone();
					caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
				}
				return response;
			})
			.catch(() => {
				// Offline: try cache
				return caches.match(event.request).then((cached) => {
					return cached || new Response('Offline', { status: 503 });
				});
			})
	);
});
