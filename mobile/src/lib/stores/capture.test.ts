// M1-05: Capture store tests — 20 tests
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	captureSession,
	captureStep,
	selectedPageIndex,
	uploadProgress,
	documentStatuses,
	offlineQueue,
	pageCount,
	canAddPage,
	allPagesReady,
	isUploading,
	processingDocuments,
	completedDocuments,
	failedDocuments,
	hasProcessingDocuments,
	offlineQueueCount,
	startCaptureSession,
	addPage,
	removePage,
	retakePage,
	cancelCapture,
	setUploadStarted,
	updateUploadProgress,
	setUploadComplete,
	setUploadFailed,
	addProcessingDocument,
	updateProcessingStage,
	markDocumentComplete,
	markDocumentFailed,
	dismissDocumentStatus,
	queueForOffline,
	removeFromQueue,
	clearOfflineQueue,
	resetCaptureState
} from './capture.js';
import type { QualityCheck } from '$lib/types/capture.js';
import { MAX_PAGES_PER_DOCUMENT } from '$lib/types/capture.js';

function goodQuality(): QualityCheck {
	return { aligned: true, brightness: 'ok', sharpness: 'ok', coverage: 'ok', skew: 'ok', ready: true };
}

function badQuality(): QualityCheck {
	return { aligned: false, brightness: 'too_dark', sharpness: 'blurry', coverage: 'too_far', skew: 'tilted', ready: false };
}

// === CAPTURE SESSION ===

describe('capture — session management', () => {
	beforeEach(() => resetCaptureState());

	it('starts a capture session', () => {
		startCaptureSession();
		const session = get(captureSession);
		expect(session).not.toBeNull();
		expect(session!.pages).toEqual([]);
		expect(session!.startedAt).toBeTruthy();
		expect(get(captureStep)).toBe('camera');
	});

	it('adds a page to the session', () => {
		startCaptureSession();
		const page = addPage('data:image/jpeg;base64,abc', 2400, 3200, 1500000, goodQuality());
		expect(page.id).toMatch(/^page-/);
		expect(page.width).toBe(2400);
		expect(get(pageCount)).toBe(1);
	});

	it('adds multiple pages (Thomas multi-page flow)', () => {
		startCaptureSession();
		addPage('data:1', 2400, 3200, 1000000, goodQuality());
		addPage('data:2', 2400, 3200, 1200000, goodQuality());
		expect(get(pageCount)).toBe(2);
		expect(get(canAddPage)).toBe(true);
	});

	it('enforces max page limit', () => {
		startCaptureSession();
		for (let i = 0; i < MAX_PAGES_PER_DOCUMENT; i++) {
			addPage(`data:${i}`, 2400, 3200, 100000, goodQuality());
		}
		expect(get(pageCount)).toBe(MAX_PAGES_PER_DOCUMENT);
		expect(get(canAddPage)).toBe(false);

		// Adding beyond limit is silently ignored
		addPage('data:overflow', 2400, 3200, 100000, goodQuality());
		expect(get(pageCount)).toBe(MAX_PAGES_PER_DOCUMENT);
	});

	it('removes a page', () => {
		startCaptureSession();
		const p1 = addPage('data:1', 2400, 3200, 1000000, goodQuality());
		addPage('data:2', 2400, 3200, 1200000, goodQuality());
		removePage(p1.id);
		expect(get(pageCount)).toBe(1);
	});

	it('retakes a page (preserves ID, replaces data)', () => {
		startCaptureSession();
		const original = addPage('data:old', 2400, 3200, 1000000, badQuality());
		retakePage(original.id, 'data:new', 2400, 3200, 1100000, goodQuality());

		const session = get(captureSession);
		expect(session!.pages[0].id).toBe(original.id);
		expect(session!.pages[0].dataUrl).toBe('data:new');
		expect(session!.pages[0].quality.ready).toBe(true);
	});

	it('cancels session clears everything', () => {
		startCaptureSession();
		addPage('data:1', 2400, 3200, 1000000, goodQuality());
		cancelCapture();
		expect(get(captureSession)).toBeNull();
		expect(get(captureStep)).toBe('camera');
		expect(get(uploadProgress).status).toBe('idle');
	});
});

// === QUALITY-BASED DERIVED ===

describe('capture — quality checks', () => {
	beforeEach(() => resetCaptureState());

	it('allPagesReady is true when all pages have good quality', () => {
		startCaptureSession();
		addPage('data:1', 2400, 3200, 1000000, goodQuality());
		addPage('data:2', 2400, 3200, 1000000, goodQuality());
		expect(get(allPagesReady)).toBe(true);
	});

	it('allPagesReady is false when any page has bad quality', () => {
		startCaptureSession();
		addPage('data:1', 2400, 3200, 1000000, goodQuality());
		addPage('data:2', 2400, 3200, 1000000, badQuality());
		expect(get(allPagesReady)).toBe(false);
	});

	it('allPagesReady is false when no pages', () => {
		startCaptureSession();
		expect(get(allPagesReady)).toBe(false);
	});
});

// === UPLOAD STATE ===

describe('capture — upload lifecycle', () => {
	beforeEach(() => resetCaptureState());

	it('tracks upload progress', () => {
		setUploadStarted(2);
		expect(get(isUploading)).toBe(true);
		expect(get(uploadProgress).totalPages).toBe(2);
		expect(get(captureStep)).toBe('uploading');

		updateUploadProgress(50, 1);
		expect(get(uploadProgress).percent).toBe(50);
	});

	it('marks upload complete', () => {
		startCaptureSession();
		addPage('data:1', 2400, 3200, 1000000, goodQuality());
		setUploadStarted(1);
		setUploadComplete();
		expect(get(uploadProgress).status).toBe('upload_complete');
		expect(get(uploadProgress).percent).toBe(100);
		expect(get(captureStep)).toBe('done');
		expect(get(captureSession)).toBeNull();
	});

	it('marks upload failed with error message', () => {
		setUploadStarted(1);
		setUploadFailed('Network timeout');
		expect(get(uploadProgress).status).toBe('upload_failed');
		expect(get(uploadProgress).errorMessage).toBe('Network timeout');
	});

	it('clamps progress between 0-100', () => {
		setUploadStarted(1);
		updateUploadProgress(150, 1);
		expect(get(uploadProgress).percent).toBe(100);
		updateUploadProgress(-10, 1);
		expect(get(uploadProgress).percent).toBe(0);
	});
});

// === PROCESSING STATUS (WebSocket) ===

describe('capture — processing status', () => {
	beforeEach(() => resetCaptureState());

	it('adds processing document status', () => {
		addProcessingDocument('doc-1', 2);
		expect(get(processingDocuments)).toHaveLength(1);
		expect(get(hasProcessingDocuments)).toBe(true);
	});

	it('updates processing stage', () => {
		addProcessingDocument('doc-1', 1);
		updateProcessingStage('doc-1', 'extracting_text', 50);
		const docs = get(processingDocuments);
		expect(docs[0].state === 'processing' && docs[0].stage).toBe('extracting_text');
	});

	it('marks document complete', () => {
		addProcessingDocument('doc-1', 2);
		markDocumentComplete('doc-1', 'Prescription — Dr. Ndiaye', 2, 3);
		expect(get(completedDocuments)).toHaveLength(1);
		expect(get(processingDocuments)).toHaveLength(0);
		const completed = get(completedDocuments)[0];
		expect(completed.state === 'complete' && completed.title).toBe('Prescription — Dr. Ndiaye');
	});

	it('marks document failed', () => {
		addProcessingDocument('doc-1', 1);
		markDocumentFailed('doc-1', 'Could not extract text');
		expect(get(failedDocuments)).toHaveLength(1);
		expect(get(processingDocuments)).toHaveLength(0);
	});

	it('dismisses document status', () => {
		addProcessingDocument('doc-1', 1);
		markDocumentComplete('doc-1', 'Test', 1, 1);
		dismissDocumentStatus('doc-1');
		expect(get(documentStatuses)).toHaveLength(0);
	});
});

// === OFFLINE QUEUE ===

describe('capture — offline queue', () => {
	beforeEach(() => resetCaptureState());

	it('queues document for offline send', () => {
		const pages = [{ id: 'p1', dataUrl: 'data:1', width: 2400, height: 3200, sizeBytes: 1000000, quality: goodQuality(), capturedAt: new Date().toISOString() }];
		const queued = queueForOffline(pages);
		expect(queued.id).toMatch(/^queued-/);
		expect(get(offlineQueueCount)).toBe(1);
	});

	it('removes from offline queue', () => {
		const pages = [{ id: 'p1', dataUrl: 'data:1', width: 2400, height: 3200, sizeBytes: 1000000, quality: goodQuality(), capturedAt: new Date().toISOString() }];
		const queued = queueForOffline(pages);
		removeFromQueue(queued.id);
		expect(get(offlineQueueCount)).toBe(0);
	});

	it('clears entire offline queue', () => {
		const pages = [{ id: 'p1', dataUrl: 'data:1', width: 2400, height: 3200, sizeBytes: 1000000, quality: goodQuality(), capturedAt: new Date().toISOString() }];
		queueForOffline(pages);
		queueForOffline(pages);
		clearOfflineQueue();
		expect(get(offlineQueueCount)).toBe(0);
	});
});
