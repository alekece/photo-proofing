import { env } from 'node:process';
import { error, redirect } from '@sveltejs/kit';
import type { Gallery, GallerySort } from '#lib/gallery/api.js';
import type { SharedGallery, SharedPhoto } from '#lib/gallery/types.js';
import { everyItem, isGallery, isSharedGallery, isSharedPhoto } from '#lib/gallery/validate.js';

async function readGalleries<T>(
	fetcher: typeof fetch,
	cookie: string | null,
	path: string,
	check: (value: unknown) => value is T
) {
	let response: Response;
	try {
		response = await fetcher(
			`${env.PP_API_INTERNAL_URL ?? 'http://127.0.0.1:8080'}/api/v1/galleries${path}`,
			{
				headers: cookie ? { Cookie: cookie } : {},
				cache: 'no-store',
				redirect: 'error'
			}
		);
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
	if (!check(body)) error(503, 'Galleries are temporarily unavailable. Please try again.');
	return body;
}

/** Server-side read with the request cookie; 401 redirects to sign-in, other failures become 503. */
export function listGalleries(
	fetcher: typeof fetch,
	cookie: string | null,
	sort: GallerySort
): Promise<Gallery[]> {
	return readGalleries<Gallery[]>(fetcher, cookie, `?sort=${sort}`, (value): value is Gallery[] =>
		everyItem(value, isGallery)
	);
}

/** Unknown, foreign and malformed ids are all a 404. */
export function getGallery(
	fetcher: typeof fetch,
	cookie: string | null,
	id: string
): Promise<Gallery> {
	return readGalleries<Gallery>(fetcher, cookie, `/${encodeURIComponent(id)}`, isGallery);
}

async function readShared<T>(
	fetcher: typeof fetch,
	token: string,
	path: string,
	check: (value: unknown) => value is T
) {
	const unavailable = 'This gallery is temporarily unavailable. Please try again.';
	let response: Response;
	try {
		response = await fetcher(
			`${env.PP_API_INTERNAL_URL ?? 'http://127.0.0.1:8080'}/api/v1/galleries/shared/${encodeURIComponent(token)}${path}`,
			{ headers: {}, cache: 'no-store', redirect: 'error' }
		);
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
	if (!check(body)) error(503, unavailable);
	return body;
}

/**
 * Resolves a private link without credentials. A 404 (unknown, malformed or expired link,
 * which the API deliberately does not distinguish) is `null`; everything else is a 503.
 */
export function getSharedGallery(
	fetcher: typeof fetch,
	token: string
): Promise<SharedGallery | null> {
	return readShared<SharedGallery>(fetcher, token, '', isSharedGallery);
}

/** Lists the photos a client may see, with the same contract as `getSharedGallery`. */
export function getSharedPhotos(
	fetcher: typeof fetch,
	token: string
): Promise<SharedPhoto[] | null> {
	return readShared<SharedPhoto[]>(fetcher, token, '/photos', (value): value is SharedPhoto[] =>
		everyItem(value, isSharedPhoto)
	);
}
