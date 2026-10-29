import { env } from 'node:process';
import { error } from '@sveltejs/kit';
import type { AuthStatus } from '#lib/auth.js';

export async function getAuthStatus(
	fetcher: typeof fetch,
	cookie: string | null
): Promise<AuthStatus> {
	try {
		const response = await fetcher(
			`${env.PP_API_INTERNAL_URL ?? 'http://127.0.0.1:8080'}/api/v1/auth/status`,
			{
				headers: cookie ? { Cookie: cookie } : {},
				cache: 'no-store',
				redirect: 'error'
			}
		);
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
