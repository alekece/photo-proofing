import { defineCustomClientStrategy } from '#lib/paraglide/runtime.js';

/**
 * Keeps the browser on the language the server rendered. The API holds the setting, so there is
 * nothing to store when it changes: `saveLanguage` saves it and the page reloads.
 */
export function registerClientLocale(): void {
	defineCustomClientStrategy('custom-workspace', {
		getLocale: () => document.documentElement.lang || undefined,
		setLocale: () => {}
	});
}
