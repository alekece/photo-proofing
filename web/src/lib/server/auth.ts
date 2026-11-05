import { error } from '@sveltejs/kit';
import type { AuthStatus } from '#lib/auth.js';
import { apiUrl } from './api';

/**
 * Asks the API who the request's cookie belongs to, from the server only. The cookie goes to
 * the configured internal API URL, never to a browser-supplied target. Any failure or
 * malformed answer becomes a 503, so an outage is never mistaken for "signed out" or "setup open".
 */
export async function getAuthStatus(
	fetcher: typeof fetch,
	cookie: string | null
): Promise<AuthStatus> {
	const url = apiUrl('/api/v1/auth/status');

	try {
		const response = await fetcher(url, {
			headers: cookie ? { Cookie: cookie } : {},
			cache: 'no-store',
			redirect: 'error'
		});
		if (!response.ok) throw new Error('Authentication unavailable');
		const result: AuthStatus = await response.json();
		if (
			typeof result.setup_available !== 'boolean' ||
			!Number.isFinite(result.idle_timeout_ms) ||
			result.idle_timeout_ms < 1000 ||
			(result.identity !== null &&
				(typeof result.identity?.account_id !== 'string' ||
					typeof result.identity?.workspace_id !== 'string'))
		)
			throw new Error('Invalid authentication response');
		return result;
	} catch {
		error(503, 'Sign-in is temporarily unavailable. Please try again.');
	}
}
