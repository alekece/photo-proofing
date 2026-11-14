import { beforeEach, describe, expect, it, vi } from 'vitest';

const server = vi.hoisted(() => ({
	getAuthStatus: vi.fn(),
	getGallery: vi.fn(),
	getSettings: vi.fn()
}));

vi.mock('#lib/server/auth.js', () => ({ getAuthStatus: server.getAuthStatus }));
vi.mock('#lib/server/galleries.js', () => ({ getGallery: server.getGallery }));
vi.mock('#lib/server/settings.js', () => ({ getSettings: server.getSettings }));

import { load } from './+page.server';

function run() {
	return load({
		fetch: vi.fn(),
		params: { id: 'g1' },
		request: new Request('http://localhost/dashboard/g1', {
			headers: { cookie: 'pp_session=abc' }
		}),
		setHeaders: vi.fn()
	} as unknown as Parameters<typeof load>[0]) as Promise<Record<string, unknown>>;
}

describe('gallery studio load', () => {
	beforeEach(() => {
		server.getAuthStatus.mockReset();
		server.getGallery.mockReset();
		server.getSettings.mockReset();
	});

	it('redirects to login without a session', async () => {
		server.getAuthStatus.mockResolvedValue({ identity: null });

		await expect(run()).rejects.toMatchObject({ status: 303, location: '/login' });
	});

	it('loads the gallery alongside workspace settings including emailEnabled', async () => {
		server.getAuthStatus.mockResolvedValue({
			identity: { account_id: 'a', workspace_id: 'w' },
			idle_timeout_ms: 1000
		});
		const gallery = { id: 'g1', title: 'Wedding' };
		const settings = { studioName: 'Atelier', emailEnabled: true };

		server.getGallery.mockResolvedValue(gallery);
		server.getSettings.mockResolvedValue(settings);

		const data = await run();

		expect(server.getGallery).toHaveBeenCalledWith(expect.any(Function), 'pp_session=abc', 'g1');
		expect(server.getSettings).toHaveBeenCalledWith(expect.any(Function), 'pp_session=abc');
		expect(data).toEqual({ gallery, settings, idleTimeoutMs: 1000 });
	});
});
