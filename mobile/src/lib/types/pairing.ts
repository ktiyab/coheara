// M0-02: QR pairing types — desktop↔phone pairing protocol

/** QR code data encoded by desktop's start_pairing() command */
export interface QrPairingData {
	v: number;
	url: string;
	token: string;
	cert_fp: string;
	pubkey: string;
}

/** Desktop's POST /api/auth/pair response */
export interface PairResponse {
	session_token: string;
	cache_key_encrypted: string;
	profile_name: string;
}

/** Pairing flow state machine */
export type PairingPhase =
	| { phase: 'idle' }
	| { phase: 'scanning' }
	| { phase: 'connecting'; message: string }
	| { phase: 'success'; profileName: string }
	| { phase: 'error'; message: string; retryable: boolean }
	| { phase: 'camera_denied' };
