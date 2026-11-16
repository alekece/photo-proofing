import { parseSettings, type WorkspaceSettings } from '#lib/settings/settings.js';
import { readWorkspaceJson } from './http.js';

const UNAVAILABLE = 'Settings are temporarily unavailable. Please try again.';

/** Server-side read with the request cookie; 401 redirects to sign-in, other failures become 503. */
export function getSettings(
	fetcher: typeof fetch,
	cookie: string | null
): Promise<WorkspaceSettings> {
	return readWorkspaceJson(fetcher, cookie, '/api/v1/settings', (body) => parseSettings(body), {
		unavailable: UNAVAILABLE
	});
}
