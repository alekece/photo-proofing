import { describe, expect, it, vi } from 'vitest';
import { createGallery, GalleryApiError, shareGallery, updateGallery } from './api';

describe('gallery API errors', () => {
	it('reads the failing setting from context[0] of a 422', async () => {
		const fetcher = vi
			.fn()
			.mockResolvedValue(
				new Response(JSON.stringify({ error: 'too long', context: ['title'] }), { status: 422 })
			);
		await expect(updateGallery('g1', { title: 'x' }, fetcher)).rejects.toMatchObject({
			status: 422,
			message: 'too long',
			field: 'title'
		});
		await expect(updateGallery('g1', {}, fetcher)).rejects.toBeInstanceOf(GalleryApiError);
	});
});

const validGallery = {
	id: 'g',
	title: null,
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	share_token: null,
	states: ['draft'],
	created_at: '2026-11-05T10:00:00Z',
	updated_at: '2026-11-05T10:00:00Z'
};

describe('malformed gallery responses', () => {
	it.each([
		['createGallery', (f: typeof fetch) => createGallery(f)],
		['updateGallery', (f: typeof fetch) => updateGallery('g', {}, f)],
		['shareGallery', (f: typeof fetch) => shareGallery('g', f)]
	])('%s rejects a body that is not a gallery', async (_name, call) => {
		for (const body of ['{"id":"g"}', 'null', 'not json']) {
			const fetcher = vi.fn().mockResolvedValue(new Response(body));

			await expect(call(fetcher)).rejects.toBeInstanceOf(GalleryApiError);
		}
	});
});

describe('shareGallery', () => {
	it('posts to the share path and returns the gallery with its link token', async () => {
		const gallery = { ...validGallery, id: 'g 1', share_token: 'A'.repeat(22) };
		const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify(gallery)));
		expect(await shareGallery('g 1', fetcher)).toEqual(gallery);
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/g%201/share',
			expect.objectContaining({ method: 'POST' })
		);
	});
});
