// M1-01: HTTP + WebSocket API client — replaces Tauri invoke() for mobile
import { secureGet, STORAGE_KEYS } from '$lib/utils/secure-storage.js';

/** API client configuration */
export interface ApiClientConfig {
	baseUrl: string;
	deviceId: string;
	sessionToken: string;
}

/** Device signature — enriches Paired Devices view on desktop */
export interface DeviceSignature {
	name: string;
	os: string;
	model: string;
}

/** WebSocket message handler */
export type WsMessageHandler = (data: unknown) => void;

/** HTTP response wrapper */
export interface ApiResponse<T> {
	ok: boolean;
	status: number;
	data?: T;
	error?: string;
}

/**
 * Mobile API client — communicates with desktop over local WiFi.
 * Handles auth headers, nonce generation, and error mapping.
 */
export class MobileApiClient {
	private config: ApiClientConfig | null = null;
	private ws: WebSocket | null = null;
	private wsHandlers = new Map<string, WsMessageHandler[]>();
	private lastNonce = '';
	private deviceSignature: DeviceSignature = { name: 'Unknown', os: 'Unknown', model: 'Unknown' };

	/** Initialize with connection details from pairing */
	configure(config: ApiClientConfig): void {
		this.config = config;
	}

	/** Set device signature for Paired Devices enrichment on desktop */
	setDeviceSignature(signature: DeviceSignature): void {
		this.deviceSignature = signature;
	}

	/** Check if client is configured */
	get isConfigured(): boolean {
		return this.config !== null;
	}

	/** Load configuration from secure storage */
	async loadFromStorage(): Promise<boolean> {
		const baseUrl = await secureGet(STORAGE_KEYS.DESKTOP_URL);
		const deviceId = await secureGet(STORAGE_KEYS.DEVICE_ID);
		const token = await secureGet(STORAGE_KEYS.SESSION_TOKEN);

		if (!baseUrl || !deviceId || !token) return false;

		this.config = { baseUrl, deviceId, sessionToken: token };
		return true;
	}

	/** Make an authenticated GET request */
	async get<T>(path: string): Promise<ApiResponse<T>> {
		return this.request<T>('GET', path);
	}

	/** Make an authenticated POST request */
	async post<T>(path: string, body?: unknown, options?: { signal?: AbortSignal }): Promise<ApiResponse<T>> {
		return this.request<T>('POST', path, body, options);
	}

	/** Make an authenticated PUT request */
	async put<T>(path: string, body?: unknown, options?: { signal?: AbortSignal }): Promise<ApiResponse<T>> {
		return this.request<T>('PUT', path, body, options);
	}

	/** Make an authenticated DELETE request */
	async delete<T>(path: string): Promise<ApiResponse<T>> {
		return this.request<T>('DELETE', path);
	}

	/** Core request method with auth headers */
	private async request<T>(
		method: string,
		path: string,
		body?: unknown,
		options?: { signal?: AbortSignal }
	): Promise<ApiResponse<T>> {
		if (!this.config) {
			return { ok: false, status: 0, error: 'Client not configured' };
		}

		const url = `${this.config.baseUrl}${path}`;
		const nonce = crypto.randomUUID();
		this.lastNonce = nonce;

		const headers: Record<string, string> = {
			'X-Device-Id': this.config.deviceId,
			'Authorization': `Bearer ${this.config.sessionToken}`,
			'X-Request-Nonce': nonce,
			'X-Request-Timestamp': String(Math.floor(Date.now() / 1000)),
			'X-Device-Name': this.deviceSignature.name,
			'X-Device-OS': this.deviceSignature.os,
			'X-Device-Model': this.deviceSignature.model
		};

		if (body !== undefined) {
			headers['Content-Type'] = 'application/json';
		}

		try {
			const response = await fetch(url, {
				method,
				headers,
				body: body !== undefined ? JSON.stringify(body) : undefined,
				signal: options?.signal
			});

			if (response.status === 204) {
				return { ok: true, status: 204 };
			}

			// Rotate session token if desktop provides a new one
			const newToken = response.headers.get('X-New-Token');
			if (newToken && this.config) {
				this.config.sessionToken = newToken;
			}

			if (!response.ok) {
				const errorText = await response.text().catch(() => 'Unknown error');
				return { ok: false, status: response.status, error: errorText };
			}

			const data = await response.json() as T;
			return { ok: true, status: response.status, data };
		} catch (err) {
			if (err instanceof DOMException && err.name === 'AbortError') {
				return { ok: false, status: 0, error: 'Request aborted' };
			}
			const message = err instanceof Error ? err.message : 'Network error';
			return { ok: false, status: 0, error: message };
		}
	}

	// --- WebSocket ---

	/** Connect WebSocket for real-time updates.
	 *  Flow: POST /api/auth/ws-ticket → GET /ws/connect?ticket=xxx
	 *  Ticket is one-time, 30s TTL — session token never exposed in URL. */
	async connectWebSocket(): Promise<boolean> {
		if (!this.config) return false;

		try {
			// Step 1: Acquire one-time WS ticket via authenticated REST call
			const ticketResp = await this.post<{ ticket: string; expires_in: number }>(
				'/api/auth/ws-ticket'
			);
			if (!ticketResp.ok || !ticketResp.data?.ticket) return false;

			// Step 2: Connect to WS endpoint with ticket (not session token)
			const wsUrl = this.config.baseUrl
				.replace('https://', 'wss://')
				.replace('http://', 'ws://');

			this.ws = new WebSocket(
				`${wsUrl}/ws/connect?ticket=${encodeURIComponent(ticketResp.data.ticket)}`
			);

			return new Promise((resolve) => {
				if (!this.ws) { resolve(false); return; }

				this.ws.onopen = () => resolve(true);
				this.ws.onerror = () => resolve(false);
				this.ws.onclose = () => { this.ws = null; };
				this.ws.onmessage = (event) => {
					try {
						const msg = JSON.parse(event.data as string) as Record<string, unknown>;
						const msgType = msg.type as string;
						if (!msgType) return;

						// Auto-respond to Heartbeat with Pong (feeds green dot status)
						if (msgType === 'Heartbeat') {
							this.sendWsMessage({ type: 'Pong' });
							return;
						}

						const handlers = this.wsHandlers.get(msgType);
						if (handlers) {
							for (const handler of handlers) {
								handler(msg);
							}
						}
					} catch {
						// Ignore malformed messages
					}
				};
			});
		} catch {
			return false;
		}
	}

	/** Register a WebSocket message handler */
	onMessage(type: string, handler: WsMessageHandler): () => void {
		const handlers = this.wsHandlers.get(type) ?? [];
		handlers.push(handler);
		this.wsHandlers.set(type, handlers);

		return () => {
			const current = this.wsHandlers.get(type);
			if (current) {
				this.wsHandlers.set(type, current.filter((h) => h !== handler));
			}
		};
	}

	/** Send a message over WebSocket. Returns true if sent successfully. */
	sendWsMessage(payload: Record<string, unknown>): boolean {
		if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return false;
		try {
			this.ws.send(JSON.stringify(payload));
			return true;
		} catch {
			return false;
		}
	}

	/** Disconnect WebSocket */
	disconnectWebSocket(): void {
		if (this.ws) {
			this.ws.close();
			this.ws = null;
		}
	}

	/** Check if WebSocket is connected */
	get isWsConnected(): boolean {
		return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
	}

	/** Clean up all resources */
	destroy(): void {
		this.disconnectWebSocket();
		this.wsHandlers.clear();
		this.config = null;
	}
}

/** Singleton API client instance */
export const apiClient = new MobileApiClient();
