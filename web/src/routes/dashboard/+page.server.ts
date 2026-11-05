import { isHttpError, redirect } from '@sveltejs/kit';
import { parseGallerySort } from '#lib/gallery/validate.js';
import { getAuthStatus } from '#lib/server/auth.js';
import { listGalleries } from '#lib/server/galleries.js';
import type { PageServerLoad } from './$types';

/** Guards the page on the server: an invalid session redirects before any dashboard HTML is sent. */
export const load: PageServerLoad = async ({ fetch, request, setHeaders, url }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const cookie = request.headers.get('cookie');
	const status = await getAuthStatus(fetch, cookie);
	if (!status.identity) redirect(303, '/login');
	const sort = parseGallerySort(url.searchParams.get('sort'));
	// A list outage keeps the page (and sign-out) usable; `null` renders the error state.
	const galleries = await listGalleries(fetch, cookie, sort).catch((error) => {
		if (isHttpError(error) && error.status === 503) return null;
		throw error;
	});
	return { identity: status.identity, idleTimeoutMs: status.idle_timeout_ms, galleries, sort };
};
