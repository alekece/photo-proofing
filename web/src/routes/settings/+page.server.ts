import { redirect } from '@sveltejs/kit';
import { getAuthStatus } from '#lib/server/auth.js';
import { DEFAULT_SETTINGS } from '#lib/settings/settings.js';
import type { PageServerLoad } from './$types';

/** Guards the page like the dashboard; settings come from the defaults until #67 stores them. */
export const load: PageServerLoad = async ({ fetch, request, setHeaders }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const status = await getAuthStatus(fetch, request.headers.get('cookie'));

	if (!status.identity) redirect(303, '/login');

	return { idleTimeoutMs: status.idle_timeout_ms, settings: DEFAULT_SETTINGS };
};
