import type { Locale } from '#lib/paraglide/runtime.js';

/** Saves the workspace language; the caller reloads so every page follows it. */
export async function saveLanguage(
	language: Locale,
	fetcher: typeof fetch = (...args) => fetch(...args)
): Promise<void> {
	const response = await fetcher('/api/v1/settings/language', {
		method: 'PUT',
		credentials: 'same-origin',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ language })
	});

	if (!response.ok) throw new Error(`The language was not saved (${response.status}).`);
}
