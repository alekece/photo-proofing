import { toProofs } from '#lib/gallery/proofs.js';
import { getSharedFavorites, getSharedPhotos, getSharedGallery } from '#lib/server/galleries.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, fetch, cookies, setHeaders }) => {
	// The token is a bearer credential: keep the page out of caches and Referer headers.
	setHeaders({ 'Cache-Control': 'no-store', 'Referrer-Policy': 'no-referrer' });

	const unlockCookie = cookies.get('pp_gallery_unlock');
	const shared = await getSharedGallery(fetch, params.token, unlockCookie);

	if (!shared) {
		return { gallery: null, lockedGallery: null, proofs: [], favorites: [] };
	}

	if (shared.locked === true) {
		return {
			gallery: null,
			lockedGallery: shared,
			proofs: [],
			favorites: [],
			coverUrl: shared.has_cover
				? `/api/v1/galleries/shared/${encodeURIComponent(params.token)}/cover`
				: undefined
		};
	}

	const [photos, favorites] = await Promise.all([
		getSharedPhotos(fetch, params.token, unlockCookie),
		getSharedFavorites(fetch, params.token, unlockCookie)
	]);

	// The link can expire between the reads; the client then sees it as unavailable.
	if (!photos || !favorites) {
		return { gallery: null, lockedGallery: null, proofs: [], favorites: [] };
	}

	return {
		gallery: shared,
		lockedGallery: null,
		proofs: toProofs(params.token, photos, shared.delivered),
		favorites,
		coverUrl: shared.hero_id
			? `/api/v1/galleries/shared/${encodeURIComponent(params.token)}/cover?hero=${encodeURIComponent(shared.hero_id)}`
			: undefined
	};
};
