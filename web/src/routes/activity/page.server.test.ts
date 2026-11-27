import { error } from '@sveltejs/kit';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const server = vi.hoisted(() => ({
	getAuthStatus: vi.fn(),
	getSettings: vi.fn()
}));

vi.mock('#lib/server/auth.js', () => ({ getAuthStatus: server.getAuthStatus }));
vi.mock('#lib/server/settings.js', () => ({ getSettings: server.getSettings }));

import { load } from './+page.server';

function run() {
	const setHeaders = vi.fn();

	return {
		setHeaders,
		promise: load({
			fetch: vi.fn(),
			request: new Request('http://localhost/activity', { headers: { cookie: 'pp_session=xyz' } }),
			setHeaders
		} as unknown as Parameters<typeof load>[0])
	};
}

describe('activity server load', () => {
	beforeEach(() => {
		server.getAuthStatus.mockResolvedValue({
			identity: { account_id: 'a', workspace_id: 'w' },
			idle_timeout_ms: 120000
		});
		server.getSettings.mockResolvedValue({ studioName: 'Atelier Lumiere' });
	});

	it('redirects unauthenticated requests to /login', async () => {
		server.getAuthStatus.mockResolvedValueOnce({ identity: null, idle_timeout_ms: 120000 });

		await expect(run().promise).rejects.toMatchObject({ status: 303, location: '/login' });
	});

	it('loads the workspace settings with Cache-Control: no-store', async () => {
		const { setHeaders, promise } = run();

		expect(await promise).toEqual({
			identity: { account_id: 'a', workspace_id: 'w' },
			idleTimeoutMs: 120000,
			settings: { studioName: 'Atelier Lumiere' }
		});
		expect(setHeaders).toHaveBeenCalledWith({ 'Cache-Control': 'no-store' });
	});

	it('still opens when workspace settings are unavailable (503)', async () => {
		server.getSettings.mockImplementationOnce(async () => error(503, 'Unavailable'));

		expect(await run().promise).toMatchObject({ settings: null });
	});
});
