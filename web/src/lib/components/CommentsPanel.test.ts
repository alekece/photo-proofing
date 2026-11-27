import { render, screen, waitFor, within } from '@testing-library/svelte';
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

const stub = (
	counts: { photo_id: string; count: number; latest_body?: string }[],
	summary = () => Response.json(counts)
) =>
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
						deleted_at: null,
						editable: false
					}
				]);
			if (url.endsWith('/photos/p1/comments')) return Response.json([]);
			if (url.endsWith('/comments')) return summary();
			return Response.json([photo('p1'), photo('p2')]);
		})
	);

afterEach(() => vi.unstubAllGlobals());

describe('CommentsPanel', () => {
	it('lists only the photos with comments, reports their number and opens the thread', async () => {
		const user = userEvent.setup();
		const oncount = vi.fn();
		stub([{ photo_id: 'p2', count: 2, latest_body: 'Brighter please' }]);
		render(Harness, { props: { events, oncount } });

		const button = await screen.findByRole('button', { name: /p2\.jpg/ });
		const row = button.closest('li')!;
		expect(within(row).getByText('Brighter please')).toBeTruthy();
		await user.click(button);

		expect(screen.queryByRole('button', { name: /p1\.jpg/ })).toBeNull();
		const thread = within(row).getByRole('region', { name: 'Comments' });
		expect(await within(thread).findByText('Brighter please')).toBeTruthy();
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(oncount).toHaveBeenLastCalledWith(1);
	});

	it('opens the thread of the photo a link asked for', async () => {
		stub([
			{ photo_id: 'p1', count: 1 },
			{ photo_id: 'p2', count: 2 }
		]);
		render(Harness, { props: { events, openThread: { photoId: 'p2' } } });

		const second = await screen.findByRole('button', { name: /p2\.jpg/ });

		await waitFor(() => expect(second.getAttribute('aria-expanded')).toBe('true'));
		expect(screen.getByRole('button', { name: /p1\.jpg/ }).getAttribute('aria-expanded')).toBe(
			'false'
		);
	});

	it('keeps unsent replies on their own row when switching threads', async () => {
		const user = userEvent.setup();
		stub([
			{ photo_id: 'p1', count: 1 },
			{ photo_id: 'p2', count: 2 }
		]);
		render(Harness, { props: { events } });

		const first = await screen.findByRole('button', { name: /p1\.jpg/ });
		const second = screen.getByRole('button', { name: /p2\.jpg/ });
		await user.click(first);
		await user.type(await screen.findByRole('textbox'), 'Try a lighter version');
		await user.click(second);
		expect(screen.getByRole('textbox')).toHaveProperty('value', '');
		await user.click(first);
		expect(screen.getByRole('textbox')).toHaveProperty('value', 'Try a lighter version');
	});

	it('keeps the open reply when a live summary refresh fails', async () => {
		const user = userEvent.setup();
		const counts = [{ photo_id: 'p2', count: 1 }];
		const summary = vi
			.fn(() => Response.json(counts))
			.mockReturnValueOnce(Response.json(counts))
			.mockReturnValueOnce(Response.json({}, { status: 503 }));
		stub(counts, summary);
		let refresh: Parameters<LiveEvents['onNotification']>[0] | undefined;
		render(Harness, {
			props: {
				events: {
					...events,
					onNotification: (handler) => {
						refresh = handler;
						return () => {};
					}
				}
			}
		});

		await user.click(await screen.findByRole('button', { name: /p2\.jpg/ }));
		await user.type(await screen.findByRole('textbox'), 'Try a lighter version');
		refresh?.({ gallery_id: 'g1' });
		expect(await screen.findByRole('alert')).toBeTruthy();
		expect(screen.getByRole('textbox')).toHaveProperty('value', 'Try a lighter version');
	});

	it('explains when no photo has comments', async () => {
		stub([]);
		render(Harness, { props: { events } });

		const heading = await screen.findByText('No comments yet');
		expect(heading.closest('[data-slot="empty-state"]')?.className).not.toContain('border-dashed');
	});
});
