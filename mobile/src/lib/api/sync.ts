// M0-04: Sync API client — POST /api/sync
import type { SyncRequest, SyncResponse } from '$lib/types/sync.js';

/** Result of a sync API call */
export type SyncApiResult =
	| { status: 204 }
	| { status: 200; data: SyncResponse }
	| { status: 'error'; message: string };

/** Send sync request to desktop */
export async function postSync(
	baseUrl: string,
	token: string,
	request: SyncRequest
): Promise<SyncApiResult> {
	try {
		const response = await fetch(`${baseUrl}/api/sync`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
				'Authorization': `Bearer ${token}`
			},
			body: JSON.stringify(request)
		});

		if (response.status === 204) {
			return { status: 204 };
		}

		if (response.ok) {
			const data: SyncResponse = await response.json();
			return { status: 200, data };
		}

		return { status: 'error', message: `Sync failed: ${response.status}` };
	} catch (err) {
		return {
			status: 'error',
			message: err instanceof Error ? err.message : 'Network error'
		};
	}
}
