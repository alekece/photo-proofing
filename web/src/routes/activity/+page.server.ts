import { isHttpError, redirect } from '@sveltejs/kit';
import { getAuthStatus } from '#lib/server/auth.js';
import { listWorkspaceContacts } from '#lib/server/contacts.js';
import { listGalleries } from '#lib/server/galleries.js';
import { getSettings } from '#lib/server/settings.js';
import type { PageServerLoad } from './$types';

/** Guards the activity page on the server and loads the choices its filters offer. */
export const load: PageServerLoad = async ({ fetch, request, setHeaders }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const cookie = request.headers.get('cookie');
	const status = await getAuthStatus(fetch, cookie);

	if (!status.identity) redirect(303, '/login');

	// The filters are a convenience: an outage leaves them empty rather than blocking the log.
	const orEmpty = (error: unknown): never[] => {
		if (isHttpError(error) && error.status === 503) return [];

		throw error;
	};
	const [galleries, contacts, settings] = await Promise.all([
		listGalleries(fetch, cookie, 'updated', []).catch(orEmpty),
		listWorkspaceContacts(fetch, cookie).catch(orEmpty),
		getSettings(fetch, cookie).catch((error) => {
			if (isHttpError(error) && error.status === 503) return null;

			throw error;
		})
	]);

	return {
		identity: status.identity,
		idleTimeoutMs: status.idle_timeout_ms,
		galleries,
		contacts,
		settings
	};
};
