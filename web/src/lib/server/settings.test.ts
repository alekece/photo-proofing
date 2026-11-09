import { describe, expect, it, vi } from 'vitest';
import { getSettings } from './settings';

const body = {
	studio_name: 'Studio',
	mark: 'name',
	pattern: 'repeat',
	position: 8,
	size: 2,
	signature: 'name',
	signature_size: 4,
	proof_quality: 'medium',
	has_logo: false
};

describe('getSettings', () => {
	it('parses the settings and forwards the cookie', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify(body)));

		expect((await getSettings(fetcher, 'pp_session=x')).studioName).toBe('Studio');
		expect(fetcher).toHaveBeenCalledWith(
			expect.stringContaining('/api/v1/settings'),
			expect.objectContaining({ headers: { Cookie: 'pp_session=x' }, cache: 'no-store' })
		);
	});

	it('redirects to sign-in on 401 and fails with 503 otherwise', async () => {
		await expect(
			getSettings(vi.fn().mockResolvedValue(new Response('{}', { status: 401 })), null)
		).rejects.toMatchObject({ status: 303, location: '/login' });
		await expect(
			getSettings(vi.fn().mockResolvedValue(new Response('{}', { status: 500 })), null)
		).rejects.toMatchObject({ status: 503 });
		await expect(
			getSettings(vi.fn().mockRejectedValue(new Error('down')), null)
		).rejects.toMatchObject({ status: 503 });
		await expect(
			getSettings(vi.fn().mockResolvedValue(new Response('{"studio_name":1}')), null)
		).rejects.toMatchObject({ status: 503 });
	});
});
