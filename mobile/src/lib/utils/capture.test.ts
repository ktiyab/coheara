// M1-05: Capture utils tests — 22 tests
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
	evaluateQuality,
	getQualityHint,
	qualityLabel,
	qualityColor,
	frameBorderColor,
	calculateOptimizedSize,
	estimateOptimizedSize,
	isPageTooLarge,
	formatFileSize,
	totalUploadSize,
	makeGoodQuality,
	makeBadQuality,
	uploadProgressText,
	processingStageText,
	stripExifMetadata,
	stripAndOptimize
} from './capture.js';
import type { CapturedPage } from '$lib/types/capture.js';
import { QUALITY_THRESHOLDS, MAX_PAGE_SIZE_BYTES } from '$lib/types/capture.js';

// === QUALITY EVALUATION ===

describe('capture utils — quality evaluation', () => {
	it('evaluates all-good quality as ready', () => {
		const q = evaluateQuality(120, 200, true, 4, 0.6, 5);
		expect(q.ready).toBe(true);
		expect(q.aligned).toBe(true);
		expect(q.brightness).toBe('ok');
		expect(q.sharpness).toBe('ok');
		expect(q.coverage).toBe('ok');
		expect(q.skew).toBe('ok');
	});

	it('detects too dark brightness', () => {
		const q = evaluateQuality(30, 200, true, 4, 0.6, 5);
		expect(q.brightness).toBe('too_dark');
		expect(q.ready).toBe(false);
	});

	it('detects too bright brightness', () => {
		const q = evaluateQuality(230, 200, true, 4, 0.6, 5);
		expect(q.brightness).toBe('too_bright');
		expect(q.ready).toBe(false);
	});

	it('detects blurry image', () => {
		const q = evaluateQuality(120, 50, true, 4, 0.6, 5);
		expect(q.sharpness).toBe('blurry');
		expect(q.ready).toBe(false);
	});

	it('detects too far coverage', () => {
		const q = evaluateQuality(120, 200, true, 4, 0.2, 5);
		expect(q.coverage).toBe('too_far');
		expect(q.ready).toBe(false);
	});

	it('detects too close coverage', () => {
		const q = evaluateQuality(120, 200, true, 4, 0.98, 5);
		expect(q.coverage).toBe('too_close');
		expect(q.ready).toBe(false);
	});

	it('detects tilted document', () => {
		const q = evaluateQuality(120, 200, true, 4, 0.6, 20);
		expect(q.skew).toBe('tilted');
		expect(q.ready).toBe(false);
	});

	it('detects unaligned edges', () => {
		const q = evaluateQuality(120, 200, false, 2, 0.6, 5);
		expect(q.aligned).toBe(false);
		expect(q.ready).toBe(false);
	});
});

// === QUALITY HINTS ===

describe('capture utils — quality hints', () => {
	it('shows "Ready to capture" when all ok', () => {
		const hint = getQualityHint(makeGoodQuality());
		expect(hint.message).toBe('Ready to capture');
		expect(hint.severity).toBe('ok');
	});

	it('prioritizes darkness hint', () => {
		const hint = getQualityHint(makeBadQuality());
		expect(hint.message).toBe('Move to better light');
		expect(hint.severity).toBe('warning');
	});

	it('shows alignment hint when not aligned', () => {
		const q = makeGoodQuality();
		q.aligned = false;
		q.ready = false;
		const hint = getQualityHint(q);
		expect(hint.message).toBe('Align document within frame');
	});

	it('qualityLabel summarizes issues', () => {
		expect(qualityLabel(makeGoodQuality())).toBe('Good');
		const bad = makeBadQuality();
		const label = qualityLabel(bad);
		expect(label).toContain('lighting');
		expect(label).toContain('focus');
	});

	it('qualityColor returns success for ready, warning otherwise', () => {
		expect(qualityColor(makeGoodQuality())).toBe('var(--color-success)');
		expect(qualityColor(makeBadQuality())).toBe('var(--color-warning)');
	});

	it('frameBorderColor matches quality state', () => {
		expect(frameBorderColor(makeGoodQuality())).toBe('var(--color-success)');
		expect(frameBorderColor(makeBadQuality())).toBe('var(--color-warning)');
	});
});

// === IMAGE OPTIMIZATION ===

describe('capture utils — image optimization', () => {
	it('calculates optimized size for oversized image', () => {
		const result = calculateOptimizedSize(4800, 6400);
		expect(result.width).toBe(2400);
		expect(result.height).toBe(3200);
	});

	it('preserves size for small image', () => {
		const result = calculateOptimizedSize(1200, 1600);
		expect(result.width).toBe(1200);
		expect(result.height).toBe(1600);
	});

	it('detects page too large', () => {
		expect(isPageTooLarge(MAX_PAGE_SIZE_BYTES + 1)).toBe(true);
		expect(isPageTooLarge(MAX_PAGE_SIZE_BYTES)).toBe(false);
		expect(isPageTooLarge(1000000)).toBe(false);
	});

	it('formats file sizes correctly', () => {
		expect(formatFileSize(500)).toBe('500 B');
		expect(formatFileSize(1500)).toBe('1.5 KB');
		expect(formatFileSize(1500000)).toBe('1.4 MB');
	});

	it('calculates total upload size', () => {
		const pages: CapturedPage[] = [
			{ id: 'p1', dataUrl: 'data:1', width: 2400, height: 3200, sizeBytes: 1000000, quality: makeGoodQuality(), capturedAt: '' },
			{ id: 'p2', dataUrl: 'data:2', width: 2400, height: 3200, sizeBytes: 1500000, quality: makeGoodQuality(), capturedAt: '' }
		];
		expect(totalUploadSize(pages)).toBe(2500000);
	});
});

// === PROGRESS & STAGE TEXT ===

describe('capture utils — display text', () => {
	it('upload progress text for single page', () => {
		expect(uploadProgressText(1, 1, 72)).toBe('Sending... 72%');
	});

	it('upload progress text for multi-page', () => {
		expect(uploadProgressText(1, 2, 50)).toBe('Page 1 of 2 · 50%');
	});

	it('processing stage text with page count', () => {
		expect(processingStageText('extracting_text', 2)).toBe('Extracting text · 2 pages');
		expect(processingStageText('extracting_text', 1)).toBe('Extracting text · 1 page');
		expect(processingStageText('analyzing_content', 1)).toBe('Analyzing content');
		expect(processingStageText('storing', 1)).toBe('Storing results');
	});
});

// === EXIF STRIPPING ===

describe('capture utils — EXIF stripping', () => {
	let mockCtx: { drawImage: ReturnType<typeof vi.fn> };
	let mockCanvas: { width: number; height: number; getContext: ReturnType<typeof vi.fn>; toDataURL: ReturnType<typeof vi.fn> };
	let origDocument: typeof globalThis.document;
	let origImage: typeof globalThis.Image;

	beforeEach(() => {
		mockCtx = { drawImage: vi.fn() };
		mockCanvas = {
			width: 0,
			height: 0,
			getContext: vi.fn(() => mockCtx),
			toDataURL: vi.fn(() => 'data:image/jpeg;base64,cleanimage')
		};

		// Save originals and mock document + Image
		origDocument = globalThis.document;
		origImage = globalThis.Image;

		globalThis.document = {
			createElement: vi.fn(() => mockCanvas)
		} as unknown as Document;
	});

	afterEach(() => {
		globalThis.document = origDocument;
		globalThis.Image = origImage;
		vi.restoreAllMocks();
	});

	function mockImageClass(width: number, height: number): void {
		globalThis.Image = vi.fn().mockImplementation(function (this: Record<string, unknown>) {
			const self = this;
			self.naturalWidth = width;
			self.naturalHeight = height;
			setTimeout(() => {
				if (typeof self.onload === 'function') (self.onload as () => void)();
			}, 0);
			return self;
		}) as unknown as typeof Image;
	}

	it('stripExifMetadata redraws image via canvas to strip metadata', async () => {
		mockImageClass(800, 600);

		const result = await stripExifMetadata('data:image/jpeg;base64,originalwithexif');

		expect(result).toBe('data:image/jpeg;base64,cleanimage');
		expect(mockCanvas.getContext).toHaveBeenCalledWith('2d');
		expect(mockCtx.drawImage).toHaveBeenCalled();
		expect(mockCanvas.toDataURL).toHaveBeenCalledWith('image/jpeg', expect.any(Number));
	});

	it('stripAndOptimize resizes while stripping metadata', async () => {
		mockImageClass(4000, 3000);

		const result = await stripAndOptimize('data:image/jpeg;base64,largeimage');

		expect(result).toBe('data:image/jpeg;base64,cleanimage');
		expect(mockCanvas.width).toBeLessThanOrEqual(2400);
		expect(mockCtx.drawImage).toHaveBeenCalled();
	});

	it('stripExifMetadata returns original if canvas context fails', async () => {
		mockCanvas.getContext = vi.fn(() => null);
		mockImageClass(100, 100);

		const original = 'data:image/jpeg;base64,keepme';
		const result = await stripExifMetadata(original);
		expect(result).toBe(original);
	});
});
