import { describe, expect, it, vi } from 'vitest';
import { GalleryApiError, updateGallery } from './api';

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
