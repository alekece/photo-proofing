export type GalleryLayout = 'grid' | 'masonry';
export type GallerySort = 'updated' | 'created' | 'title';

export interface Gallery {
	id: string;
	title: string | null;
	layout: GalleryLayout;
	expires_at: string | null;
	included_photos: number | null;
	watermark_enabled: boolean;
	created_at: string;
	updated_at: string;
}

/** Subset of settings sent by a PATCH: a missing key is kept, `null` clears the optional ones. */
export interface GalleryChanges {
	title?: string | null;
	layout?: GalleryLayout;
	expires_at?: string | null;
	included_photos?: number | null;
	watermark_enabled?: boolean;
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
	return (await response.json()) as Gallery;
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
