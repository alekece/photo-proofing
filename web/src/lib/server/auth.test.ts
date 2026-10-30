import { describe, expect, it, vi } from 'vitest';
import { getAuthStatus } from './auth';

const status = {
	setup_available: false,
	identity: { account_id: 'account', workspace_id: 'workspace' },
	idle_timeout_ms: 2592000000
};

describe('server-side authentication', () => {
	it('forwards cookies to the configured API without caching or following redirects', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify(status)));
		expect(await getAuthStatus(fetcher, 'pp_session=opaque')).toEqual(status);
		expect(fetcher).toHaveBeenCalledWith(
			expect.stringContaining('/api/v1/auth/status'),
			expect.objectContaining({
				headers: { Cookie: 'pp_session=opaque' },
				cache: 'no-store',
				redirect: 'error'
			})
		);
	});
	it('fails closed on service errors and malformed identity responses', async () => {
		for (const response of [
			new Response('unavailable', { status: 503 }),
			new Response('{}'),
			new Response(JSON.stringify({ ...status, identity: {} }))
		]) {
			await expect(getAuthStatus(vi.fn().mockResolvedValue(response), null)).rejects.toMatchObject({
				status: 503
			});
		}
	});
});
