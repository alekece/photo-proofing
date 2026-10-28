import { loadClientGallery } from '#lib/gallery/load.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params }) => ({
	view: await loadClientGallery(params.gallery)
});
