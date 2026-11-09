import { redirect } from '@sveltejs/kit';
import { getAuthStatus } from '#lib/server/auth.js';
import { getSettings } from '#lib/server/settings.js';
import type { PageServerLoad } from './$types';

/** Guards the page like the dashboard and loads the saved settings. */
export const load: PageServerLoad = async ({ fetch, request, setHeaders }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const status = await getAuthStatus(fetch, request.headers.get('cookie'));

	if (!status.identity) redirect(303, '/login');

	const settings = await getSettings(fetch, request.headers.get('cookie'));

	return { idleTimeoutMs: status.idle_timeout_ms, settings };
};
