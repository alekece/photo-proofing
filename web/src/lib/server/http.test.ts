import * as v from 'valibot';
import { describe, expect, it, vi } from 'vitest';
import { readSharedJson, readWorkspaceJson } from './http.js';

const itemSchema = v.object({ id: v.string() });

describe('readWorkspaceJson', () => {
	const parse = (body: unknown) => {
		const parsed = v.safeParse(itemSchema, body);
		return parsed.success ? parsed.output : null;
	};

	it('forwards Cookie, disables caching/redirects, and parses valid JSON', async () => {
		const fetcher = vi.fn(async () => Response.json({ id: 'g1' }));

		expect(
			await readWorkspaceJson(fetcher, 'pp_session=xyz', '/api/v1/items/g1', parse, {
				unavailable: 'Unavailable',
				notFound: 'Missing'
			})
		).toEqual({ id: 'g1' });
		expect(fetcher).toHaveBeenCalledWith(expect.stringContaining('/api/v1/items/g1'), {
			headers: { Cookie: 'pp_session=xyz' },
			cache: 'no-store',
			redirect: 'error'
		});
	});

	it('redirects 401 to /login, maps 404 to notFound only when provided, and maps other failures to 503', async () => {
		await expect(
			readWorkspaceJson(
				vi.fn(async () => new Response(null, { status: 401 })),
				null,
				'/api/v1/items',
				parse,
				{ unavailable: 'Unavailable' }
			)
		).rejects.toMatchObject({ status: 303, location: '/login' });

		await expect(
			readWorkspaceJson(
				vi.fn(async () => new Response(null, { status: 404 })),
				null,
				'/api/v1/items/missing',
				parse,
				{ unavailable: 'Unavailable', notFound: 'Missing item' }
			)
		).rejects.toMatchObject({ status: 404, body: { message: 'Missing item' } });

		await expect(
			readWorkspaceJson(
				vi.fn(async () => new Response(null, { status: 404 })),
				null,
				'/api/v1/items',
				parse,
				{ unavailable: 'Unavailable' }
			)
		).rejects.toMatchObject({ status: 503, body: { message: 'Unavailable' } });
	});
});

describe('readSharedJson', () => {
	it('forwards only valid pp_gallery_unlock cookies and drops malformed values', async () => {
		const fetcher = vi.fn(async () => Response.json({ id: 'p1' }));

		expect(
			await readSharedJson(fetcher, 'tok_1', '/photos', itemSchema, 'valid_Token-1', 'Unavailable')
		).toEqual({ id: 'p1' });
		expect(fetcher).toHaveBeenLastCalledWith(
			expect.stringContaining('/api/v1/galleries/shared/tok_1/photos'),
			{
				headers: { Cookie: 'pp_gallery_unlock=valid_Token-1' },
				cache: 'no-store',
				redirect: 'error'
			}
		);

		const badCookieFetch = vi.fn(async () => Response.json({ id: 'p1' }));
		await readSharedJson(
			badCookieFetch,
			'tok_1',
			'',
			itemSchema,
			'bad; pp_session=steal',
			'Unavailable'
		);
		expect(badCookieFetch).toHaveBeenCalledWith(
			expect.stringContaining('/api/v1/galleries/shared/tok_1'),
			expect.objectContaining({ headers: {} })
		);
	});

	it('returns null on 401 or 404 without redirecting and throws 503 on invalid payloads', async () => {
		for (const status of [401, 404]) {
			const fetcher = vi.fn(async () => new Response('{}', { status }));
			expect(
				await readSharedJson(fetcher, 'tok_1', '', itemSchema, null, 'Unavailable')
			).toBeNull();
		}

		await expect(
			readSharedJson(
				vi.fn(async () => new Response('{"bad":true}', { status: 200 })),
				'tok_1',
				'',
				itemSchema,
				null,
				'Unavailable'
			)
		).rejects.toMatchObject({ status: 503 });
	});
});
