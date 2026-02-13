// M1-05: Document Capture store — photo queue, upload, processing, offline queue
import { writable, derived, get } from 'svelte/store';
import type {
	CapturedPage,
	CaptureSession,
	UploadProgress,
	DocumentStatus,
	QueuedDocument,
	CaptureFlowStep,
	QualityCheck
} from '$lib/types/capture.js';
import { MAX_PAGES_PER_DOCUMENT } from '$lib/types/capture.js';

// === CAPTURE SESSION ===

export const captureSession = writable<CaptureSession | null>(null);
export const captureStep = writable<CaptureFlowStep>('camera');
export const selectedPageIndex = writable<number>(0);

export const pageCount = derived(captureSession, ($s) => $s?.pages.length ?? 0);

export const canAddPage = derived(captureSession, ($s) =>
	($s?.pages.length ?? 0) < MAX_PAGES_PER_DOCUMENT
);

export const allPagesReady = derived(captureSession, ($s) => {
	if (!$s || $s.pages.length === 0) return false;
	return $s.pages.every((p) => p.quality.ready);
});

// === UPLOAD STATE ===

export const uploadProgress = writable<UploadProgress>({
	status: 'idle',
	percent: 0,
	currentPage: 0,
	totalPages: 0
});

export const isUploading = derived(uploadProgress, ($u) => $u.status === 'uploading');

// === PROCESSING STATUS (from WebSocket) ===

export const documentStatuses = writable<DocumentStatus[]>([]);

export const processingDocuments = derived(documentStatuses, ($docs) =>
	$docs.filter((d) => d.state === 'processing')
);

export const completedDocuments = derived(documentStatuses, ($docs) =>
	$docs.filter((d) => d.state === 'complete')
);

export const failedDocuments = derived(documentStatuses, ($docs) =>
	$docs.filter((d) => d.state === 'failed')
);

export const hasProcessingDocuments = derived(processingDocuments, ($p) => $p.length > 0);

// === OFFLINE QUEUE ===

export const offlineQueue = writable<QueuedDocument[]>([]);

export const offlineQueueCount = derived(offlineQueue, ($q) => $q.length);

// === SESSION MANAGEMENT ===

let pageCounter = 0;

export function startCaptureSession(): void {
	pageCounter = 0;
	captureSession.set({
		pages: [],
		startedAt: new Date().toISOString()
	});
	captureStep.set('camera');
	selectedPageIndex.set(0);
}

export function addPage(dataUrl: string, width: number, height: number, sizeBytes: number, quality: QualityCheck): CapturedPage {
	const page: CapturedPage = {
		id: `page-${Date.now()}-${++pageCounter}`,
		dataUrl,
		width,
		height,
		sizeBytes,
		quality,
		capturedAt: new Date().toISOString()
	};

	captureSession.update(($s) => {
		if (!$s) return $s;
		if ($s.pages.length >= MAX_PAGES_PER_DOCUMENT) return $s;
		return { ...$s, pages: [...$s.pages, page] };
	});

	return page;
}

export function removePage(pageId: string): void {
	captureSession.update(($s) => {
		if (!$s) return $s;
		const filtered = $s.pages.filter((p) => p.id !== pageId);
		return { ...$s, pages: filtered };
	});

	// Adjust selected index if needed
	const session = get(captureSession);
	const idx = get(selectedPageIndex);
	if (session && idx >= session.pages.length && session.pages.length > 0) {
		selectedPageIndex.set(session.pages.length - 1);
	}
}

export function retakePage(pageId: string, dataUrl: string, width: number, height: number, sizeBytes: number, quality: QualityCheck): void {
	captureSession.update(($s) => {
		if (!$s) return $s;
		const pages = $s.pages.map((p) => {
			if (p.id !== pageId) return p;
			return { ...p, dataUrl, width, height, sizeBytes, quality, capturedAt: new Date().toISOString() };
		});
		return { ...$s, pages };
	});
}

export function cancelCapture(): void {
	captureSession.set(null);
	captureStep.set('camera');
	selectedPageIndex.set(0);
	uploadProgress.set({ status: 'idle', percent: 0, currentPage: 0, totalPages: 0 });
}

// === UPLOAD MANAGEMENT ===

export function setUploadStarted(totalPages: number): void {
	uploadProgress.set({ status: 'uploading', percent: 0, currentPage: 1, totalPages });
	captureStep.set('uploading');
}

export function updateUploadProgress(percent: number, currentPage: number): void {
	uploadProgress.update(($u) => ({
		...$u,
		percent: Math.min(100, Math.max(0, percent)),
		currentPage
	}));
}

export function setUploadComplete(): void {
	uploadProgress.update(($u) => ({
		...$u,
		status: 'upload_complete',
		percent: 100
	}));
	captureStep.set('done');
	// Clear session pages after successful upload
	captureSession.set(null);
}

export function setUploadFailed(errorMessage: string): void {
	uploadProgress.update(($u) => ({
		...$u,
		status: 'upload_failed',
		errorMessage
	}));
}

// === PROCESSING STATUS MANAGEMENT ===

let documentIdCounter = 0;

export function addProcessingDocument(documentId: string, pageCount: number): void {
	documentStatuses.update(($docs) => [
		...$docs,
		{
			state: 'processing' as const,
			documentId,
			stage: 'receiving' as const,
			sentAt: new Date().toISOString(),
			pageCount
		}
	]);
}

export function updateProcessingStage(documentId: string, stage: import('$lib/types/capture.js').ProcessingStage, progress?: number): void {
	documentStatuses.update(($docs) =>
		$docs.map((d) => {
			if (d.documentId !== documentId || d.state !== 'processing') return d;
			return { ...d, stage, progress };
		})
	);
}

export function markDocumentComplete(documentId: string, title: string, pageCount: number, entitiesFound: number): void {
	documentStatuses.update(($docs) =>
		$docs.map((d) => {
			if (d.documentId !== documentId) return d;
			return {
				state: 'complete' as const,
				documentId,
				title,
				pageCount,
				entitiesFound
			};
		})
	);
}

export function markDocumentFailed(documentId: string, reason: string): void {
	documentStatuses.update(($docs) =>
		$docs.map((d) => {
			if (d.documentId !== documentId) return d;
			return {
				state: 'failed' as const,
				documentId,
				reason
			};
		})
	);
}

export function dismissDocumentStatus(documentId: string): void {
	documentStatuses.update(($docs) =>
		$docs.filter((d) => d.documentId !== documentId)
	);
}

// === OFFLINE QUEUE MANAGEMENT ===

let queueCounter = 0;

export function queueForOffline(pages: CapturedPage[]): QueuedDocument {
	const doc: QueuedDocument = {
		id: `queued-${Date.now()}-${++queueCounter}`,
		pages,
		queuedAt: new Date().toISOString()
	};
	offlineQueue.update(($q) => [...$q, doc]);
	return doc;
}

export function removeFromQueue(queuedId: string): void {
	offlineQueue.update(($q) => $q.filter((d) => d.id !== queuedId));
}

export function clearOfflineQueue(): void {
	offlineQueue.set([]);
}

// === RESET ===

export function resetCaptureState(): void {
	pageCounter = 0;
	documentIdCounter = 0;
	queueCounter = 0;
	captureSession.set(null);
	captureStep.set('camera');
	selectedPageIndex.set(0);
	uploadProgress.set({ status: 'idle', percent: 0, currentPage: 0, totalPages: 0 });
	documentStatuses.set([]);
	offlineQueue.set([]);
}
