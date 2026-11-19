import { m } from '#lib/paraglide/messages.js';
import {
	deletedPhotosSchema,
	parseOrNull,
	photoSchema,
	sharedPhotoSchema,
	uploadLimitsSchema,
	workspacePhotoSchema,
	type Photo
} from './schemas';
import * as v from 'valibot';
import { ApiError, requestEmpty, requestJson } from '#lib/http.js';
import { startDownload } from './export.js';

export type { Photo, PhotoStatus, UploadLimits, WorkspacePhoto } from './schemas';

/** How one upload ended, already translated for the queue. */
export type UploadOutcome =
	| { kind: 'created'; photo: Photo }
	| { kind: 'busy'; retryAfterMs: number }
	| { kind: 'storage-full' }
	| { kind: 'rejected'; reason: string }
	| { kind: 'failed' }
	| { kind: 'gone' }
	| { kind: 'signed-out' };

const REJECTIONS: Record<number, () => string> = {
	413: m.upload_too_large,
	415: m.upload_not_jpeg,
	422: m.upload_unreadable
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

export function listPhotos(
	galleryId: string,
	fetcher: typeof fetch = fetch,
	set: PhotoSet = 'photos'
) {
	return requestJson(photosPath(galleryId, { set }), {}, v.array(photoSchema), fetcher);
}

/** Every ready proof or final photo across the workspace's galleries, newest first. */
export function listWorkspacePhotos(fetcher: typeof fetch = fetch) {
	return requestJson('/api/v1/photos', {}, v.array(workspacePhotoSchema), fetcher);
}

/** Clean files remain usable while marked copies are being regenerated. */
export function hasCleanPhoto(photo: Photo): boolean {
	return photo.clean_ready ?? photo.status === 'ready';
}

export type PhotoReadiness = {
	ready: Photo[];
	readyCount: number;
	uploadingCount: number;
	failedCount: number;
	empty: boolean;
	warned: boolean;
};

/** Summarizes ready, uploading, and failed photo counts for share and delivery prechecks. */
export function summarizePhotoReadiness(
	photos: readonly Photo[],
	inFlightUploads = 0
): PhotoReadiness {
	const ready: Photo[] = [];
	let uploadingCount = inFlightUploads;
	let failedCount = 0;

	for (const photo of photos) {
		if (photo.status === 'ready') {
			ready.push(photo);
		} else if (photo.status === 'waiting' || photo.status === 'processing') {
			uploadingCount += 1;
		} else if (typeof photo.status === 'object') {
			failedCount += 1;
		}
	}

	const empty = ready.length === 0;
	return {
		ready,
		readyCount: ready.length,
		uploadingCount,
		failedCount,
		empty,
		warned: empty || uploadingCount > 0 || failedCount > 0
	};
}

/** The proofs ready in their marked or clean files, as the client gallery would show them. */
export function listClientPreviewPhotos(
	galleryId: string,
	watermarked: boolean,
	fetcher: typeof fetch = fetch
) {
	return requestJson(
		`/api/v1/galleries/${encodeURIComponent(galleryId)}/photos?watermark=${watermarked}`,
		{},
		v.array(sharedPhotoSchema),
		fetcher
	);
}

export function deletePhoto(
	galleryId: string,
	photoId: string,
	fetcher = fetch,
	set: PhotoSet = 'photos'
) {
	return requestEmpty(photosPath(galleryId, { photoId, set }), { method: 'DELETE' }, fetcher);
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
	const body = await requestJson(
		photosPath(galleryId, { action: 'delete', set }),
		{
			method: 'POST',
			body: JSON.stringify({ photo_ids: photoIds })
		},
		deletedPhotosSchema,
		fetcher
	);

	return body.deleted;
}

export function retryPhoto(
	galleryId: string,
	photoId: string,
	fetcher = fetch,
	set: PhotoSet = 'photos'
) {
	return requestEmpty(
		photosPath(galleryId, { photoId, action: 'retry', set }),
		{ method: 'POST' },
		fetcher
	);
}

export function getUploadLimits(fetcher: typeof fetch = fetch) {
	return requestJson('/api/v1/upload/limits', {}, uploadLimitsSchema, fetcher);
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

	return reason === undefined ? { kind: 'failed' } : { kind: 'rejected', reason: reason() };
}

export function thumbnailUrl(
	galleryId: string,
	photoId: string,
	set: PhotoSet = 'photos',
	revision?: number
) {
	const path = photosPath(galleryId, { photoId, action: 'thumbnail', set });
	return revision === undefined
		? path
		: `${path}${set === 'finals' ? '&' : '?'}revision=${revision}`;
}

export function previewUrl(
	galleryId: string,
	photoId: string,
	set: PhotoSet = 'photos',
	revision?: number
) {
	const path = photosPath(galleryId, { photoId, action: 'preview', set });
	return revision === undefined
		? path
		: `${path}${set === 'finals' ? '&' : '?'}revision=${revision}`;
}

/**
 * Downloads one unchanged original through the authenticated photographer route: a `HEAD`
 * check first (so a signed-out or gone photo fails before the browser opens a save dialog),
 * then the same URL as a real download. Throws `ApiError` when the check fails.
 */
export async function downloadOriginal(
	galleryId: string,
	photoId: string,
	fetcher: typeof fetch = fetch,
	set: PhotoSet = 'photos'
) {
	const url = photosPath(galleryId, { photoId, action: 'download', set });
	const response = await fetcher(url, { method: 'HEAD', credentials: 'same-origin' });

	if (!response.ok) throw new ApiError(response.status, m.favorites_download_failed());

	startDownload(url);
}
