import { error, redirect } from '@sveltejs/kit';
import type * as v from 'valibot';
import { parseOrNull } from '#lib/gallery/schemas.js';
import { apiUrl } from './api.js';

const UNLOCK_COOKIE_PATTERN = /^[A-Za-z0-9_-]+$/;

/** Server-side read with the workspace session cookie; 401 redirects to sign-in, other failures become 503 (or 404 when `notFound` is configured). */
export async function readWorkspaceJson<T>(
	fetcher: typeof fetch,
	cookie: string | null,
	path: string,
	parse: (body: unknown) => T | null,
	options: { unavailable: string; notFound?: string }
): Promise<T> {
	let response: Response;

	try {
		response = await fetcher(apiUrl(path), {
			headers: cookie ? { Cookie: cookie } : {},
			cache: 'no-store',
			redirect: 'error'
		});
	} catch {
		error(503, options.unavailable);
	}

	if (response.status === 401) redirect(303, '/login');
	if (options.notFound !== undefined && response.status === 404) error(404, options.notFound);
	if (!response.ok) error(503, options.unavailable);

	const parsed = parse(await response.json().catch(() => null));

	if (parsed === null) error(503, options.unavailable);

	return parsed;
}

/**
 * Server-side read for a public share token, forwarding only `pp_gallery_unlock` when valid.
 * Returns `null` on 401 or 404; other failures raise 503.
 */
export async function readSharedJson<const S extends v.GenericSchema>(
	fetcher: typeof fetch,
	token: string,
	path: string,
	schema: S,
	unlockCookie: string | null | undefined,
	unavailable: string
): Promise<v.InferOutput<S> | null> {
	const headers: HeadersInit =
		unlockCookie && UNLOCK_COOKIE_PATTERN.test(unlockCookie)
			? { Cookie: `pp_gallery_unlock=${unlockCookie}` }
			: {};
	let response: Response;

	try {
		response = await fetcher(
			apiUrl(`/api/v1/galleries/shared/${encodeURIComponent(token)}${path}`),
			{
				headers,
				cache: 'no-store',
				redirect: 'error'
			}
		);
	} catch {
		error(503, unavailable);
	}

	if (response.status === 404 || response.status === 401) return null;
	if (!response.ok) error(503, unavailable);

	const parsed = parseOrNull(schema, await response.json().catch(() => null));

	if (parsed === null) error(503, unavailable);

	return parsed;
}
