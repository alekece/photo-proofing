import { beforeEach, describe, expect, it, vi } from 'vitest';

const api = vi.hoisted(() => ({ getSharedGallery: vi.fn(), getSharedPhotos: vi.fn() }));

vi.mock('#lib/server/galleries.js', () => api);

import { load } from './+page.server.js';

const gallery = { title: 'Wedding', layout: 'grid', expires_at: null };
const run = () =>
	load({ params: { token: 'tok' }, fetch, setHeaders: vi.fn() } as never) as Promise<unknown>;

describe('client gallery load', () => {
	beforeEach(() => vi.resetAllMocks());

	it('builds proofs from the ready photos', async () => {
		api.getSharedGallery.mockResolvedValue(gallery);
		api.getSharedPhotos.mockResolvedValue([{ id: 'a', width: 3, height: 2 }]);

		expect(await run()).toMatchObject({
			gallery,
			proofs: [{ id: 'a', previewUrl: '/api/v1/galleries/shared/tok/photos/a/preview' }]
		});
	});

	it('does not list photos of an unavailable link', async () => {
		api.getSharedGallery.mockResolvedValue(null);

		expect(await run()).toEqual({ gallery: null, proofs: [] });
		expect(api.getSharedPhotos).not.toHaveBeenCalled();
	});

	it('shows the unavailable page when the link expires between the two reads', async () => {
		api.getSharedGallery.mockResolvedValue(gallery);
		api.getSharedPhotos.mockResolvedValue(null);

		expect(await run()).toEqual({ gallery: null, proofs: [] });
	});
});
