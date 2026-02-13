// IMP-011: CapacitorCamera — wraps @capacitor/camera for document photography
import { Camera, CameraResultType, CameraSource } from '@capacitor/camera';

/** Result of a camera capture */
export interface CaptureResult {
	dataUrl: string;
	format: 'jpeg' | 'png';
}

/**
 * Take a photo using the device camera.
 * Returns the image as a base64 data URL.
 */
export async function takePhoto(): Promise<CaptureResult> {
	const image = await Camera.getPhoto({
		quality: 90,
		allowEditing: false,
		resultType: CameraResultType.DataUrl,
		source: CameraSource.Camera,
		correctOrientation: true,
		width: 2048,
		height: 2048,
	});

	if (!image.dataUrl) {
		throw new Error('Camera did not return an image');
	}

	return {
		dataUrl: image.dataUrl,
		format: image.format === 'png' ? 'png' : 'jpeg',
	};
}

/**
 * Pick a photo from the device gallery.
 * Returns the image as a base64 data URL.
 */
export async function pickFromGallery(): Promise<CaptureResult> {
	const image = await Camera.getPhoto({
		quality: 90,
		allowEditing: false,
		resultType: CameraResultType.DataUrl,
		source: CameraSource.Photos,
		correctOrientation: true,
		width: 2048,
		height: 2048,
	});

	if (!image.dataUrl) {
		throw new Error('Gallery did not return an image');
	}

	return {
		dataUrl: image.dataUrl,
		format: image.format === 'png' ? 'png' : 'jpeg',
	};
}

/** Check if the camera is available */
export async function isCameraAvailable(): Promise<boolean> {
	try {
		const perms = await Camera.checkPermissions();
		return perms.camera !== 'denied';
	} catch {
		return false;
	}
}

/** Request camera permission */
export async function requestCameraPermission(): Promise<boolean> {
	try {
		const result = await Camera.requestPermissions({ permissions: ['camera'] });
		return result.camera === 'granted';
	} catch {
		return false;
	}
}
