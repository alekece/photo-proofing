import { error, redirect } from '@sveltejs/kit';
import { parseSettings, type WorkspaceSettings } from '#lib/settings/settings.js';
import { apiUrl } from './api';

const UNAVAILABLE = 'Settings are temporarily unavailable. Please try again.';

/** Server-side read with the request cookie; 401 redirects to sign-in, other failures become 503. */
export async function getSettings(
	fetcher: typeof fetch,
	cookie: string | null
): Promise<WorkspaceSettings> {
	let response: Response;

	try {
		response = await fetcher(apiUrl('/api/v1/settings'), {
			headers: cookie ? { Cookie: cookie } : {},
			cache: 'no-store',
			redirect: 'error'
		});
	} catch {
		error(503, UNAVAILABLE);
	}
	if (response.status === 401) redirect(303, '/login');
	if (!response.ok) error(503, UNAVAILABLE);

	const settings = parseSettings(await response.json().catch(() => null));

	if (!settings) error(503, UNAVAILABLE);

	return settings;
}
