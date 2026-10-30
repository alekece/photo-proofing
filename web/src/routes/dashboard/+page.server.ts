import { redirect } from '@sveltejs/kit';
import { getAuthStatus } from '#lib/server/auth.js';
import type { PageServerLoad } from './$types';

/** Guards the page on the server: an invalid session redirects before any dashboard HTML is sent. */
export const load: PageServerLoad = async ({ fetch, request, setHeaders }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const status = await getAuthStatus(fetch, request.headers.get('cookie'));
	if (!status.identity) redirect(303, '/login');
	return { identity: status.identity, idleTimeoutMs: status.idle_timeout_ms };
};
