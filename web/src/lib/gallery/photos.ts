import { GalleryApiError } from './api';
import { everyItem, isPhoto, isUploadLimits } from './validate';

export type PhotoStatus = 'waiting' | 'processing' | 'ready' | { failed: string };

export interface Photo {
	id: string;
	original_name: string;
	size: number;
	width: number;
	height: number;
	status: PhotoStatus;
	created_at: string;
}

export interface UploadLimits {
	concurrency: number;
	max_size: number;
	max_pixels: number;
}

/** How one upload ended, already translated for the queue. */
export type UploadOutcome =
	| { kind: 'created'; photo: Photo }
	| { kind: 'busy'; retryAfterMs: number }
	| { kind: 'storage-full' }
	| { kind: 'rejected'; reason: string }
	| { kind: 'failed' }
	| { kind: 'gone' }
	| { kind: 'signed-out' };

const REJECTIONS: Record<number, string> = {
	413: 'Too large',
	415: 'Not a JPEG',
	422: 'Unreadable image'
};

function photosPath(galleryId: string, photoId?: string) {
	const base = `/api/v1/galleries/${encodeURIComponent(galleryId)}/photos`;

	return photoId === undefined ? base : `${base}/${encodeURIComponent(photoId)}`;
}

const unexpected = () => new GalleryApiError(502, 'Unexpected response from the server.');

async function request(path: string, init: RequestInit, fetcher: typeof fetch) {
	const response = await fetcher(path, { ...init, credentials: 'same-origin' });

	if (!response.ok) {
		const data = await response.json().catch(() => ({}));

		throw new GalleryApiError(
			response.status,
			typeof data.error === 'string' ? data.error : 'Unable to complete the request.'
		);
	}

	return response;
}

export async function listPhotos(galleryId: string, fetcher: typeof fetch = fetch) {
	const response = await request(photosPath(galleryId), {}, fetcher);

	const photos: unknown = await response.json().catch(() => null);

	if (!everyItem(photos, isPhoto)) throw unexpected();

	return photos;
}

export async function deletePhoto(galleryId: string, photoId: string, fetcher = fetch) {
	await request(photosPath(galleryId, photoId), { method: 'DELETE' }, fetcher);
}

/** Most photos one bulk delete request accepts. */
export const BULK_DELETE_LIMIT = 1000;

/** Deletes up to `BULK_DELETE_LIMIT` photos; ids already gone are not counted. */
export async function deletePhotos(galleryId: string, photoIds: string[], fetcher = fetch) {
	const response = await request(
		`${photosPath(galleryId)}/delete`,
		{
			method: 'POST',
			body: JSON.stringify({ photo_ids: photoIds }),
			headers: { 'Content-Type': 'application/json' }
		},
		fetcher
	);

	const body: unknown = await response.json().catch(() => null);
	const deleted = (body as { deleted?: unknown } | null)?.deleted;

	if (!Number.isInteger(deleted)) throw unexpected();

	return deleted as number;
}

export async function retryPhoto(galleryId: string, photoId: string, fetcher = fetch) {
	await request(`${photosPath(galleryId, photoId)}/retry`, { method: 'POST' }, fetcher);
}

export async function getUploadLimits(fetcher: typeof fetch = fetch) {
	const response = await request('/api/v1/upload/limits', {}, fetcher);

	const limits: unknown = await response.json().catch(() => null);

	if (!isUploadLimits(limits)) throw unexpected();

	return limits;
}

/** Sends one file; only an abort throws, every server answer becomes an outcome. */
export async function uploadPhoto(
	galleryId: string,
	file: File,
	signal: AbortSignal,
	fetcher: typeof fetch = fetch
): Promise<UploadOutcome> {
	let response: Response;

	try {
		response = await fetcher(photosPath(galleryId), {
			method: 'POST',
			body: file,
			signal,
			credentials: 'same-origin',
			headers: { 'Content-Type': 'image/jpeg', 'X-Filename': encodeURIComponent(file.name) }
		});
	} catch (error) {
		if (error instanceof DOMException && error.name === 'AbortError') throw error;

		return { kind: 'failed' };
	}

	if (response.status === 201) {
		const photo: unknown = await response.json().catch(() => null);

		return isPhoto(photo) ? { kind: 'created', photo } : { kind: 'failed' };
	}

	if (response.status === 429) {
		const seconds = Number(response.headers.get('Retry-After'));

		return { kind: 'busy', retryAfterMs: seconds > 0 ? seconds * 1000 : 1000 };
	}

	if (response.status === 507) return { kind: 'storage-full' };

	if (response.status === 404) return { kind: 'gone' };

	if (response.status === 401) return { kind: 'signed-out' };

	const reason = REJECTIONS[response.status];

	return reason === undefined ? { kind: 'failed' } : { kind: 'rejected', reason };
}

export function thumbnailUrl(galleryId: string, photoId: string) {
	return `${photosPath(galleryId, photoId)}/thumbnail`;
}

export function previewUrl(galleryId: string, photoId: string) {
	return `${photosPath(galleryId, photoId)}/preview`;
}
