import type * as v from 'valibot';
import { parseOrNull } from '#lib/gallery/schemas.js';
import { apiErrorMessage, readResponseErrorCode } from '#lib/i18n/errors.js';

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string,
		/** The API's error `code`, when it sent one. */
		public code?: string
	) {
		super(message);
	}
}

function buildInit(init: RequestInit): RequestInit {
	if (init.body === undefined) {
		return { ...init, credentials: 'same-origin' };
	}

	if (init.headers === undefined) {
		return {
			...init,
			credentials: 'same-origin',
			headers: { 'Content-Type': 'application/json' }
		};
	}

	if (new Headers(init.headers).has('Content-Type')) {
		return { ...init, credentials: 'same-origin' };
	}

	if (init.headers instanceof Headers || Array.isArray(init.headers)) {
		const headers = new Headers(init.headers);
		headers.set('Content-Type', 'application/json');

		return { ...init, credentials: 'same-origin', headers };
	}

	return {
		...init,
		credentials: 'same-origin',
		headers: { ...init.headers, 'Content-Type': 'application/json' }
	};
}

export async function requestJson<const S extends v.GenericSchema>(
	url: string,
	init: RequestInit,
	schema: S,
	fetcher?: typeof fetch
): Promise<v.InferOutput<S>>;
export async function requestJson<const S extends v.GenericSchema>(
	url: string,
	init: RequestInit,
	schema: S,
	fetcher: typeof fetch | undefined,
	options: { emptyAsNull: true }
): Promise<v.InferOutput<S> | null>;
export async function requestJson<const S extends v.GenericSchema>(
	url: string,
	init: RequestInit,
	schema: S,
	fetcher: typeof fetch = fetch,
	options?: { emptyAsNull?: boolean }
): Promise<v.InferOutput<S> | null> {
	const response = await fetcher(url, buildInit(init));

	if (!response.ok) {
		const code = await readResponseErrorCode(response);

		throw new ApiError(response.status, apiErrorMessage(response.status, code), code);
	}

	if (options?.emptyAsNull && response.status === 204) {
		return null;
	}

	const parsed = parseOrNull(schema, await response.json().catch(() => null));

	if (parsed === null) throw new ApiError(502, apiErrorMessage(502));

	return parsed;
}

export async function requestEmpty(
	url: string,
	init: RequestInit,
	fetcher: typeof fetch = fetch
): Promise<void> {
	const response = await fetcher(url, buildInit(init));

	if (!response.ok) {
		const code = await readResponseErrorCode(response);

		throw new ApiError(response.status, apiErrorMessage(response.status, code), code);
	}
}
