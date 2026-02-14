// M0-02: QR pairing utility — X25519 ECDH + HKDF + AES-GCM
import nacl from 'tweetnacl';
import type { QrPairingData, PairResponse } from '$lib/types/pairing.js';
import { secureSet, STORAGE_KEYS } from '$lib/utils/secure-storage.js';
import { apiClient } from '$lib/api/client.js';
import { setConnected, setConnecting } from '$lib/stores/connection.js';

/** Result of a successful pairing */
export interface PairResult {
	profileName: string;
	deviceId: string;
}

/** Parse QR code text into typed pairing data. Returns null if invalid. */
export function parseQrData(text: string): QrPairingData | null {
	try {
		const data = JSON.parse(text) as Record<string, unknown>;
		if (
			typeof data.v !== 'number' ||
			typeof data.url !== 'string' ||
			typeof data.token !== 'string' ||
			typeof data.cert_fp !== 'string' ||
			typeof data.pubkey !== 'string'
		) {
			return null;
		}
		return data as unknown as QrPairingData;
	} catch {
		return null;
	}
}

/**
 * Execute the full pairing handshake with the desktop.
 *
 * Flow:
 * 1. Generate X25519 keypair (tweetnacl)
 * 2. POST /api/auth/pair with token + phone_pubkey + device info
 * 3. Desktop long-polls until user approves (up to 60s)
 * 4. Derive shared secret via ECDH
 * 5. Derive transport key via HKDF-SHA256
 * 6. Decrypt cache_key_encrypted via AES-256-GCM
 * 7. Store credentials in secure storage
 * 8. Configure API client
 */
export async function executePairing(
	qrData: QrPairingData,
	deviceName: string,
	deviceModel: string,
	onStatus: (message: string) => void
): Promise<PairResult> {
	// 1. Generate X25519 keypair
	onStatus('Generating security keys\u2026');
	const phoneKeypair = nacl.box.keyPair();
	const phonePubkeyB64 = uint8ToBase64(phoneKeypair.publicKey);

	// 2. POST /api/auth/pair (long-polls for up to 60s)
	onStatus('Waiting for desktop approval\u2026');
	const pairResponse = await postPairRequest(qrData.url, {
		token: qrData.token,
		phone_pubkey: phonePubkeyB64,
		device_name: deviceName,
		device_model: deviceModel
	});

	// 3. Derive shared secret via X25519 ECDH
	onStatus('Securing connection\u2026');
	const desktopPubBytes = base64ToUint8(qrData.pubkey);
	const sharedSecret = nacl.scalarMult(phoneKeypair.secretKey, desktopPubBytes);

	// 4. Derive transport key via HKDF-SHA256
	const transportKey = await deriveKey(sharedSecret, 'coheara-transport-key', 'v1');

	// 5. Decrypt cache_key_encrypted
	const cacheKey = await decryptCacheKey(transportKey, pairResponse.cache_key_encrypted);

	// 6. Generate device ID
	const deviceId = crypto.randomUUID();

	// 7. Store credentials in secure storage
	onStatus('Saving credentials\u2026');
	await secureSet(STORAGE_KEYS.SESSION_TOKEN, pairResponse.session_token);
	await secureSet(STORAGE_KEYS.CACHE_KEY, uint8ToBase64(new Uint8Array(cacheKey)));
	await secureSet(STORAGE_KEYS.DESKTOP_URL, qrData.url);
	await secureSet(STORAGE_KEYS.DEVICE_ID, deviceId);
	await secureSet(STORAGE_KEYS.LAST_PROFILE, pairResponse.profile_name);

	// 8. Configure API client + update connection state
	apiClient.configure({
		baseUrl: qrData.url,
		deviceId,
		sessionToken: pairResponse.session_token
	});

	const now = new Date().toISOString();
	setConnected(pairResponse.profile_name, now);

	return {
		profileName: pairResponse.profile_name,
		deviceId
	};
}

// --- HTTP ---

interface PairRequestBody {
	token: string;
	phone_pubkey: string;
	device_name: string;
	device_model: string;
}

async function postPairRequest(baseUrl: string, body: PairRequestBody): Promise<PairResponse> {
	const response = await fetch(`${baseUrl}/api/auth/pair`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(70_000) // 60s server timeout + 10s buffer
	});

	if (!response.ok) {
		const errorText = await response.text().catch(() => 'Unknown error');
		throw new PairingError(mapHttpError(response.status, errorText));
	}

	return (await response.json()) as PairResponse;
}

function mapHttpError(status: number, body: string): string {
	if (status === 403) return 'Pairing was denied by the desktop user.';
	if (status === 429) return 'Too many attempts. Please wait a moment.';
	if (status === 401) return 'Invalid or expired QR code. Generate a new one on desktop.';
	if (body.includes('TokenExpired')) return 'QR code has expired. Generate a new one on desktop.';
	if (body.includes('TokenConsumed')) return 'QR code already used. Generate a new one on desktop.';
	if (body.includes('MaxDevices')) return 'Maximum paired devices reached. Unpair a device on desktop first.';
	if (body.includes('ApprovalTimeout')) return 'Approval timed out. Try again and approve on desktop.';
	return `Pairing failed (${status}): ${body}`;
}

export class PairingError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'PairingError';
	}
}

// --- Crypto Helpers ---

async function deriveKey(
	sharedSecret: Uint8Array,
	salt: string,
	info: string
): Promise<CryptoKey> {
	// Copy to fresh ArrayBuffer (tweetnacl returns ArrayBufferLike which isn't BufferSource)
	const secretBytes = new Uint8Array(sharedSecret).buffer;
	const keyMaterial = await crypto.subtle.importKey(
		'raw',
		secretBytes,
		'HKDF',
		false,
		['deriveKey']
	);

	return crypto.subtle.deriveKey(
		{
			name: 'HKDF',
			hash: 'SHA-256',
			salt: new TextEncoder().encode(salt),
			info: new TextEncoder().encode(info)
		},
		keyMaterial,
		{ name: 'AES-GCM', length: 256 },
		false,
		['decrypt']
	);
}

async function decryptCacheKey(
	transportKey: CryptoKey,
	encryptedB64: string
): Promise<ArrayBuffer> {
	const encrypted = base64ToUint8(encryptedB64);

	// Format: 12-byte nonce + ciphertext (with 16-byte auth tag appended)
	if (encrypted.length < 13) {
		throw new PairingError('Invalid encrypted cache key format');
	}

	const nonce = encrypted.slice(0, 12);
	const ciphertext = encrypted.slice(12);

	return crypto.subtle.decrypt(
		{ name: 'AES-GCM', iv: nonce },
		transportKey,
		ciphertext
	);
}

// --- Base64 Helpers ---

function base64ToUint8(b64: string): Uint8Array {
	// Handle URL-safe base64 (no padding)
	const standard = b64.replace(/-/g, '+').replace(/_/g, '/');
	const padded = standard + '='.repeat((4 - (standard.length % 4)) % 4);
	const binary = atob(padded);
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) {
		bytes[i] = binary.charCodeAt(i);
	}
	return bytes;
}

function uint8ToBase64(bytes: Uint8Array): string {
	let binary = '';
	for (const byte of bytes) {
		binary += String.fromCharCode(byte);
	}
	// Use URL-safe base64 without padding (matches Rust base64::URL_SAFE_NO_PAD)
	return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}
