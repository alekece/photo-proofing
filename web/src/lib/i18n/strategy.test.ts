import { afterEach, describe, expect, it, vi } from 'vitest';
import { customClientStrategies, customServerStrategies } from '#lib/paraglide/runtime.js';
import { registerServerLocale } from '#lib/server/locale.js';
import { registerClientLocale } from './strategy.js';

vi.mock('$app/env/private', () => ({ PP_API_INTERNAL_URL: 'http://api.test' }));

const serverLocale = async (fetcher: typeof fetch) => {
	registerServerLocale(fetcher);

	return customServerStrategies.get('custom-workspace')?.getLocale(new Request('http://web.test/'));
};

describe('the workspace language strategy', () => {
	afterEach(() => document.documentElement.removeAttribute('lang'));

	it('takes the language the API reports on the server', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response('{"language":"fr"}'));

		expect(await serverLocale(fetcher)).toBe('fr');
		expect(fetcher).toHaveBeenCalledWith(
			'http://api.test/api/v1/language',
			expect.objectContaining({ cache: 'no-store' })
		);
	});

	it('lets the next strategy decide when the API has no answer', async () => {
		expect(await serverLocale(vi.fn().mockResolvedValue(new Response('{"language":null}')))).toBe(
			undefined
		);
		expect(await serverLocale(vi.fn().mockRejectedValue(new Error('down')))).toBe(undefined);
		expect(await serverLocale(vi.fn().mockResolvedValue(new Response('{}', { status: 503 })))).toBe(
			undefined
		);
		expect(await serverLocale(vi.fn().mockResolvedValue(new Response('{"language":"de"}')))).toBe(
			undefined
		);
	});

	it('reads the language the server rendered in the browser', () => {
		registerClientLocale();
		document.documentElement.lang = 'fr';

		expect(customClientStrategies.get('custom-workspace')?.getLocale()).toBe('fr');
	});
});
