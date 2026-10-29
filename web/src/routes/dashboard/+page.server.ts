import { redirect } from '@sveltejs/kit';
import { getAuthStatus } from '#lib/server/auth.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, request, setHeaders }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const status = await getAuthStatus(fetch, request.headers.get('cookie'));
	if (!status.identity) redirect(303, '/login');
	return { identity: status.identity, idleTimeoutMs: status.idle_timeout_ms };
};
