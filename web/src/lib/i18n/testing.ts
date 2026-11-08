import { afterEach, beforeEach } from 'vitest';
import { getLocale, overwriteGetLocale, type Locale } from '#lib/paraglide/runtime.js';

/** Renders the surrounding tests in `locale`, then gives the browser's language back. */
export function useLocale(locale: Locale): void {
	const browserLocale = getLocale;

	beforeEach(() => overwriteGetLocale(() => locale));
	afterEach(() => overwriteGetLocale(browserLocale));
}
