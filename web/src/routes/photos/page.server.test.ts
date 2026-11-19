import { error } from '@sveltejs/kit';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const server = vi.hoisted(() => ({
	getAuthStatus: vi.fn(),
	listWorkspacePhotos: vi.fn(),
	getSettings: vi.fn()
}));

vi.mock('#lib/server/auth.js', () => ({ getAuthStatus: server.getAuthStatus }));
vi.mock('#lib/server/photos.js', () => ({
	listWorkspacePhotos: server.listWorkspacePhotos
}));
vi.mock('#lib/server/settings.js', () => ({ getSettings: server.getSettings }));

import { load } from './+page.server';

function run() {
	const setHeaders = vi.fn();
	return {
		setHeaders,
		promise: load({
			fetch: vi.fn(),
			request: new Request('http://localhost/photos', {
				headers: { cookie: 'pp_session=xyz' }
			}),
			setHeaders
		} as unknown as Parameters<typeof load>[0])
	};
}

const photo = {
	id: 'p1',
	original_name: 'DSC_1234.jpg',
	gallery_id: 'g1',
	gallery_title: 'Wedding',
	kind: 'proof' as const,
	revision: 0,
	width: 2,
	height: 1,
	created_at: '2026-11-04T10:00:00Z'
};

describe('photos server load', () => {
	beforeEach(() => {
		server.getAuthStatus.mockResolvedValue({
			identity: { account_id: 'a', workspace_id: 'w' },
			idle_timeout_ms: 120000
		});
		server.listWorkspacePhotos.mockResolvedValue([photo]);
		server.getSettings.mockResolvedValue({ studioName: 'Atelier Lumiere' });
	});

	it('redirects unauthenticated requests to /login', async () => {
		server.getAuthStatus.mockResolvedValueOnce({ identity: null, idle_timeout_ms: 120000 });

		await expect(run().promise).rejects.toMatchObject({ status: 303, location: '/login' });
	});

	it('loads every ready workspace photo with Cache-Control: no-store', async () => {
		const { setHeaders, promise } = run();
		const result = await promise;

		expect(setHeaders).toHaveBeenCalledWith({ 'Cache-Control': 'no-store' });
		expect(result).toEqual({
			identity: { account_id: 'a', workspace_id: 'w' },
			idleTimeoutMs: 120000,
			photos: [photo],
			settings: { studioName: 'Atelier Lumiere' }
		});
	});

	it('returns photos: null when the backend is unavailable (503)', async () => {
		server.listWorkspacePhotos.mockImplementationOnce(async () => error(503, 'Unavailable'));

		const result = await run().promise;

		expect(result).toMatchObject({ photos: null });
	});
});
