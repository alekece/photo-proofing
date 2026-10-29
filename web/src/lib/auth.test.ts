import { describe, expect, it, vi } from 'vitest';
import { AuthError, authRequest, createActivityRenewal } from './auth';

describe('activity renewal', () => {
	it('renews only for activity and coalesces using half short timeouts', async () => {
		let now = 1000;
		const renew = vi.fn().mockResolvedValue(undefined);
		const expired = vi.fn();
		const activity = createActivityRenewal(2000, renew, expired, () => now);
		expect(renew).not.toHaveBeenCalled();
		await activity();
		await activity();
		expect(renew).toHaveBeenCalledTimes(1);
		now += 1000;
		await activity();
		expect(renew).toHaveBeenCalledTimes(2);
		expect(expired).not.toHaveBeenCalled();
	});
	it('redirects for revoked sessions but keeps credentials on transient failures', async () => {
		const expired = vi.fn();
		const renew = vi
			.fn()
			.mockRejectedValueOnce(new AuthError(503, 'unavailable'))
			.mockRejectedValueOnce(new AuthError(401, 'expired'));
		let now = 0;
		const activity = createActivityRenewal(2000, renew, expired, () => now);
		await activity();
		expect(expired).not.toHaveBeenCalled();
		now = 1000;
		await activity();
		expect(expired).toHaveBeenCalledOnce();
	});
});

it('sends same-origin credentials and handles JSON failures', async () => {
	const fetcher = vi.fn().mockResolvedValue(
		new Response(JSON.stringify({ error: 'invalid email or password' }), {
			status: 401
		})
	);
	await expect(
		authRequest('login', { email: 'test@example.test', password: 'password' }, fetcher)
	).rejects.toMatchObject({ status: 401 });
	expect(fetcher).toHaveBeenCalledWith(
		'/api/v1/auth/login',
		expect.objectContaining({ method: 'POST', credentials: 'same-origin' })
	);
});
