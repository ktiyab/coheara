/** SEC-HTTPS-01: Mobile API HTTPS server types. */

/** Active HTTPS server session metadata. */
export interface MobileApiSession {
	session_id: string;
	server_addr: string;
	port: number;
	started_at: string;
}

/** Mobile API server status (running + session info). */
export interface MobileApiStatus {
	running: boolean;
	session: MobileApiSession | null;
}
