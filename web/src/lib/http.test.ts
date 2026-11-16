import * as v from 'valibot';
import { describe, expect, it, vi } from 'vitest';
import { ApiError, requestEmpty, requestJson } from './http.js';

const itemSchema = v.object({ id: v.string() });

describe('requestJson', () => {
	it('omits Content-Type when init has no body and sets credentials: same-origin', async () => {
		const fetcher = vi.fn(async () => Response.json({ id: 'a' }));

		expect(await requestJson('/api/v1/items', { method: 'GET' }, itemSchema, fetcher)).toEqual({
			id: 'a'
		});
		expect(fetcher).toHaveBeenCalledWith('/api/v1/items', {
			method: 'GET',
			credentials: 'same-origin'
		});
	});

	it('sets Content-Type: application/json when init.body is defined and preserves caller options like keepalive', async () => {
		const fetcher = vi.fn(async () => Response.json({ id: 'b' }));

		expect(
			await requestJson(
				'/api/v1/items/b',
				{ method: 'PATCH', body: JSON.stringify({ name: 'B' }), keepalive: true },
				itemSchema,
				fetcher
			)
		).toEqual({ id: 'b' });
		expect(fetcher).toHaveBeenCalledWith('/api/v1/items/b', {
			method: 'PATCH',
			body: JSON.stringify({ name: 'B' }),
			keepalive: true,
			credentials: 'same-origin',
			headers: { 'Content-Type': 'application/json' }
		});
	});

	it('returns null on 204 when emptyAsNull is enabled', async () => {
		const fetcher = vi.fn(async () => new Response(null, { status: 204 }));

		expect(
			await requestJson('/api/v1/items/latest', { method: 'GET' }, itemSchema, fetcher, {
				emptyAsNull: true
			})
		).toBeNull();
	});

	it('maps non-2xx responses and error codes to ApiError', async () => {
		const fetcher = vi.fn(
			async () =>
				new Response(JSON.stringify({ code: 'invalid_title' }), {
					status: 422,
					headers: { 'Content-Type': 'application/json' }
				})
		);

		await expect(
			requestJson('/api/v1/items', { method: 'POST' }, itemSchema, fetcher)
		).rejects.toMatchObject({
			status: 422,
			code: 'invalid_title',
			message: 'Enter a title of up to 255 characters.'
		} satisfies Partial<ApiError>);
	});

	it('throws ApiError(502) when JSON parsing or schema validation fails', async () => {
		const badJson = vi.fn(async () => new Response('not json', { status: 200 }));
		await expect(requestJson('/api/v1/items', {}, itemSchema, badJson)).rejects.toEqual(
			new ApiError(502, 'Unexpected response from the server.')
		);

		const badShape = vi.fn(async () => Response.json({ id: 123 }));
		await expect(requestJson('/api/v1/items', {}, itemSchema, badShape)).rejects.toMatchObject({
			status: 502
		});
	});
});

describe('requestEmpty', () => {
	it('resolves on 204 and maps non-2xx responses to ApiError', async () => {
		const okFetch = vi.fn(async () => new Response(null, { status: 204 }));
		await requestEmpty('/api/v1/items/a', { method: 'DELETE' }, okFetch);
		expect(okFetch).toHaveBeenCalledWith('/api/v1/items/a', {
			method: 'DELETE',
			credentials: 'same-origin'
		});

		const errFetch = vi.fn(
			async () => new Response(JSON.stringify({ code: 'not_found' }), { status: 404 })
		);
		await expect(
			requestEmpty('/api/v1/items/a', { method: 'DELETE' }, errFetch)
		).rejects.toBeInstanceOf(ApiError);
	});
});
