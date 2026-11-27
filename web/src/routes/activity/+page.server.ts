import { isHttpError, redirect } from '@sveltejs/kit';
import { getAuthStatus } from '#lib/server/auth.js';
import { getSettings } from '#lib/server/settings.js';
import type { PageServerLoad } from './$types';

/** Guards the activity page on the server and loads the workspace settings. */
export const load: PageServerLoad = async ({ fetch, request, setHeaders }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const cookie = request.headers.get('cookie');
	const status = await getAuthStatus(fetch, cookie);

	if (!status.identity) redirect(303, '/login');

	const settings = await getSettings(fetch, cookie).catch((error) => {
		if (isHttpError(error) && error.status === 503) return null;

		throw error;
	});

	return {
		identity: status.identity,
		idleTimeoutMs: status.idle_timeout_ms,
		settings
	};
};
