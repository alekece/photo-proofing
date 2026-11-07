import { redirect } from '@sveltejs/kit';
import { UNTITLED_GALLERY } from '#lib/gallery/format.js';
import { previewUrl } from '#lib/gallery/photos.js';
import { getAuthStatus } from '#lib/server/auth.js';
import { listGalleries } from '#lib/server/galleries.js';
import { getSettings } from '#lib/server/settings.js';
import type { PageServerLoad } from './$types';

/** Guards the page like the dashboard and loads the saved settings. */
export const load: PageServerLoad = async ({ fetch, request, setHeaders }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const status = await getAuthStatus(fetch, request.headers.get('cookie'));

	if (!status.identity) redirect(303, '/login');

	const settings = await getSettings(fetch, request.headers.get('cookie'));

	// The previews borrow the owner's own cover photos; a list outage just leaves plain tiles.
	const galleries = await listGalleries(fetch, request.headers.get('cookie'), 'updated', []).catch(
		() => []
	);
	const covered = galleries.filter((gallery) => gallery.hero_id !== null);
	const samples = covered.map((gallery) => previewUrl(gallery.id, gallery.hero_id!));
	const cover = covered.length
		? { image: samples[0], title: covered[0].title ?? UNTITLED_GALLERY }
		: null;

	return { idleTimeoutMs: status.idle_timeout_ms, settings, samples, cover };
};
