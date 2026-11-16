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
import { readSharedJson, readWorkspaceJson } from './http.js';

const UNAVAILABLE = 'Galleries are temporarily unavailable. Please try again.';
const SHARED_UNAVAILABLE = 'This gallery is temporarily unavailable. Please try again.';

/** Server-side read with the request cookie; 401 redirects to sign-in, other failures become 503. */
export function listGalleries(
	fetcher: typeof fetch,
	cookie: string | null,
	sort: GallerySort,
	states: GalleryState[]
): Promise<GalleryOverview[]> {
	const query = new URLSearchParams({ sort });

	for (const state of states) query.append('state', state);

	return readWorkspaceJson(
		fetcher,
		cookie,
		`/api/v1/galleries?${query}`,
		(body) => parseOrNull(v.array(galleryOverviewSchema), body),
		{ unavailable: UNAVAILABLE }
	);
}

/** Unknown, foreign and malformed ids are all a 404. */
export function getGallery(
	fetcher: typeof fetch,
	cookie: string | null,
	id: string
): Promise<Gallery> {
	return readWorkspaceJson(
		fetcher,
		cookie,
		`/api/v1/galleries/${encodeURIComponent(id)}`,
		(body) => parseOrNull(gallerySchema, body),
		{ unavailable: UNAVAILABLE, notFound: 'Gallery not found' }
	);
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
	return readSharedJson(
		fetcher,
		token,
		'',
		sharedGalleryResponseSchema,
		unlockCookie,
		SHARED_UNAVAILABLE
	);
}

/** Lists the photos a client may see, with the same contract as `getSharedGallery`. */
export function getSharedPhotos(
	fetcher: typeof fetch,
	token: string,
	unlockCookie?: string | null
): Promise<SharedPhoto[] | null> {
	return readSharedJson(
		fetcher,
		token,
		'/photos',
		v.array(sharedPhotoSchema),
		unlockCookie,
		SHARED_UNAVAILABLE
	);
}

/** Lists the ids the client selected, with the same contract as `getSharedGallery`. */
export function getSharedFavorites(
	fetcher: typeof fetch,
	token: string,
	unlockCookie?: string | null
): Promise<string[] | null> {
	return readSharedJson(
		fetcher,
		token,
		'/favorites',
		favoriteIdsSchema,
		unlockCookie,
		SHARED_UNAVAILABLE
	);
}
