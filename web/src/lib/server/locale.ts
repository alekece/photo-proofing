import { defineCustomServerStrategy, isLocale, type Locale } from '#lib/paraglide/runtime.js';
import { apiUrl } from './api.js';

/**
 * Renders every page in the workspace language. Without an answer (no workspace yet, the API
 * down) the next strategy, the browser language, decides.
 */
export function registerServerLocale(fetcher: typeof fetch): void {
	defineCustomServerStrategy('custom-workspace', {
		getLocale: async (): Promise<Locale | undefined> => {
			try {
				const response = await fetcher(apiUrl('/api/v1/language'), { cache: 'no-store' });
				if (!response.ok) return undefined;

				const { language } = await response.json();

				return isLocale(language) ? language : undefined;
			} catch {
				return undefined;
			}
		}
	});
}
