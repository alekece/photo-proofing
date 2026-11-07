import { GalleryApiError } from './api';
import {
	deletedPhotosSchema,
	parseOrNull,
	photoSchema,
	uploadLimitsSchema,
	type Photo
} from './schemas';
import * as v from 'valibot';

export type { Photo, PhotoStatus, UploadLimits } from './schemas';

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

/** The set a route addresses: the client proofs, or the photographer's finished photos. */
export type PhotoSet = 'photos' | 'finals';

/** The photo routes of a gallery, which address the finals with `?finals=true`. */
function photosPath(
	galleryId: string,
	{ photoId, action, set = 'photos' }: { photoId?: string; action?: string; set?: PhotoSet } = {}
) {
	const segments = [photoId === undefined ? undefined : encodeURIComponent(photoId), action];
	const path = [`/api/v1/galleries/${encodeURIComponent(galleryId)}/photos`, ...segments]
		.filter((segment) => segment !== undefined)
		.join('/');

	return set === 'finals' ? `${path}?finals=true` : path;
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

export async function listPhotos(
	galleryId: string,
	fetcher: typeof fetch = fetch,
	set: PhotoSet = 'photos'
) {
	const response = await request(photosPath(galleryId, { set }), {}, fetcher);

	const photos = parseOrNull(v.array(photoSchema), await response.json().catch(() => null));

	if (!photos) throw unexpected();

	return photos;
}

export async function deletePhoto(
	galleryId: string,
	photoId: string,
	fetcher = fetch,
	set: PhotoSet = 'photos'
) {
	await request(photosPath(galleryId, { photoId, set }), { method: 'DELETE' }, fetcher);
}

/** Most photos one bulk delete request accepts. */
export const BULK_DELETE_LIMIT = 1000;

/** Deletes up to `BULK_DELETE_LIMIT` photos; ids already gone are not counted. */
export async function deletePhotos(
	galleryId: string,
	photoIds: string[],
	fetcher = fetch,
	set: PhotoSet = 'photos'
) {
	const response = await request(
		photosPath(galleryId, { action: 'delete', set }),
		{
			method: 'POST',
			body: JSON.stringify({ photo_ids: photoIds }),
			headers: { 'Content-Type': 'application/json' }
		},
		fetcher
	);

	const body = parseOrNull(deletedPhotosSchema, await response.json().catch(() => null));

	if (!body) throw unexpected();

	return body.deleted;
}

export async function retryPhoto(
	galleryId: string,
	photoId: string,
	fetcher = fetch,
	set: PhotoSet = 'photos'
) {
	await request(
		photosPath(galleryId, { photoId, action: 'retry', set }),
		{ method: 'POST' },
		fetcher
	);
}

export async function getUploadLimits(fetcher: typeof fetch = fetch) {
	const response = await request('/api/v1/upload/limits', {}, fetcher);

	const limits = parseOrNull(uploadLimitsSchema, await response.json().catch(() => null));

	if (!limits) throw unexpected();

	return limits;
}

/** Sends one file; only an abort throws, every server answer becomes an outcome. */
export async function uploadPhoto(
	galleryId: string,
	file: File,
	signal: AbortSignal,
	fetcher: typeof fetch = fetch,
	set: PhotoSet = 'photos'
): Promise<UploadOutcome> {
	let response: Response;

	try {
		response = await fetcher(photosPath(galleryId, { set }), {
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
		const photo = parseOrNull(photoSchema, await response.json().catch(() => null));

		return photo ? { kind: 'created', photo } : { kind: 'failed' };
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

export function thumbnailUrl(galleryId: string, photoId: string, set: PhotoSet = 'photos') {
	return photosPath(galleryId, { photoId, action: 'thumbnail', set });
}

export function previewUrl(galleryId: string, photoId: string, set: PhotoSet = 'photos') {
	return photosPath(galleryId, { photoId, action: 'preview', set });
}
