import { isHttpError, redirect } from '@sveltejs/kit';
import { getAuthStatus } from '#lib/server/auth.js';
import { listWorkspacePhotos } from '#lib/server/photos.js';
import { getSettings } from '#lib/server/settings.js';
import type { PageServerLoad } from './$types';

/** Guards the All Photos workspace page on the server and loads every ready photo. */
export const load: PageServerLoad = async ({ fetch, request, setHeaders }) => {
	setHeaders({ 'Cache-Control': 'no-store' });
	const cookie = request.headers.get('cookie');
	const status = await getAuthStatus(fetch, cookie);

	if (!status.identity) redirect(303, '/login');

	const [photos, settings] = await Promise.all([
		listWorkspacePhotos(fetch, cookie).catch((error) => {
			if (isHttpError(error) && error.status === 503) return null;

			throw error;
		}),
		getSettings(fetch, cookie).catch((error) => {
			if (isHttpError(error) && error.status === 503) return null;

			throw error;
		})
	]);

	return {
		identity: status.identity,
		idleTimeoutMs: status.idle_timeout_ms,
		photos,
		settings
	};
};
