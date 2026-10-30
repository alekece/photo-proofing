import { getSharedGallery } from '#lib/server/galleries.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, fetch, setHeaders }) => {
	// The token is a bearer credential: keep the page out of caches and Referer headers.
	setHeaders({ 'Cache-Control': 'no-store', 'Referrer-Policy': 'no-referrer' });

	return { gallery: await getSharedGallery(fetch, params.token) };
};
