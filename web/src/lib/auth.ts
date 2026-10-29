export interface Identity {
	account_id: string;
	workspace_id: string;
}
export interface AuthStatus {
	setup_available: boolean;
	identity: Identity | null;
	idle_timeout_ms: number;
}

export class AuthError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
	}
}

export async function authRequest(
	action: 'setup' | 'login' | 'logout' | 'renew',
	input?: { email: string; password: string },
	fetcher: typeof fetch = fetch
): Promise<void> {
	const response = await fetcher(`/api/v1/auth/${action}`, {
		method: 'POST',
		credentials: 'same-origin',
		headers: { 'Content-Type': 'application/json' },
		body: input ? JSON.stringify(input) : undefined
	});
	if (!response.ok) {
		const data = await response.json().catch(() => ({}));
		throw new AuthError(
			response.status,
			typeof data.error === 'string'
				? data.error
				: 'Unable to complete the request. Please try again.'
		);
	}
}

/** Only callers handling user activity invoke this; no background timer renews a session. */
export function createActivityRenewal(
	idleTimeoutMs: number,
	renew: () => Promise<void>,
	onExpired: () => void,
	now: () => number = Date.now
) {
	const interval = Math.min(5 * 60 * 1000, idleTimeoutMs / 2);
	let lastAttempt = -Infinity;
	let pending = false;
	let expired = false;
	return async () => {
		if (expired || pending || now() - lastAttempt < interval) return;
		lastAttempt = now();
		pending = true;
		try {
			await renew();
		} catch (error) {
			if (error instanceof AuthError && error.status === 401) {
				expired = true;
				onExpired();
			}
		} finally {
			pending = false;
		}
	};
}
