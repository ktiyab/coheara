/** ADS: App Distribution Server types. */

/** QR code data for the install page. */
export interface InstallQrCode {
	url: string;
	svg: string;
	desktop_version: string;
}

/** Distribution server session metadata. */
export interface DistributionSession {
	session_id: string;
	server_addr: string;
	url: string;
	started_at: string;
	desktop_version: string;
}

/** Distribution server status. */
export interface DistributionStatus {
	session: DistributionSession;
	request_count: number;
	apk_available: boolean;
	pwa_available: boolean;
}
