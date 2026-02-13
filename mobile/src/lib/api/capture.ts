// M1-05: Document Capture API — upload to desktop
import { apiClient } from './client.js';
import type { UploadResponse, CapturedPage, UploadMetadata } from '$lib/types/capture.js';
import { mapHttpToUploadError, UPLOAD_ERROR_MESSAGES } from '$lib/types/capture.js';
import type { UploadErrorType } from '$lib/types/capture.js';

export interface UploadResult {
	ok: boolean;
	data?: UploadResponse;
	errorType?: UploadErrorType;
	errorMessage?: string;
}

/** Upload document pages to desktop for processing via L1-01→L1-04 pipeline */
export async function uploadDocument(
	pages: CapturedPage[],
	deviceName: string
): Promise<UploadResult> {
	const metadata: UploadMetadata = {
		page_count: pages.length,
		device_name: deviceName,
		captured_at: new Date().toISOString()
	};

	// Build the payload — in a real Capacitor app this would use FormData/multipart.
	// For the SvelteKit companion, we send as JSON with base64 data URLs.
	const payload = {
		metadata,
		pages: pages.map((p, i) => ({
			name: `page_${i + 1}`,
			data: p.dataUrl,
			width: p.width,
			height: p.height,
			size_bytes: p.sizeBytes
		}))
	};

	const response = await apiClient.post<UploadResponse>('/api/documents/upload', payload);

	if (response.ok && response.data) {
		return { ok: true, data: response.data };
	}

	const errorType = mapHttpToUploadError(response.status);
	return {
		ok: false,
		errorType,
		errorMessage: response.error ?? UPLOAD_ERROR_MESSAGES[errorType]
	};
}
