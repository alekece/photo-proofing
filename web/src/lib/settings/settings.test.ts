import { expect, it, vi } from 'vitest';
import {
	DEFAULT_SETTINGS,
	LOGO_URL,
	parseSettings,
	SettingsApi,
	SettingsApiError,
	settingsChanged,
	watermarkChanged
} from './settings.js';

it('tells a watermark change from a signature-only change', () => {
	const signature = { ...DEFAULT_SETTINGS, signature: 'none' as const };

	expect(settingsChanged(signature, DEFAULT_SETTINGS)).toBe(true);
	expect(watermarkChanged(signature, DEFAULT_SETTINGS)).toBe(false);
	expect(watermarkChanged({ ...DEFAULT_SETTINGS, size: 4 }, DEFAULT_SETTINGS)).toBe(true);
	expect(settingsChanged({ ...DEFAULT_SETTINGS }, DEFAULT_SETTINGS)).toBe(false);
});

const body = {
	studio_name: 'Studio',
	mark: 'both',
	pattern: 'once',
	position: 4,
	size: 3,
	signature: 'logo',
	has_logo: true
};

it('maps the API settings and rejects a malformed body', () => {
	expect(parseSettings(body)).toEqual({
		studioName: 'Studio',
		logo: LOGO_URL,
		mark: 'both',
		pattern: 'once',
		position: 4,
		size: 3,
		signature: 'logo'
	});
	expect(parseSettings({ ...body, position: 9 })).toBeNull();
	expect(parseSettings({ ...body, mark: 'x' })).toBeNull();
	expect(parseSettings(null)).toBeNull();
});

function sent(fetcher: ReturnType<typeof vi.fn>) {
	return JSON.parse(fetcher.mock.calls[0][1].body);
}

it.each([
	[
		'a new PNG is replaced',
		'data:image/png;base64,QUJD',
		null,
		{ action: 'replace', image_base64: 'QUJD' }
	],
	[
		'a new JPEG is replaced',
		'data:image/jpeg;base64,QUJD',
		null,
		{ action: 'replace', image_base64: 'QUJD' }
	],
	['a saved logo is kept', LOGO_URL, LOGO_URL, { action: 'keep' }],
	['a dropped logo is removed', null, LOGO_URL, { action: 'remove' }],
	['no logo stays none', null, null, { action: 'keep' }]
])('sends the logo change when %s', async (_name, draft, saved, logo) => {
	const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify(body)));
	const api = new SettingsApi(fetcher);

	await api.save({ ...DEFAULT_SETTINGS, logo: draft }, { ...DEFAULT_SETTINGS, logo: saved });

	expect(sent(fetcher).logo).toEqual(logo);
});

it('refetches a replaced logo and reports a refusal with its setting', async () => {
	const ok = new SettingsApi(vi.fn().mockResolvedValue(new Response(JSON.stringify(body))));
	const settings = await ok.save(
		{ ...DEFAULT_SETTINGS, logo: 'data:image/png;base64,QUJD' },
		DEFAULT_SETTINGS
	);

	expect(settings.logo).toMatch(new RegExp(`^${LOGO_URL}\\?v=\\d+$`));

	const refused = new SettingsApi(
		vi.fn().mockResolvedValue(
			new Response(JSON.stringify({ error: 'invalid settings', context: ['logo'] }), {
				status: 422
			})
		)
	);

	await expect(refused.save(DEFAULT_SETTINGS, DEFAULT_SETTINGS)).rejects.toMatchObject({
		status: 422,
		field: 'logo'
	});
	await expect(refused.save(DEFAULT_SETTINGS, DEFAULT_SETTINGS)).rejects.toBeInstanceOf(
		SettingsApiError
	);
});
