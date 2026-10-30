import { describe, expect, it, vi } from 'vitest';
import { getSharedGallery } from './galleries';

const gallery = { title: 'Wedding', layout: 'grid', expires_at: null };
const token = 'A'.repeat(22);

describe('getSharedGallery', () => {
	it('parses the gallery and forwards no cookie', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify(gallery)));
		expect(await getSharedGallery(fetcher, token)).toEqual(gallery);
		expect(fetcher).toHaveBeenCalledWith(
			expect.stringContaining(`/api/v1/shared/${token}`),
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
