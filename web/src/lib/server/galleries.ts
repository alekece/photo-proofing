import { env } from 'node:process';
import { error, redirect } from '@sveltejs/kit';
import type { Gallery, GallerySort } from '#lib/gallery/api.js';
import type { SharedGallery } from '#lib/gallery/types.js';

async function readGalleries<T>(fetcher: typeof fetch, cookie: string | null, path: string) {
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
	try {
		return (await response.json()) as T;
	} catch {
		error(503, 'Galleries are temporarily unavailable. Please try again.');
	}
}

/** Server-side read with the request cookie; 401 redirects to sign-in, other failures become 503. */
export function listGalleries(
	fetcher: typeof fetch,
	cookie: string | null,
	sort: GallerySort
): Promise<Gallery[]> {
	return readGalleries<Gallery[]>(fetcher, cookie, `?sort=${sort}`);
}

/** Unknown, foreign and malformed ids are all a 404. */
export function getGallery(
	fetcher: typeof fetch,
	cookie: string | null,
	id: string
): Promise<Gallery> {
	return readGalleries<Gallery>(fetcher, cookie, `/${encodeURIComponent(id)}`);
}

/**
 * Resolves a private link without credentials. A 404 (unknown, malformed or expired link,
 * which the API deliberately does not distinguish) is `null`; everything else is a 503.
 */
export async function getSharedGallery(
	fetcher: typeof fetch,
	token: string
): Promise<SharedGallery | null> {
	const unavailable = 'This gallery is temporarily unavailable. Please try again.';
	let response: Response;
	try {
		response = await fetcher(
			`${env.PP_API_INTERNAL_URL ?? 'http://127.0.0.1:8080'}/api/v1/shared/${encodeURIComponent(token)}`,
			{ headers: {}, cache: 'no-store', redirect: 'error' }
		);
	} catch {
		error(503, unavailable);
	}
	if (response.status === 404) return null;
	if (!response.ok) error(503, unavailable);
	try {
		return (await response.json()) as SharedGallery;
	} catch {
		error(503, unavailable);
	}
}
