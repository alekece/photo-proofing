import { toProofs } from '#lib/gallery/proofs.js';
import { getSharedFavorites, getSharedPhotos, getSharedGallery } from '#lib/server/galleries.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, fetch, setHeaders }) => {
	// The token is a bearer credential: keep the page out of caches and Referer headers.
	setHeaders({ 'Cache-Control': 'no-store', 'Referrer-Policy': 'no-referrer' });

	const gallery = await getSharedGallery(fetch, params.token);
	const [photos, favorites] = gallery
		? await Promise.all([
				getSharedPhotos(fetch, params.token),
				getSharedFavorites(fetch, params.token)
			])
		: [null, null];

	// The link can expire between the reads; the client then sees it as unavailable.
	if (!gallery || !photos || !favorites) return { gallery: null, proofs: [], favorites: [] };

	return { gallery, proofs: toProofs(params.token, photos), favorites };
};
