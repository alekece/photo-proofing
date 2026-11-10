import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
import type { LiveEvents } from '#lib/events.js';
import Harness from './CommentsPanelHarness.test.svelte';

const off = () => () => {};
const events: LiveEvents = {
	onPhoto: off,
	onFavorites: off,
	onNotification: off,
	onExport: off,
	onResync: off,
	start() {},
	close() {}
};

const photo = (id: string) => ({
	id,
	original_name: `${id}.jpg`,
	size: 3,
	width: 300,
	height: 200,
	status: 'ready',
	favorited_at: null,
	created_at: '2026-11-04T10:00:00Z'
});

const stub = (counts: { photo_id: string; count: number }[]) =>
	vi.stubGlobal(
		'fetch',
		vi.fn(async (url: string) => {
			if (url.endsWith('/photos/p2/comments'))
				return Response.json([
					{
						id: 'c1',
						photo_id: 'p2',
						author: 'client',
						body: 'Brighter please',
						created_at: '2026-11-10T09:00:00Z',
						edited_at: null,
						deleted_at: null
					}
				]);
			if (url.endsWith('/comments')) return Response.json(counts);
			return Response.json([photo('p1'), photo('p2')]);
		})
	);

afterEach(() => vi.unstubAllGlobals());

describe('CommentsPanel', () => {
	it('lists only the photos with comments, reports their number and opens the thread', async () => {
		const user = userEvent.setup();
		const oncount = vi.fn();
		stub([{ photo_id: 'p2', count: 2 }]);
		render(Harness, { props: { events, oncount } });

		await user.click(await screen.findByRole('button', { name: 'Open p2.jpg' }));

		expect(screen.queryByRole('button', { name: 'Open p1.jpg' })).toBeNull();
		expect(await screen.findByText('Brighter please')).toBeTruthy();
		expect(oncount).toHaveBeenLastCalledWith(1);
	});

	it('explains when no photo has comments', async () => {
		stub([]);
		render(Harness, { props: { events } });

		expect(await screen.findByText('No comments yet')).toBeTruthy();
	});
});
