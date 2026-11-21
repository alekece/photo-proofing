import { expect, it, vi } from 'vitest';
import { ApiError } from '#lib/http.js';
import {
	DEFAULT_SETTINGS,
	LOGO_URL,
	normalizeWatermarkHex,
	parseSettings,
	SettingsApi,
	settingsChanged,
	watermarkChanged
} from './settings.js';

it('tells a watermark change from a signature-only change', () => {
	const signature = { ...DEFAULT_SETTINGS, signature: 'none' as const };

	expect(settingsChanged(signature, DEFAULT_SETTINGS)).toBe(true);
	expect(watermarkChanged(signature, DEFAULT_SETTINGS)).toBe(false);
	expect(watermarkChanged({ ...DEFAULT_SETTINGS, size: 5 }, DEFAULT_SETTINGS)).toBe(true);
	expect(settingsChanged({ ...DEFAULT_SETTINGS }, DEFAULT_SETTINGS)).toBe(false);
});

const body = {
	studio_name: 'Studio',
	mark: 'both',
	pattern: 'once',
	position: 4,
	size: 3,
	signature: 'logo',
	signature_size: 4,
	proof_quality: 'high',
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
		signature: 'logo',
		signatureSize: 4,
		signatureTextColor: null,
		proofQuality: 'high',
		watermarkOpacity: 50,
		watermarkTextColor: '#FFFFFF',
		emailEnabled: false
	});
	expect(parseSettings({ ...body, email_enabled: true })?.emailEnabled).toBe(true);
	expect(parseSettings({ ...body, proof_quality: 'ultra' })).toBeNull();
	expect(parseSettings({ ...body, position: 9 })).toBeNull();
	expect(parseSettings({ ...body, mark: 'x' })).toBeNull();
	expect(parseSettings(null)).toBeNull();
});

it('reads validated appearance, defaulting only omitted fields', () => {
	expect(
		parseSettings({ ...body, watermark_opacity: 0, watermark_text_color: '#A12B3C' })
	).toMatchObject({ watermarkOpacity: 0, watermarkTextColor: '#A12B3C' });
	for (const watermark_opacity of [-1, 101, 0.1, '50', null]) {
		expect(parseSettings({ ...body, watermark_opacity })).toBeNull();
	}
	for (const watermark_text_color of ['red', 'A12B3C', '#123456ff', '#A12B3C\n', null]) {
		expect(parseSettings({ ...body, watermark_text_color })).toBeNull();
	}
});

it('normalizes only six-digit hex from the UI', () => {
	expect(normalizeWatermarkHex(' a12b3c ')).toBe('#A12B3C');
	for (const invalid of ['#ABC', 'red', 'rgb(1 2 3)', '#123456ff', '#00 000']) {
		expect(normalizeWatermarkHex(invalid)).toBeNull();
	}
});

it('sends and detects appearance edits even with an inactive text mark', async () => {
	const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify(body)));
	const previous = { ...DEFAULT_SETTINGS, mark: 'logo' as const };
	const draft = { ...previous, watermarkOpacity: 0, watermarkTextColor: '#A12B3C' };

	expect(settingsChanged({ ...previous, watermarkTextColor: '#A12B3C' }, previous)).toBe(true);
	expect(settingsChanged({ ...previous, watermarkOpacity: 0 }, previous)).toBe(true);
	await new SettingsApi(fetcher).save(draft, previous);

	expect(sent(fetcher)).toMatchObject({ watermark_opacity: 0, watermark_text_color: '#A12B3C' });
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
			new Response(JSON.stringify({ code: 'logo_required' }), {
				status: 422
			})
		)
	);

	await expect(refused.save(DEFAULT_SETTINGS, DEFAULT_SETTINGS)).rejects.toMatchObject({
		status: 422,
		code: 'logo_required'
	});
	await expect(refused.save(DEFAULT_SETTINGS, DEFAULT_SETTINGS)).rejects.toBeInstanceOf(ApiError);
});
