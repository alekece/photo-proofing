import { gallerySchema, parseOrNull, type Gallery, type GalleryLayout } from './schemas';

export type { Gallery, GalleryLayout, GallerySort } from './schemas';

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
		/** The setting a 422 is about (`context[0]` of the error body). */
		public field?: string
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
		const data = await response.json().catch(() => ({}));
		throw new GalleryApiError(
			response.status,
			typeof data.error === 'string' ? data.error : 'Unable to complete the request.',
			Array.isArray(data.context) && typeof data.context[0] === 'string'
				? data.context[0]
				: undefined
		);
	}
	const gallery = parseOrNull(gallerySchema, await response.json().catch(() => null));

	if (!gallery) throw new GalleryApiError(502, 'Unexpected response from the server.');

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
		throw new GalleryApiError(response.status, 'Unable to delete the gallery.');
	}
}
