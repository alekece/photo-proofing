import { beforeEach, describe, expect, it, vi } from 'vitest';

const server = vi.hoisted(() => ({
	getAuthStatus: vi.fn(),
	listGalleries: vi.fn()
}));

vi.mock('#lib/server/auth.js', () => ({ getAuthStatus: server.getAuthStatus }));
vi.mock('#lib/server/galleries.js', () => ({ listGalleries: server.listGalleries }));

import { load } from './+page.server';

function run(query: string) {
	return load({
		fetch: vi.fn(),
		request: new Request('http://localhost/dashboard'),
		setHeaders: vi.fn(),
		url: new URL(`http://localhost/dashboard${query}`)
	} as unknown as Parameters<typeof load>[0]) as Promise<{ sort: string }>;
}

describe('dashboard load', () => {
	beforeEach(() => {
		server.getAuthStatus.mockResolvedValue({
			identity: { account_id: 'a', workspace_id: 'w' },
			idle_timeout_ms: 1000
		});
		server.listGalleries.mockResolvedValue([]);
	});

	it.each([
		['?sort=title', 'title'],
		['?sort=created', 'created'],
		['?sort=bogus', 'updated'],
		['?sort=', 'updated'],
		['', 'updated']
	])('reads sort %j as %s', async (query, expected) => {
		expect((await run(query)).sort).toBe(expected);
		expect(server.listGalleries).toHaveBeenLastCalledWith(expect.anything(), null, expected, []);
	});

	it('always loads every gallery; the page filters by status itself', async () => {
		await run('?state=shared&sort=title');

		expect(server.listGalleries).toHaveBeenLastCalledWith(expect.anything(), null, 'title', []);
	});
});
