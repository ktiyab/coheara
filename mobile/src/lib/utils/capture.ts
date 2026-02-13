// M1-05: Document Capture utils — quality checks, image optimization, quality hints
import type {
	QualityCheck,
	QualityHint,
	BrightnessLevel,
	SharpnessLevel,
	CoverageLevel,
	SkewLevel,
	CapturedPage
} from '$lib/types/capture.js';
import {
	QUALITY_THRESHOLDS,
	MAX_PAGE_SIZE_BYTES,
	TARGET_MAX_WIDTH,
	JPEG_QUALITY
} from '$lib/types/capture.js';

// === QUALITY CHECK ===

/** Build a QualityCheck from individual metric values */
export function evaluateQuality(
	brightness: number,
	blurVariance: number,
	edgesFound: boolean,
	cornerCount: number,
	areaPercent: number,
	skewDegrees: number
): QualityCheck {
	const brightnessLevel: BrightnessLevel =
		brightness < QUALITY_THRESHOLDS.BRIGHTNESS_MIN ? 'too_dark' :
		brightness > QUALITY_THRESHOLDS.BRIGHTNESS_MAX ? 'too_bright' : 'ok';

	const sharpnessLevel: SharpnessLevel =
		blurVariance < QUALITY_THRESHOLDS.BLUR_VARIANCE_MIN ? 'blurry' : 'ok';

	const coverageLevel: CoverageLevel =
		areaPercent < QUALITY_THRESHOLDS.COVERAGE_MIN ? 'too_far' :
		areaPercent > QUALITY_THRESHOLDS.COVERAGE_MAX ? 'too_close' : 'ok';

	const skewLevel: SkewLevel =
		skewDegrees > QUALITY_THRESHOLDS.SKEW_MAX_DEGREES ? 'tilted' : 'ok';

	const aligned = edgesFound && cornerCount === 4;

	const ready =
		aligned &&
		brightnessLevel === 'ok' &&
		sharpnessLevel === 'ok' &&
		coverageLevel === 'ok' &&
		skewLevel === 'ok';

	return { aligned, brightness: brightnessLevel, sharpness: sharpnessLevel, coverage: coverageLevel, skew: skewLevel, ready };
}

/** Get human-readable quality hint from a check result */
export function getQualityHint(check: QualityCheck): QualityHint {
	if (check.ready) {
		return { message: 'Ready to capture', severity: 'ok' };
	}
	if (check.brightness === 'too_dark') {
		return { message: 'Move to better light', severity: 'warning' };
	}
	if (check.brightness === 'too_bright') {
		return { message: 'Too bright — reduce glare', severity: 'warning' };
	}
	if (check.sharpness === 'blurry') {
		return { message: 'Hold steady', severity: 'warning' };
	}
	if (check.coverage === 'too_far') {
		return { message: 'Move closer', severity: 'warning' };
	}
	if (check.coverage === 'too_close') {
		return { message: 'Move further away', severity: 'warning' };
	}
	if (check.skew === 'tilted') {
		return { message: 'Straighten the document', severity: 'warning' };
	}
	if (!check.aligned) {
		return { message: 'Align document within frame', severity: 'warning' };
	}
	return { message: 'Adjusting...', severity: 'warning' };
}

/** Get quality check summary label */
export function qualityLabel(check: QualityCheck): string {
	if (check.ready) return 'Good';
	const issues: string[] = [];
	if (check.brightness !== 'ok') issues.push('lighting');
	if (check.sharpness !== 'ok') issues.push('focus');
	if (check.coverage !== 'ok') issues.push('distance');
	if (check.skew !== 'ok') issues.push('alignment');
	if (!check.aligned) issues.push('edges');
	return issues.length > 0 ? `Issues: ${issues.join(', ')}` : 'Checking...';
}

/** Get quality color class */
export function qualityColor(check: QualityCheck): string {
	return check.ready ? 'var(--color-success)' : 'var(--color-warning)';
}

/** Get frame border color for camera overlay */
export function frameBorderColor(check: QualityCheck): string {
	return check.ready ? 'var(--color-success)' : 'var(--color-warning)';
}

// === EXIF STRIPPING (BP-02: DATA MINIMIZATION) ===

/**
 * Strip EXIF metadata from an image data URL while preserving visual content.
 *
 * Uses the Canvas API to redraw the image, which inherently removes all
 * EXIF metadata (GPS coordinates, camera make/model, timestamps, etc.).
 * Modern browsers automatically apply EXIF orientation when drawing to canvas,
 * so the visual orientation is preserved even though the EXIF tag is removed.
 *
 * @param dataUrl - Base64 data URL of the captured image
 * @returns Promise resolving to a clean data URL with no EXIF metadata
 */
export async function stripExifMetadata(dataUrl: string): Promise<string> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		img.onload = () => {
			try {
				const canvas = document.createElement('canvas');
				canvas.width = img.naturalWidth;
				canvas.height = img.naturalHeight;

				const ctx = canvas.getContext('2d');
				if (!ctx) {
					// Fallback: return original if canvas context unavailable
					resolve(dataUrl);
					return;
				}

				ctx.drawImage(img, 0, 0);

				// Export as JPEG with the configured quality
				const cleanDataUrl = canvas.toDataURL('image/jpeg', JPEG_QUALITY);
				resolve(cleanDataUrl);
			} catch {
				// Fallback: return original on any canvas error
				resolve(dataUrl);
			}
		};
		img.onerror = () => reject(new Error('Failed to load image for EXIF stripping'));
		img.src = dataUrl;
	});
}

/**
 * Strip EXIF and resize in one pass (combines stripping + optimization).
 *
 * @param dataUrl - Base64 data URL of the captured image
 * @param maxWidth - Maximum output width (preserves aspect ratio)
 * @returns Promise resolving to a clean, optimized data URL
 */
export async function stripAndOptimize(dataUrl: string, maxWidth: number = TARGET_MAX_WIDTH): Promise<string> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		img.onload = () => {
			try {
				const { width, height } = calculateOptimizedSize(img.naturalWidth, img.naturalHeight);

				const canvas = document.createElement('canvas');
				canvas.width = width;
				canvas.height = height;

				const ctx = canvas.getContext('2d');
				if (!ctx) {
					resolve(dataUrl);
					return;
				}

				ctx.drawImage(img, 0, 0, width, height);
				const cleanDataUrl = canvas.toDataURL('image/jpeg', JPEG_QUALITY);
				resolve(cleanDataUrl);
			} catch {
				resolve(dataUrl);
			}
		};
		img.onerror = () => reject(new Error('Failed to load image for optimization'));
		img.src = dataUrl;
	});
}

// === IMAGE OPTIMIZATION ===

/** Calculate optimized dimensions (max width TARGET_MAX_WIDTH, preserve aspect ratio) */
export function calculateOptimizedSize(width: number, height: number): { width: number; height: number } {
	if (width <= TARGET_MAX_WIDTH) {
		return { width, height };
	}
	const ratio = TARGET_MAX_WIDTH / width;
	return {
		width: TARGET_MAX_WIDTH,
		height: Math.round(height * ratio)
	};
}

/** Estimate optimized file size in bytes (rough JPEG estimate) */
export function estimateOptimizedSize(width: number, height: number): number {
	const optimized = calculateOptimizedSize(width, height);
	// Rough JPEG estimate: ~1 byte per pixel at quality 90
	return Math.round(optimized.width * optimized.height * JPEG_QUALITY);
}

/** Check if a page exceeds the maximum upload size */
export function isPageTooLarge(sizeBytes: number): boolean {
	return sizeBytes > MAX_PAGE_SIZE_BYTES;
}

/** Format file size for display */
export function formatFileSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// === CAPTURE HELPERS ===

/** Calculate total upload size for all pages */
export function totalUploadSize(pages: CapturedPage[]): number {
	return pages.reduce((sum, p) => sum + p.sizeBytes, 0);
}

/** Generate a default quality check (all ok) for testing */
export function makeGoodQuality(): QualityCheck {
	return {
		aligned: true,
		brightness: 'ok',
		sharpness: 'ok',
		coverage: 'ok',
		skew: 'ok',
		ready: true
	};
}

/** Generate a quality check with specific issues */
export function makeBadQuality(overrides: Partial<QualityCheck> = {}): QualityCheck {
	return {
		aligned: false,
		brightness: 'too_dark',
		sharpness: 'blurry',
		coverage: 'too_far',
		skew: 'tilted',
		ready: false,
		...overrides
	};
}

/** Format upload progress text */
export function uploadProgressText(currentPage: number, totalPages: number, percent: number): string {
	if (totalPages <= 1) {
		return `Sending... ${percent}%`;
	}
	return `Page ${currentPage} of ${totalPages} · ${percent}%`;
}

/** Processing stage human-readable text */
export function processingStageText(stage: import('$lib/types/capture.js').ProcessingStage, pageCount: number): string {
	switch (stage) {
		case 'receiving':
			return `Receiving · ${pageCount} page${pageCount !== 1 ? 's' : ''}`;
		case 'extracting_text':
			return `Extracting text · ${pageCount} page${pageCount !== 1 ? 's' : ''}`;
		case 'analyzing_content':
			return `Analyzing content`;
		case 'storing':
			return `Storing results`;
	}
}
