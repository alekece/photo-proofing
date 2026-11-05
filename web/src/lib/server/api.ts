import { PP_API_INTERNAL_URL } from '$app/env/private';

/** The internal API URL for `path`. Startup validation guarantees the address in production. */
export function apiUrl(path: string): string {
	return `${PP_API_INTERNAL_URL}${path}`;
}
