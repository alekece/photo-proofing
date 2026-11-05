const DEV_DEFAULT = 'http://127.0.0.1:8080';

/**
 * Validates `PP_API_INTERNAL_URL`. Production must set it; only `pnpm dev` falls back to the local
 * default, and the build step, which runs without deployment settings, accepts it missing.
 */
export function resolveApiAddress(
	value: string | undefined,
	mode: { dev: boolean; building: boolean }
): string {
	if (value) return value.replace(/\/+$/, '');
	if (mode.dev) return DEV_DEFAULT;
	if (mode.building) return '';

	throw new Error(
		'PP_API_INTERNAL_URL is not set: set it to the API address reachable by the web server'
	);
}
