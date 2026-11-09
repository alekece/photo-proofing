import { gallerySchema, parseOrNull, type Gallery, type GalleryLayout } from './schemas';
import { apiErrorMessage, readResponseErrorCode } from '#lib/i18n/errors.js';

export type { Gallery, GalleryLayout, GalleryOverview, GallerySort } from './schemas';

/** Subset of settings sent by a PATCH: a missing key is kept, `null` clears the optional ones. */
export interface GalleryChanges {
	title?: string | null;
	layout?: GalleryLayout;
	expires_at?: string | null;
	included_photos?: number | null;
	watermark_enabled?: boolean;
	hero_id?: string | null;
}

export class GalleryApiError extends Error {
	constructor(
		public status: number,
		message: string,
		/** The API's error `code`, when it sent one. */
		public code?: string
	) {
		super(message);
	}
}

async function galleryRequest(path: string, init: RequestInit, fetcher: typeof fetch) {
	const response = await fetcher(`/api/v1/galleries${path}`, {
		...init,
		credentials: 'same-origin',
		headers: { 'Content-Type': 'application/json' }
	});
	if (!response.ok) {
		const code = await readResponseErrorCode(response);

		throw new GalleryApiError(response.status, apiErrorMessage(response.status, code), code);
	}
	const gallery = parseOrNull(gallerySchema, await response.json().catch(() => null));

	if (!gallery) throw new GalleryApiError(502, apiErrorMessage(502));

	return gallery;
}

export function createGallery(fetcher: typeof fetch = fetch): Promise<Gallery> {
	return galleryRequest('', { method: 'POST' }, fetcher);
}

export function updateGallery(
	id: string,
	changes: GalleryChanges,
	fetcher: typeof fetch = fetch
): Promise<Gallery> {
	return galleryRequest(
		`/${encodeURIComponent(id)}`,
		{ method: 'PATCH', body: JSON.stringify(changes), keepalive: true },
		fetcher
	);
}

/** Creates the private link on first call and returns the same one afterwards. */
export function shareGallery(id: string, fetcher: typeof fetch = fetch): Promise<Gallery> {
	return galleryRequest(`/${encodeURIComponent(id)}/share`, { method: 'POST' }, fetcher);
}

/** Switches the private link to the finished photos; delivering twice changes nothing. */
export function deliverGallery(id: string, fetcher: typeof fetch = fetch): Promise<Gallery> {
	return galleryRequest(`/${encodeURIComponent(id)}/deliver`, { method: 'POST' }, fetcher);
}

/** Hides the gallery from its client; the gallery and its photos are kept. */
export function archiveGallery(id: string, fetcher: typeof fetch = fetch): Promise<Gallery> {
	return galleryRequest(`/${encodeURIComponent(id)}/archive`, { method: 'POST' }, fetcher);
}

/** Makes an archived gallery reachable by its client again. */
export function restoreGallery(id: string, fetcher: typeof fetch = fetch): Promise<Gallery> {
	return galleryRequest(`/${encodeURIComponent(id)}/restore`, { method: 'POST' }, fetcher);
}

/** Deletes the gallery and its photos for good; there is no response body to read. */
export async function deleteGallery(id: string, fetcher: typeof fetch = fetch): Promise<void> {
	const response = await fetcher(`/api/v1/galleries/${encodeURIComponent(id)}`, {
		method: 'DELETE',
		credentials: 'same-origin'
	});

	if (!response.ok) {
		const code = await readResponseErrorCode(response);

		throw new GalleryApiError(response.status, apiErrorMessage(response.status, code), code);
	}
}
