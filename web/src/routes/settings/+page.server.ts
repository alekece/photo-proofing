import { m } from '#lib/paraglide/messages.js';
import { redirect } from '@sveltejs/kit';
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

	// Only the cover-signature preview still borrows the owner's first cover photo; #293 reworks
	// it and drops this lookup. A list outage just leaves a plain tile.
	const galleries = await listGalleries(fetch, request.headers.get('cookie'), 'updated', []).catch(
		() => []
	);
	const first = galleries.find((gallery) => gallery.hero_id !== null);
	const cover = first
		? {
				image: previewUrl(first.id, first.hero_id!),
				title: first.title ?? m.gallery_untitled()
			}
		: null;

	return { idleTimeoutMs: status.idle_timeout_ms, settings, cover };
};
