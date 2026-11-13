import { beforeEach, describe, expect, it, vi } from 'vitest';

const api = vi.hoisted(() => ({
	getSharedGallery: vi.fn(),
	getSharedPhotos: vi.fn(),
	getSharedFavorites: vi.fn()
}));

vi.mock('#lib/server/galleries.js', () => api);

import { load } from './+page.server.js';

const gallery = {
	locked: false as const,
	title: 'Wedding',
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	delivered: false
};
const run = (unlockCookie?: string) =>
	load({
		params: { token: 'tok' },
		fetch,
		cookies: { get: (name: string) => (name === 'pp_gallery_unlock' ? unlockCookie : undefined) },
		setHeaders: vi.fn()
	} as never) as Promise<unknown>;

describe('client gallery load', () => {
	beforeEach(() => vi.resetAllMocks());

	it('builds proofs from the ready photos and forwards pp_gallery_unlock when present', async () => {
		api.getSharedGallery.mockResolvedValue(gallery);
		api.getSharedPhotos.mockResolvedValue([{ id: 'a', width: 3, height: 2 }]);
		api.getSharedFavorites.mockResolvedValue(['a']);

		expect(await run('unlock_val')).toMatchObject({
			gallery,
			lockedGallery: null,
			proofs: [{ id: 'a', previewUrl: '/api/v1/galleries/shared/tok/photos/a/preview' }],
			favorites: ['a']
		});
		expect(api.getSharedGallery).toHaveBeenCalledWith(fetch, 'tok', 'unlock_val');
		expect(api.getSharedPhotos).toHaveBeenCalledWith(fetch, 'tok', 'unlock_val');
		expect(api.getSharedFavorites).toHaveBeenCalledWith(fetch, 'tok', 'unlock_val');
	});

	it('returns lockedGallery without requesting /photos or /favorites while locked', async () => {
		const locked = {
			locked: true as const,
			title: 'Secret',
			shoot_date: '2026-06-14',
			has_cover: true
		};
		api.getSharedGallery.mockResolvedValue(locked);

		expect(await run()).toEqual({
			gallery: null,
			lockedGallery: locked,
			proofs: [],
			favorites: [],
			coverUrl: '/api/v1/galleries/shared/tok/cover'
		});
		expect(api.getSharedPhotos).not.toHaveBeenCalled();
		expect(api.getSharedFavorites).not.toHaveBeenCalled();
	});

	it('does not list photos of an unavailable link', async () => {
		api.getSharedGallery.mockResolvedValue(null);

		expect(await run()).toEqual({ gallery: null, lockedGallery: null, proofs: [], favorites: [] });
		expect(api.getSharedPhotos).not.toHaveBeenCalled();
	});

	it('shows the unavailable page when the link expires between the two reads', async () => {
		api.getSharedGallery.mockResolvedValue(gallery);
		api.getSharedPhotos.mockResolvedValue([]);
		api.getSharedFavorites.mockResolvedValue(null);

		expect(await run()).toEqual({ gallery: null, lockedGallery: null, proofs: [], favorites: [] });
	});
});
