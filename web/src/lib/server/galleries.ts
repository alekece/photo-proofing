import { error, redirect } from '@sveltejs/kit';
import * as v from 'valibot';
import {
	favoriteIdsSchema,
	galleryOverviewSchema,
	gallerySchema,
	parseOrNull,
	sharedGalleryResponseSchema,
	sharedPhotoSchema,
	type Gallery,
	type GalleryOverview,
	type GallerySort,
	type GalleryState,
	type SharedGalleryResponse,
	type SharedPhoto
} from '#lib/gallery/schemas.js';
import { apiUrl } from './api';

const UNLOCK_COOKIE_PATTERN = /^[A-Za-z0-9_-]+$/;

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
): Promise<GalleryOverview[]> {
	const query = new URLSearchParams({ sort });

	for (const state of states) query.append('state', state);

	return readGalleries(fetcher, cookie, `?${query}`, v.array(galleryOverviewSchema));
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
	schema: S,
	unlockCookie?: string | null
) {
	const unavailable = 'This gallery is temporarily unavailable. Please try again.';
	const url = apiUrl(`/api/v1/galleries/shared/${encodeURIComponent(token)}${path}`);
	const headers: HeadersInit =
		unlockCookie && UNLOCK_COOKIE_PATTERN.test(unlockCookie)
			? { Cookie: `pp_gallery_unlock=${unlockCookie}` }
			: {};
	let response: Response;

	try {
		response = await fetcher(url, { headers, cache: 'no-store', redirect: 'error' });
	} catch {
		error(503, unavailable);
	}
	if (response.status === 404 || response.status === 401) return null;
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
 * Resolves a private link, forwarding only `pp_gallery_unlock` when present so unlocked
 * browsers stay unlocked across reloads without exposing photographer session cookies.
 * A 404 (unknown, malformed, archived or expired link) is `null`; everything else is a 503.
 */
export function getSharedGallery(
	fetcher: typeof fetch,
	token: string,
	unlockCookie?: string | null
): Promise<SharedGalleryResponse | null> {
	return readShared(fetcher, token, '', sharedGalleryResponseSchema, unlockCookie);
}

/** Lists the photos a client may see, with the same contract as `getSharedGallery`. */
export function getSharedPhotos(
	fetcher: typeof fetch,
	token: string,
	unlockCookie?: string | null
): Promise<SharedPhoto[] | null> {
	return readShared(fetcher, token, '/photos', v.array(sharedPhotoSchema), unlockCookie);
}

/** Lists the ids the client selected, with the same contract as `getSharedGallery`. */
export function getSharedFavorites(
	fetcher: typeof fetch,
	token: string,
	unlockCookie?: string | null
): Promise<string[] | null> {
	return readShared(fetcher, token, '/favorites', favoriteIdsSchema, unlockCookie);
}
