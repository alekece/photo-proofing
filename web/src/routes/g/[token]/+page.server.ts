import { toProofs } from '#lib/gallery/proofs.js';
import { getSharedGallery, getSharedPhotos } from '#lib/server/galleries.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, fetch, setHeaders }) => {
	// The token is a bearer credential: keep the page out of caches and Referer headers.
	setHeaders({ 'Cache-Control': 'no-store', 'Referrer-Policy': 'no-referrer' });

	const gallery = await getSharedGallery(fetch, params.token);
	const photos = gallery && (await getSharedPhotos(fetch, params.token));

	// The link can expire between the two reads; the client then sees it as unavailable.
	if (!photos) return { gallery: null, proofs: [] };

	return { gallery, proofs: toProofs(params.token, photos) };
};
