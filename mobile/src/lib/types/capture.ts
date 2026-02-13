// M1-05: Document Capture types — camera → desktop pipeline

// === QUALITY CHECKS ===

export type BrightnessLevel = 'ok' | 'too_dark' | 'too_bright';
export type SharpnessLevel = 'ok' | 'blurry';
export type CoverageLevel = 'ok' | 'too_far' | 'too_close';
export type SkewLevel = 'ok' | 'tilted';

export interface QualityCheck {
	aligned: boolean;
	brightness: BrightnessLevel;
	sharpness: SharpnessLevel;
	coverage: CoverageLevel;
	skew: SkewLevel;
	ready: boolean;
}

export interface QualityHint {
	message: string;
	severity: 'ok' | 'warning';
}

/** Quality thresholds for image validation */
export const QUALITY_THRESHOLDS = {
	BLUR_VARIANCE_MIN: 100,
	BRIGHTNESS_MIN: 50,
	BRIGHTNESS_MAX: 220,
	COVERAGE_MIN: 0.4,
	COVERAGE_MAX: 0.95,
	SKEW_MAX_DEGREES: 15
} as const;

// === CAPTURED PHOTO ===

export interface CapturedPage {
	id: string;
	dataUrl: string;
	width: number;
	height: number;
	sizeBytes: number;
	quality: QualityCheck;
	capturedAt: string;
}

export interface CaptureSession {
	pages: CapturedPage[];
	startedAt: string;
}

/** Maximum pages per document (Thomas multi-page) */
export const MAX_PAGES_PER_DOCUMENT = 10;

/** Maximum file size per page in bytes (4 MB) */
export const MAX_PAGE_SIZE_BYTES = 4 * 1024 * 1024;

/** Target width for image optimization (sufficient for OCR at 300 DPI on A4) */
export const TARGET_MAX_WIDTH = 2400;

/** JPEG quality for optimized uploads */
export const JPEG_QUALITY = 0.9;

// === UPLOAD ===

export interface UploadMetadata {
	page_count: number;
	device_name: string;
	captured_at: string;
}

export interface UploadResponse {
	document_id: string;
	status: 'processing';
	message: string;
}

export type UploadStatus =
	| 'idle'
	| 'uploading'
	| 'upload_complete'
	| 'upload_failed';

export interface UploadProgress {
	status: UploadStatus;
	percent: number;
	currentPage: number;
	totalPages: number;
	errorMessage?: string;
}

// === PROCESSING ===

export type ProcessingStage =
	| 'receiving'
	| 'extracting_text'
	| 'analyzing_content'
	| 'storing';

export const PROCESSING_STAGE_LABELS: Record<ProcessingStage, string> = {
	receiving: 'Receiving document',
	extracting_text: 'Extracting text',
	analyzing_content: 'Analyzing content',
	storing: 'Storing results'
};

export interface DocumentProcessingEvent {
	document_id: string;
	stage: ProcessingStage;
	progress?: number;
}

export interface DocumentCompleteEvent {
	document_id: string;
	title: string;
	page_count: number;
	entities_found: number;
}

export interface DocumentFailedEvent {
	document_id: string;
	reason: string;
}

export type DocumentStatus =
	| { state: 'processing'; documentId: string; stage: ProcessingStage; progress?: number; sentAt: string; pageCount: number }
	| { state: 'complete'; documentId: string; title: string; pageCount: number; entitiesFound: number }
	| { state: 'failed'; documentId: string; reason: string };

// === OFFLINE QUEUE ===

export interface QueuedDocument {
	id: string;
	pages: CapturedPage[];
	queuedAt: string;
}

// === CAPTURE FLOW ===

export type CaptureFlowStep =
	| 'camera'
	| 'preview'
	| 'uploading'
	| 'done';

// === UPLOAD ERROR TYPES ===

export type UploadErrorType =
	| 'unauthorized'
	| 'file_too_large'
	| 'unsupported_format'
	| 'storage_full'
	| 'profile_locked'
	| 'network_error'
	| 'unknown';

export const UPLOAD_ERROR_MESSAGES: Record<UploadErrorType, string> = {
	unauthorized: 'Session expired. Please reconnect to your desktop.',
	file_too_large: 'Photo is too large. Try again from closer.',
	unsupported_format: 'This file type is not supported.',
	storage_full: "Your desktop's storage is full. Free space before uploading.",
	profile_locked: 'Your desktop profile is locked. Unlock it to receive documents.',
	network_error: 'Connection lost. The photo has been saved and will be sent when reconnected.',
	unknown: 'Something went wrong. Please try again.'
};

export function mapHttpToUploadError(status: number): UploadErrorType {
	switch (status) {
		case 401: return 'unauthorized';
		case 413: return 'file_too_large';
		case 415: return 'unsupported_format';
		case 423: return 'profile_locked';
		case 507: return 'storage_full';
		default: return 'unknown';
	}
}
