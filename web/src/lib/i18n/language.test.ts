import { describe, expect, it, vi } from 'vitest';
import { saveLanguage } from './language.js';

describe('saveLanguage', () => {
	it('saves the workspace language', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response('{"language":"fr"}'));

		await saveLanguage('fr', fetcher);

		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/settings/language',
			expect.objectContaining({ method: 'PUT', body: '{"language":"fr"}' })
		);
	});

	it('fails when the API refuses', async () => {
		await expect(
			saveLanguage('fr', vi.fn().mockResolvedValue(new Response('{}', { status: 500 })))
		).rejects.toThrow();
	});
});
