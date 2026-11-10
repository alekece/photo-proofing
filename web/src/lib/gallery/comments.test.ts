import { describe, expect, it, vi } from 'vitest';
import { CommentsClient } from './comments';
import { GalleryApiError } from './api';

export const comment = {
	id: 'c1',
	photo_id: 'p1',
	author: 'client',
	body: 'Nice',
	created_at: '2026-11-10T09:00:00Z',
	edited_at: null,
	deleted_at: null
};

describe('CommentsClient', () => {
	it('maps counts and sends the scoped write methods and bodies', async () => {
		const fetcher = vi
			.fn<typeof fetch>()
			.mockResolvedValueOnce(Response.json([{ photo_id: 'p1', count: 2 }]))
			.mockResolvedValueOnce(Response.json(comment))
			.mockResolvedValueOnce(Response.json({ ...comment, body: 'Changed' }))
			.mockResolvedValueOnce(new Response(null, { status: 204 }));
		const client = new CommentsClient('/api/v1/galleries/shared/token', fetcher);
		expect(await client.counts()).toEqual(new Map([['p1', 2]]));
		expect(await client.add('p1', 'Nice')).toEqual(comment);
		expect(await client.edit('c1', 'Changed')).toMatchObject({ body: 'Changed' });
		await client.remove('c1');
		expect(fetcher).toHaveBeenNthCalledWith(
			2,
			'/api/v1/galleries/shared/token/photos/p1/comments',
			expect.objectContaining({ method: 'POST', body: '{"body":"Nice"}' })
		);
		expect(fetcher).toHaveBeenNthCalledWith(
			3,
			'/api/v1/galleries/shared/token/comments/c1',
			expect.objectContaining({ method: 'PATCH' })
		);
		expect(fetcher).toHaveBeenNthCalledWith(
			4,
			'/api/v1/galleries/shared/token/comments/c1',
			expect.objectContaining({ method: 'DELETE' })
		);
	});

	it('preserves API errors and refuses malformed data', async () => {
		const client = new CommentsClient(
			'/base',
			vi
				.fn<typeof fetch>()
				.mockResolvedValueOnce(Response.json({ code: 'gallery_delivered' }, { status: 409 }))
				.mockResolvedValueOnce(Response.json([{}]))
		);
		await expect(client.add('p1', 'Hi')).rejects.toMatchObject({
			status: 409,
			code: 'gallery_delivered'
		});
		await expect(client.thread('p1')).rejects.toBeInstanceOf(GalleryApiError);
	});
});
