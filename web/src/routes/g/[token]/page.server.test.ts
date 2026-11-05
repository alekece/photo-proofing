import { beforeEach, describe, expect, it, vi } from 'vitest';

const api = vi.hoisted(() => ({
	getSharedGallery: vi.fn(),
	getSharedPhotos: vi.fn(),
	getSharedFavorites: vi.fn()
}));

vi.mock('#lib/server/galleries.js', () => api);

import { load } from './+page.server.js';

const gallery = {
	title: 'Wedding',
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	delivered: false
};
const run = () =>
	load({ params: { token: 'tok' }, fetch, setHeaders: vi.fn() } as never) as Promise<unknown>;

describe('client gallery load', () => {
	beforeEach(() => vi.resetAllMocks());

	it('builds proofs from the ready photos', async () => {
		api.getSharedGallery.mockResolvedValue(gallery);
		api.getSharedPhotos.mockResolvedValue([{ id: 'a', width: 3, height: 2 }]);
		api.getSharedFavorites.mockResolvedValue(['a']);

		expect(await run()).toMatchObject({
			gallery,
			proofs: [{ id: 'a', previewUrl: '/api/v1/galleries/shared/tok/photos/a/preview' }],
			favorites: ['a']
		});
	});

	it('does not list photos of an unavailable link', async () => {
		api.getSharedGallery.mockResolvedValue(null);

		expect(await run()).toEqual({ gallery: null, proofs: [], favorites: [] });
		expect(api.getSharedPhotos).not.toHaveBeenCalled();
	});

	it('shows the unavailable page when the link expires between the two reads', async () => {
		api.getSharedGallery.mockResolvedValue(gallery);
		api.getSharedPhotos.mockResolvedValue([]);
		api.getSharedFavorites.mockResolvedValue(null);

		expect(await run()).toEqual({ gallery: null, proofs: [], favorites: [] });
	});
});
