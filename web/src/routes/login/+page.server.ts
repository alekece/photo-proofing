import { redirect } from '@sveltejs/kit';
import { getAuthStatus } from '#lib/server/auth.js';
import type { PageServerLoad } from './$types';

/** Signed-in visitors skip the form; otherwise the API decides whether it offers setup or sign-in. */
export const load: PageServerLoad = async ({ fetch, request, setHeaders }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const status = await getAuthStatus(fetch, request.headers.get('cookie'));
	if (status.identity) redirect(303, '/dashboard');
	return { setupAvailable: status.setup_available };
};
