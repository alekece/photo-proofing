import { describe, expect, it, vi } from 'vitest';
import { recordOpened } from './opened.js';

describe('recordOpened', () => {
	it('posts to the open route of the link', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));

		await recordOpened('a/b', fetcher);

		expect(fetcher).toHaveBeenCalledWith('/api/v1/galleries/shared/a%2Fb/open', {
			method: 'POST',
			keepalive: true
		});
	});

	it('swallows a failing request', async () => {
		const fetcher = vi.fn().mockRejectedValue(new TypeError('offline'));

		await expect(recordOpened('tok', fetcher)).resolves.toBeUndefined();
	});
});
