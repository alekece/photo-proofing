import { describe, expect, it, vi } from 'vitest';
import { Selection } from './selection.svelte';

const respond = (status: number, body?: unknown) =>
	new Response(body === undefined ? null : JSON.stringify(body), { status });

function selection(fetcher: typeof fetch, initial: string[] = []) {
	return new Selection({ token: 'tok', initial, fetcher });
}

describe('Selection', () => {
	it('selects at once and keeps it once the server confirms', async () => {
		const fetcher = vi.fn(async () => respond(204));
		const picks = selection(fetcher);
		const saving = picks.toggle('a');

		expect(picks.has('a')).toBe(true);

		await saving;

		expect(picks.has('a')).toBe(true);
		expect(picks.error).toBeNull();
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/shared/tok/favorites/a',
			expect.objectContaining({ method: 'PUT', credentials: 'same-origin' })
		);
	});

	it('unselects with DELETE', async () => {
		const fetcher = vi.fn(async () => respond(204));
		const picks = selection(fetcher, ['a']);

		await picks.toggle('a');

		expect(picks.has('a')).toBe(false);
		expect(fetcher).toHaveBeenCalledWith(
			expect.stringContaining('/favorites/a'),
			expect.objectContaining({ method: 'DELETE' })
		);
	});

	it('reverts and says so when the save fails', async () => {
		const picks = selection(async () => respond(500), ['b']);

		await picks.toggle('a');
		await picks.toggle('b');

		expect(picks.has('a')).toBe(false);
		expect(picks.has('b')).toBe(true);
		expect(picks.error).toMatch(/couldn.t save/i);
	});

	it('reverts when the network fails and clears the message on the next success', async () => {
		const fetcher = vi
			.fn<typeof fetch>()
			.mockRejectedValueOnce(new TypeError('offline'))
			.mockResolvedValue(respond(204));
		const picks = selection(fetcher);

		await picks.toggle('a');

		expect(picks.has('a')).toBe(false);
		expect(picks.error).not.toBeNull();

		await picks.toggle('a');

		expect(picks.has('a')).toBe(true);
		expect(picks.error).toBeNull();
	});

	it('explains a delivered gallery and refuses further changes', async () => {
		const fetcher = vi.fn(async () => respond(409, { error: 'gallery already delivered' }));
		const picks = selection(fetcher);

		await picks.toggle('a');

		expect(picks.has('a')).toBe(false);
		expect(picks.error).toMatch(/final/i);
	});

	it('sends rapid toggles of one photo in order', async () => {
		const methods: string[] = [];
		const fetcher = vi.fn(async (_: unknown, init?: RequestInit) => {
			methods.push(init?.method ?? '');
			await Promise.resolve();

			return respond(204);
		}) as unknown as typeof fetch;
		const picks = selection(fetcher);

		await Promise.all([picks.toggle('a'), picks.toggle('a'), picks.toggle('a')]);

		expect(methods).toEqual(['PUT', 'DELETE', 'PUT']);
		expect(picks.has('a')).toBe(true);
	});
});
