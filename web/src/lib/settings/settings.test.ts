import { expect, it } from 'vitest';
import { DEFAULT_SETTINGS, settingsChanged, watermarkChanged } from './settings.js';

it('tells a watermark change from a signature-only change', () => {
	const signature = { ...DEFAULT_SETTINGS, signature: 'none' as const };

	expect(settingsChanged(signature, DEFAULT_SETTINGS)).toBe(true);
	expect(watermarkChanged(signature, DEFAULT_SETTINGS)).toBe(false);
	expect(watermarkChanged({ ...DEFAULT_SETTINGS, size: 4 }, DEFAULT_SETTINGS)).toBe(true);
	expect(settingsChanged({ ...DEFAULT_SETTINGS }, DEFAULT_SETTINGS)).toBe(false);
});
