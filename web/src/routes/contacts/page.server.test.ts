import { error } from '@sveltejs/kit';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const server = vi.hoisted(() => ({
	getAuthStatus: vi.fn(),
	listWorkspaceContacts: vi.fn()
}));

vi.mock('#lib/server/auth.js', () => ({ getAuthStatus: server.getAuthStatus }));
vi.mock('#lib/server/contacts.js', () => ({
	listWorkspaceContacts: server.listWorkspaceContacts
}));

import { load } from './+page.server';

function run() {
	const setHeaders = vi.fn();
	return {
		setHeaders,
		promise: load({
			fetch: vi.fn(),
			request: new Request('http://localhost/contacts', {
				headers: { cookie: 'pp_session=xyz' }
			}),
			setHeaders
		} as unknown as Parameters<typeof load>[0])
	};
}

describe('contacts server load', () => {
	beforeEach(() => {
		server.getAuthStatus.mockResolvedValue({
			identity: { account_id: 'a', workspace_id: 'w' },
			idle_timeout_ms: 120000
		});
		server.listWorkspaceContacts.mockResolvedValue([]);
	});

	it('redirects unauthenticated requests to /login', async () => {
		server.getAuthStatus.mockResolvedValueOnce({ identity: null, idle_timeout_ms: 120000 });

		await expect(run().promise).rejects.toMatchObject({ status: 303, location: '/login' });
	});

	it('loads workspace contacts with Cache-Control: no-store', async () => {
		const { setHeaders, promise } = run();
		const result = await promise;

		expect(setHeaders).toHaveBeenCalledWith({ 'Cache-Control': 'no-store' });
		expect(result).toEqual({
			identity: { account_id: 'a', workspace_id: 'w' },
			idleTimeoutMs: 120000,
			contacts: []
		});
	});

	it('returns contacts: null when the backend is unavailable (503)', async () => {
		server.listWorkspaceContacts.mockImplementationOnce(async () => error(503, 'Unavailable'));

		const result = await run().promise;

		expect(result).toMatchObject({ contacts: null });
	});
});
