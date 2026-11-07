import { beforeEach, describe, expect, it, vi } from 'vitest';

const server = vi.hoisted(() => ({ getAuthStatus: vi.fn(), getSettings: vi.fn() }));

vi.mock('#lib/server/auth.js', () => ({ getAuthStatus: server.getAuthStatus }));
vi.mock('#lib/server/settings.js', () => ({ getSettings: server.getSettings }));

import { load } from './+page.server';

function run() {
	return load({
		fetch: vi.fn(),
		request: new Request('http://localhost/settings'),
		setHeaders: vi.fn()
	} as unknown as Parameters<typeof load>[0]) as Promise<{ idleTimeoutMs: number }>;
}

describe('settings load', () => {
	beforeEach(() => {
		server.getAuthStatus.mockReset();
		server.getSettings.mockReset();
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
});
