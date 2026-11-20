import { error } from '@sveltejs/kit';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const server = vi.hoisted(() => ({
	getAuthStatus: vi.fn(),
	listGalleries: vi.fn(),
	listWorkspaceContacts: vi.fn(),
	getSettings: vi.fn()
}));

vi.mock('#lib/server/auth.js', () => ({ getAuthStatus: server.getAuthStatus }));
vi.mock('#lib/server/galleries.js', () => ({ listGalleries: server.listGalleries }));
vi.mock('#lib/server/contacts.js', () => ({
	listWorkspaceContacts: server.listWorkspaceContacts
}));
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
		server.listGalleries.mockResolvedValue([{ id: 'g1', title: 'Wedding' }]);
		server.listWorkspaceContacts.mockResolvedValue([{ id: 'c1', name: 'Alice' }]);
		server.getSettings.mockResolvedValue({ studioName: 'Atelier Lumiere' });
	});

	it('redirects unauthenticated requests to /login', async () => {
		server.getAuthStatus.mockResolvedValueOnce({ identity: null, idle_timeout_ms: 120000 });

		await expect(run().promise).rejects.toMatchObject({ status: 303, location: '/login' });
	});

	it('loads the filter choices with Cache-Control: no-store', async () => {
		const { setHeaders, promise } = run();

		expect(await promise).toEqual({
			identity: { account_id: 'a', workspace_id: 'w' },
			idleTimeoutMs: 120000,
			galleries: [{ id: 'g1', title: 'Wedding' }],
			contacts: [{ id: 'c1', name: 'Alice' }],
			settings: { studioName: 'Atelier Lumiere' }
		});
		expect(setHeaders).toHaveBeenCalledWith({ 'Cache-Control': 'no-store' });
	});

	it('still opens when the filter choices are unavailable (503)', async () => {
		server.listGalleries.mockImplementationOnce(async () => error(503, 'Unavailable'));
		server.listWorkspaceContacts.mockImplementationOnce(async () => error(503, 'Unavailable'));

		expect(await run().promise).toMatchObject({ galleries: [], contacts: [] });
	});
});
