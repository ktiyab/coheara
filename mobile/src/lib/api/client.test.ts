// M1-01: MobileApiClient tests — auth headers, error mapping, WS handlers, storage
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { MobileApiClient } from './client.js';
import type { ApiClientConfig } from './client.js';
import { initSecureStorage, MemorySecureStorage, STORAGE_KEYS } from '$lib/utils/secure-storage.js';

// === MOCK: secure-storage (already uses MemorySecureStorage by default) ===

let storage: MemorySecureStorage;

beforeEach(() => {
	storage = new MemorySecureStorage();
	initSecureStorage(storage);
	vi.restoreAllMocks();
});

afterEach(() => {
	vi.restoreAllMocks();
});

// === HELPERS ===

function makeConfig(): ApiClientConfig {
	return {
		baseUrl: 'https://desktop.local:9443',
		deviceId: 'dev-abc-123',
		sessionToken: 'tok-xyz-789'
	};
}

function createConfiguredClient(): MobileApiClient {
	const client = new MobileApiClient();
	client.configure(makeConfig());
	return client;
}

function mockFetchResponse(status: number, body?: unknown, ok?: boolean): void {
	const isOk = ok ?? (status >= 200 && status < 300);
	vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
		ok: isOk,
		status,
		json: () => Promise.resolve(body),
		text: () => Promise.resolve(typeof body === 'string' ? body : JSON.stringify(body ?? ''))
	}));
}

function mockFetchNetworkError(message = 'Network error'): void {
	vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error(message)));
}

// === CONFIGURATION ===

describe('MobileApiClient — configuration', () => {
	it('starts unconfigured', () => {
		const client = new MobileApiClient();
		expect(client.isConfigured).toBe(false);
	});

	it('becomes configured after configure()', () => {
		const client = new MobileApiClient();
		client.configure(makeConfig());
		expect(client.isConfigured).toBe(true);
	});

	it('becomes unconfigured after destroy()', () => {
		const client = createConfiguredClient();
		client.destroy();
		expect(client.isConfigured).toBe(false);
	});

	it('returns error when making request while unconfigured', async () => {
		const client = new MobileApiClient();
		const result = await client.get('/api/health');
		expect(result.ok).toBe(false);
		expect(result.status).toBe(0);
		expect(result.error).toBe('Client not configured');
	});
});

// === AUTH HEADERS ===

describe('MobileApiClient — auth headers', () => {
	it('sends correct auth headers on GET request', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, { status: 'ok' });

		await client.get('/api/health');

		const fetchCall = vi.mocked(fetch).mock.calls[0];
		const [url, options] = fetchCall;
		expect(url).toBe('https://desktop.local:9443/api/health');
		expect(options?.method).toBe('GET');

		const headers = options?.headers as Record<string, string>;
		expect(headers['X-Device-Id']).toBe('dev-abc-123');
		expect(headers['Authorization']).toBe('Bearer tok-xyz-789');
		expect(headers['X-Nonce']).toBeTruthy();
	});

	it('sends unique nonces (UUID format) per request', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, {});

		await client.get('/api/first');
		const nonce1 = (vi.mocked(fetch).mock.calls[0][1]?.headers as Record<string, string>)['X-Nonce'];

		await client.get('/api/second');
		const nonce2 = (vi.mocked(fetch).mock.calls[1][1]?.headers as Record<string, string>)['X-Nonce'];

		expect(nonce1).not.toBe(nonce2);
		// UUID v4 format check
		expect(nonce1).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
	});

	it('includes Content-Type for POST with body', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, { id: '1' });

		await client.post('/api/chat/send', { message: 'Hello' });

		const headers = vi.mocked(fetch).mock.calls[0][1]?.headers as Record<string, string>;
		expect(headers['Content-Type']).toBe('application/json');
	});

	it('omits Content-Type for GET request', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, {});

		await client.get('/api/health');

		const headers = vi.mocked(fetch).mock.calls[0][1]?.headers as Record<string, string>;
		expect(headers['Content-Type']).toBeUndefined();
	});

	it('serializes body as JSON for POST', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, {});

		await client.post('/api/chat/send', { message: 'Hello' });

		const body = vi.mocked(fetch).mock.calls[0][1]?.body;
		expect(body).toBe('{"message":"Hello"}');
	});
});

// === HTTP METHOD ROUTING ===

describe('MobileApiClient — HTTP methods', () => {
	it('uses GET method for get()', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, {});
		await client.get('/api/test');
		expect(vi.mocked(fetch).mock.calls[0][1]?.method).toBe('GET');
	});

	it('uses POST method for post()', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, {});
		await client.post('/api/test');
		expect(vi.mocked(fetch).mock.calls[0][1]?.method).toBe('POST');
	});

	it('uses PUT method for put()', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, {});
		await client.put('/api/test');
		expect(vi.mocked(fetch).mock.calls[0][1]?.method).toBe('PUT');
	});

	it('uses DELETE method for delete()', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, {});
		await client.delete('/api/test');
		expect(vi.mocked(fetch).mock.calls[0][1]?.method).toBe('DELETE');
	});
});

// === ERROR MAPPING ===

describe('MobileApiClient — error mapping', () => {
	it('maps 200 response to ok=true with data', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, { name: 'Mamadou' });

		const result = await client.get<{ name: string }>('/api/home');
		expect(result.ok).toBe(true);
		expect(result.status).toBe(200);
		expect(result.data?.name).toBe('Mamadou');
	});

	it('maps 204 to ok=true with no data', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(204);

		const result = await client.post('/api/sync');
		expect(result.ok).toBe(true);
		expect(result.status).toBe(204);
		expect(result.data).toBeUndefined();
	});

	it('maps 401 to ok=false with error text', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(401, 'Authentication required', false);

		const result = await client.get('/api/health');
		expect(result.ok).toBe(false);
		expect(result.status).toBe(401);
		expect(result.error).toContain('Authentication required');
	});

	it('maps 404 to ok=false', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(404, 'Not found', false);

		const result = await client.get('/api/medications/unknown');
		expect(result.ok).toBe(false);
		expect(result.status).toBe(404);
	});

	it('maps 500 to ok=false', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(500, 'Internal error', false);

		const result = await client.get('/api/home');
		expect(result.ok).toBe(false);
		expect(result.status).toBe(500);
	});

	it('maps network error to status 0', async () => {
		const client = createConfiguredClient();
		mockFetchNetworkError('Failed to fetch');

		const result = await client.get('/api/health');
		expect(result.ok).toBe(false);
		expect(result.status).toBe(0);
		expect(result.error).toBe('Failed to fetch');
	});

	it('handles non-Error throw as generic network error', async () => {
		const client = createConfiguredClient();
		vi.stubGlobal('fetch', vi.fn().mockRejectedValue('string error'));

		const result = await client.get('/api/health');
		expect(result.ok).toBe(false);
		expect(result.status).toBe(0);
		expect(result.error).toBe('Network error');
	});
});

// === LOAD FROM STORAGE ===

describe('MobileApiClient — loadFromStorage', () => {
	it('returns false when no credentials stored', async () => {
		const client = new MobileApiClient();
		const result = await client.loadFromStorage();
		expect(result).toBe(false);
		expect(client.isConfigured).toBe(false);
	});

	it('returns false when only partial credentials stored', async () => {
		const client = new MobileApiClient();
		await storage.set(STORAGE_KEYS.DESKTOP_URL, 'https://desktop.local:9443');
		await storage.set(STORAGE_KEYS.DEVICE_ID, 'dev-1');
		// Missing SESSION_TOKEN

		const result = await client.loadFromStorage();
		expect(result).toBe(false);
	});

	it('returns true and configures when all credentials present', async () => {
		const client = new MobileApiClient();
		await storage.set(STORAGE_KEYS.DESKTOP_URL, 'https://desktop.local:9443');
		await storage.set(STORAGE_KEYS.DEVICE_ID, 'dev-1');
		await storage.set(STORAGE_KEYS.SESSION_TOKEN, 'token-abc');

		const result = await client.loadFromStorage();
		expect(result).toBe(true);
		expect(client.isConfigured).toBe(true);
	});
});

// === WEBSOCKET MESSAGE ROUTING ===

describe('MobileApiClient — WebSocket handlers', () => {
	it('registers and unregisters message handlers', () => {
		const client = new MobileApiClient();
		const handler = vi.fn();

		const unsubscribe = client.onMessage('ChatToken', handler);
		expect(typeof unsubscribe).toBe('function');

		// Unsubscribe should not throw
		unsubscribe();
	});

	it('sendWsMessage returns false when not connected', () => {
		const client = new MobileApiClient();
		const result = client.sendWsMessage({ type: 'Pong' });
		expect(result).toBe(false);
	});

	it('isWsConnected returns false when no WebSocket', () => {
		const client = new MobileApiClient();
		expect(client.isWsConnected).toBe(false);
	});

	it('connectWebSocket returns false when not configured', async () => {
		const client = new MobileApiClient();
		const result = await client.connectWebSocket();
		expect(result).toBe(false);
	});
});

// === ABORT SIGNAL (RS-M1-05-003) ===

describe('MobileApiClient — abort signal', () => {
	it('forwards abort signal to fetch', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, { ok: true });
		const controller = new AbortController();

		await client.post('/api/upload', { data: 'test' }, { signal: controller.signal });

		const fetchOptions = vi.mocked(fetch).mock.calls[0][1];
		expect(fetchOptions?.signal).toBe(controller.signal);
	});

	it('returns abort error when request is aborted', async () => {
		const client = createConfiguredClient();
		vi.stubGlobal('fetch', vi.fn().mockRejectedValue(
			new DOMException('The operation was aborted', 'AbortError')
		));

		const controller = new AbortController();
		const result = await client.post('/api/upload', { data: 'test' }, { signal: controller.signal });

		expect(result.ok).toBe(false);
		expect(result.status).toBe(0);
		expect(result.error).toBe('Request aborted');
	});

	it('does not send signal when not provided', async () => {
		const client = createConfiguredClient();
		mockFetchResponse(200, {});

		await client.post('/api/test', { data: 'test' });

		const fetchOptions = vi.mocked(fetch).mock.calls[0][1];
		expect(fetchOptions?.signal).toBeUndefined();
	});
});
