import { describe, expect, it, vi } from 'vitest';
import { getSharedGallery, getSharedPhotos } from './galleries';

const gallery = { title: 'Wedding', layout: 'grid', expires_at: null };
const token = 'A'.repeat(22);

describe('getSharedGallery', () => {
	it('parses the gallery and forwards no cookie', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify(gallery)));
		expect(await getSharedGallery(fetcher, token)).toEqual(gallery);
		expect(fetcher).toHaveBeenCalledWith(
			expect.stringContaining(`/api/v1/galleries/shared/${token}`),
			expect.objectContaining({
				headers: {},
				cache: 'no-store',
				redirect: 'error'
			})
		);
	});

	it('returns null for an unavailable link', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response('{}', { status: 404 }));
		expect(await getSharedGallery(fetcher, token)).toBeNull();
	});

	it('fails with 503 on a network error or a server error', async () => {
		await expect(
			getSharedGallery(vi.fn().mockRejectedValue(new Error('down')), token)
		).rejects.toMatchObject({ status: 503 });
		await expect(
			getSharedGallery(vi.fn().mockResolvedValue(new Response('', { status: 500 })), token)
		).rejects.toMatchObject({ status: 503 });
	});
});

describe('getSharedPhotos', () => {
	const photos = [{ id: 'a', width: 3, height: 2 }];

	it('parses the list and forwards no cookie', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify(photos)));
		expect(await getSharedPhotos(fetcher, token)).toEqual(photos);
		expect(fetcher).toHaveBeenCalledWith(
			expect.stringContaining(`/api/v1/galleries/shared/${token}/photos`),
			expect.objectContaining({ headers: {}, cache: 'no-store', redirect: 'error' })
		);
	});

	it('returns null for an unavailable link', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response('{}', { status: 404 }));
		expect(await getSharedPhotos(fetcher, token)).toBeNull();
	});

	it('fails with 503 on a network error or a server error', async () => {
		await expect(
			getSharedPhotos(vi.fn().mockRejectedValue(new Error('down')), token)
		).rejects.toMatchObject({ status: 503 });
		await expect(
			getSharedPhotos(vi.fn().mockResolvedValue(new Response('', { status: 500 })), token)
		).rejects.toMatchObject({ status: 503 });
	});
});
