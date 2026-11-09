import { beforeEach, describe, expect, it, vi } from 'vitest';

const server = vi.hoisted(() => ({
	getAuthStatus: vi.fn(),
	getSettings: vi.fn(),
	listGalleries: vi.fn()
}));

vi.mock('#lib/server/auth.js', () => ({ getAuthStatus: server.getAuthStatus }));
vi.mock('#lib/server/settings.js', () => ({ getSettings: server.getSettings }));
vi.mock('#lib/server/galleries.js', () => ({ listGalleries: server.listGalleries }));

import { load } from './+page.server';

function run() {
	return load({
		fetch: vi.fn(),
		request: new Request('http://localhost/settings'),
		setHeaders: vi.fn()
	} as unknown as Parameters<typeof load>[0]) as Promise<Record<string, unknown>>;
}

describe('settings load', () => {
	beforeEach(() => {
		server.getAuthStatus.mockReset();
		server.getSettings.mockReset();
		server.listGalleries.mockReset();
		server.listGalleries.mockResolvedValue([]);
	});

	it('redirects to login without a session', async () => {
		server.getAuthStatus.mockResolvedValue({ identity: null });

		await expect(run()).rejects.toMatchObject({ status: 303, location: '/login' });
	});

	it('returns the idle timeout for a signed-in user', async () => {
		server.getAuthStatus.mockResolvedValue({
			identity: { account_id: 'a', workspace_id: 'w' },
			idle_timeout_ms: 1000
		});
		const settings = { studioName: 'Studio' };

		server.getSettings.mockResolvedValue(settings);

		const data = (await run()) as { idleTimeoutMs: number; settings: unknown };

		expect(data.idleTimeoutMs).toBe(1000);
		expect(data.settings).toBe(settings);
	});

	it('loads only the first cover for the cover preview, not a sample per gallery', async () => {
		server.getAuthStatus.mockResolvedValue({
			identity: { account_id: 'a', workspace_id: 'w' },
			idle_timeout_ms: 1000
		});
		server.getSettings.mockResolvedValue({});
		server.listGalleries.mockResolvedValue([
			{ id: 'g0', title: 'No cover', hero_id: null },
			{ id: 'g1', title: 'Wedding', hero_id: 'p1' },
			{ id: 'g2', title: 'Other', hero_id: 'p2' }
		]);

		const data = await run();

		expect(data).not.toHaveProperty('samples');
		expect(data.cover).toMatchObject({ title: 'Wedding' });
		expect(String((data.cover as { image: string }).image)).toContain('g1');
	});

	it('still loads when the gallery list is unavailable', async () => {
		server.getAuthStatus.mockResolvedValue({
			identity: { account_id: 'a', workspace_id: 'w' },
			idle_timeout_ms: 1000
		});
		server.getSettings.mockResolvedValue({});
		server.listGalleries.mockRejectedValue(new Error('down'));

		expect((await run()).cover).toBeNull();
	});
});
