import { redirect } from '@sveltejs/kit';
import { getAuthStatus } from '#lib/server/auth.js';
import { getGallery } from '#lib/server/galleries.js';
import { getSettings } from '#lib/server/settings.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, params, request, setHeaders }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const cookie = request.headers.get('cookie');
	const status = await getAuthStatus(fetch, cookie);
	if (!status.identity) redirect(303, '/login');
	const [gallery, settings] = await Promise.all([
		getGallery(fetch, cookie, params.id),
		getSettings(fetch, cookie)
	]);
	return { gallery, settings, idleTimeoutMs: status.idle_timeout_ms };
};
