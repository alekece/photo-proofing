import { error, redirect } from '@sveltejs/kit';
import * as v from 'valibot';
import {
	gallerySchema,
	parseOrNull,
	sharedGallerySchema,
	sharedPhotoSchema,
	type Gallery,
	type GallerySort,
	type GalleryState,
	type SharedGallery,
	type SharedPhoto
} from '#lib/gallery/schemas.js';
import { apiUrl } from './api';

async function readGalleries<const S extends v.GenericSchema>(
	fetcher: typeof fetch,
	cookie: string | null,
	path: string,
	schema: S
) {
	const url = apiUrl(`/api/v1/galleries${path}`);
	let response: Response;

	try {
		response = await fetcher(url, {
			headers: cookie ? { Cookie: cookie } : {},
			cache: 'no-store',
			redirect: 'error'
		});
	} catch {
		error(503, 'Galleries are temporarily unavailable. Please try again.');
	}
	if (response.status === 401) redirect(303, '/login');
	if (response.status === 404) error(404, 'Gallery not found');
	if (!response.ok) error(503, 'Galleries are temporarily unavailable. Please try again.');
	let body: unknown;
	try {
		body = await response.json();
	} catch {
		error(503, 'Galleries are temporarily unavailable. Please try again.');
	}
	const parsed = parseOrNull(schema, body);
	if (parsed === null) error(503, 'Galleries are temporarily unavailable. Please try again.');
	return parsed;
}

/** Server-side read with the request cookie; 401 redirects to sign-in, other failures become 503. */
export function listGalleries(
	fetcher: typeof fetch,
	cookie: string | null,
	sort: GallerySort,
	states: GalleryState[]
): Promise<Gallery[]> {
	const query = new URLSearchParams({ sort });

	for (const state of states) query.append('state', state);

	return readGalleries(fetcher, cookie, `?${query}`, v.array(gallerySchema));
}

/** Unknown, foreign and malformed ids are all a 404. */
export function getGallery(
	fetcher: typeof fetch,
	cookie: string | null,
	id: string
): Promise<Gallery> {
	return readGalleries(fetcher, cookie, `/${encodeURIComponent(id)}`, gallerySchema);
}

async function readShared<const S extends v.GenericSchema>(
	fetcher: typeof fetch,
	token: string,
	path: string,
	schema: S
) {
	const unavailable = 'This gallery is temporarily unavailable. Please try again.';
	const url = apiUrl(`/api/v1/galleries/shared/${encodeURIComponent(token)}${path}`);
	let response: Response;

	try {
		response = await fetcher(url, { headers: {}, cache: 'no-store', redirect: 'error' });
	} catch {
		error(503, unavailable);
	}
	if (response.status === 404) return null;
	if (!response.ok) error(503, unavailable);
	let body: unknown;
	try {
		body = await response.json();
	} catch {
		error(503, unavailable);
	}
	const parsed = parseOrNull(schema, body);
	if (parsed === null) error(503, unavailable);
	return parsed;
}

/**
 * Resolves a private link without credentials. A 404 (unknown, malformed or expired link,
 * which the API deliberately does not distinguish) is `null`; everything else is a 503.
 */
export function getSharedGallery(
	fetcher: typeof fetch,
	token: string
): Promise<SharedGallery | null> {
	return readShared(fetcher, token, '', sharedGallerySchema);
}

/** Lists the photos a client may see, with the same contract as `getSharedGallery`. */
export function getSharedPhotos(
	fetcher: typeof fetch,
	token: string
): Promise<SharedPhoto[] | null> {
	return readShared(fetcher, token, '/photos', v.array(sharedPhotoSchema));
}
