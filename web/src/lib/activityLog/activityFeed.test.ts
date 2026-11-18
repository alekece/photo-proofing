import { describe, expect, it, vi } from 'vitest';
import type { LiveEvents } from '#lib/events.js';
import { ActivityFeed } from './activityFeed.svelte';
import type { ActivityEvent } from './api';

function line(id: string, lastAt: string, over: Partial<ActivityEvent> = {}): ActivityEvent {
	return {
		id,
		gallery_id: 'g1',
		gallery_title: 'Wedding',
		contact: null,
		kind: 'favorite',
		count: 1,
		photo_ids: [],
		detail: null,
		first_at: lastAt,
		last_at: lastAt,
		...over
	};
}

const page = (events: ActivityEvent[], next: string | null = null) =>
	Response.json({ events, next });

const asFetch = (fetcher: unknown) => fetcher as typeof fetch;
const ids = (feed: ActivityFeed) => feed.events.map(({ id }) => id);

function liveEvents() {
	const handlers = {
		notification: [] as ((event: { gallery_id: string }) => void)[],
		resync: [] as (() => void)[]
	};
	const events = {
		onNotification: (handler: (event: { gallery_id: string }) => void) => {
			handlers.notification.push(handler);

			return () => handlers.notification.splice(handlers.notification.indexOf(handler), 1);
		},
		onResync: (handler: () => void) => {
			handlers.resync.push(handler);

			return () => handlers.resync.splice(handlers.resync.indexOf(handler), 1);
		}
	};

	return { handlers, events: events as unknown as LiveEvents };
}

describe('ActivityFeed', () => {
	it('loads the first page with the filters as query parameters', async () => {
		const fetcher = vi.fn<(url: string) => Promise<Response>>(async () =>
			page([line('a', '2026-11-10T10:00:00Z')], 'cursor-1')
		);
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		feed.filters = { gallery: 'g1', contact: 'anonymous', kind: 'comment', range: '7d' };
		await feed.load();

		expect(fetcher.mock.calls[0]?.[0]).toBe(
			'/api/v1/timeline?gallery=g1&contact=anonymous&kind=comment&range=7d'
		);
		expect(ids(feed)).toEqual(['a']);
		expect(feed.next).toBe('cursor-1');
	});

	it('scopes a gallery feed to its own route', async () => {
		const fetcher = vi.fn<(url: string) => Promise<Response>>(async () => page([]));
		const feed = new ActivityFeed({ galleryId: 'g 1', fetcher: asFetch(fetcher) });

		await feed.load();

		expect(fetcher.mock.calls[0]?.[0]).toBe('/api/v1/galleries/g%201/timeline');
	});

	it('appends the next page without repeating a line', async () => {
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([line('a', '2026-11-10T10:00:00Z')], 'c1'))
			.mockResolvedValueOnce(
				page([line('a', '2026-11-10T10:00:00Z'), line('b', '2026-11-09T10:00:00Z')])
			);
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		await feed.load();
		await feed.loadMore();

		expect(fetcher.mock.calls[1]?.[0]).toBe('/api/v1/timeline?cursor=c1');
		expect(ids(feed)).toEqual(['a', 'b']);
		expect(feed.next).toBeNull();
	});

	it('refreshes by merging: loaded pages stay and the list is never emptied', async () => {
		let answer!: (response: Response) => void;
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([line('a', '2026-11-10T10:00:00Z')], 'c1'))
			.mockResolvedValueOnce(page([line('b', '2026-11-09T10:00:00Z')]))
			.mockImplementationOnce(() => new Promise<Response>((resolve) => (answer = resolve)));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		await feed.load();
		await feed.loadMore();

		const refreshing = feed.refresh();

		expect(ids(feed)).toEqual(['a', 'b']);

		answer(
			page(
				[line('n', '2026-11-10T12:00:00Z'), line('a', '2026-11-10T11:00:00Z', { count: 3 })],
				'c9'
			)
		);
		await refreshing;

		expect(ids(feed)).toEqual(['n', 'a', 'b']);
		expect(feed.events.find(({ id }) => id === 'a')?.count).toBe(3);
		expect(feed.next).toBeNull();
	});

	it('keeps the old list while a filter change loads, then replaces it', async () => {
		let answer!: (response: Response) => void;
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([line('a', '2026-11-10T10:00:00Z')]))
			.mockImplementationOnce(() => new Promise<Response>((resolve) => (answer = resolve)));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		await feed.load();
		feed.filters = { kind: 'download' };

		const loading = feed.load();

		expect(feed.loading).toBe(true);
		expect(ids(feed)).toEqual(['a']);

		answer(page([line('d', '2026-11-10T09:00:00Z', { kind: 'download' })]));
		await loading;

		expect(feed.loading).toBe(false);
		expect(ids(feed)).toEqual(['d']);
	});

	it('ignores an answer that arrives after a newer request', async () => {
		const answers: ((response: Response) => void)[] = [];
		const fetcher = vi.fn(() => new Promise<Response>((resolve) => answers.push(resolve)));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		const first = feed.load();
		const second = feed.load();

		answers[1]!(page([line('new', '2026-11-10T10:00:00Z')]));
		await second;
		answers[0]!(page([line('old', '2026-11-09T10:00:00Z')]));
		await first;

		expect(ids(feed)).toEqual(['new']);
	});

	it('applies a filter change even when a live refresh arrives while it loads', async () => {
		const fetcher = vi.fn<(url: string) => Promise<Response>>(async (url) =>
			url.includes('kind=comment')
				? page([line('c', '2026-11-10T09:00:00Z', { kind: 'comment' })])
				: page([line('f', '2026-11-10T10:00:00Z')])
		);
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		await feed.load();
		feed.filters = { kind: 'comment' };

		const loading = feed.load();
		const refreshing = feed.refresh();

		await Promise.all([loading, refreshing]);

		expect(ids(feed)).toEqual(['c']);
		expect(feed.loading).toBe(false);
	});

	it('keeps what it shows when a refresh fails', async () => {
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([line('a', '2026-11-10T10:00:00Z')]))
			.mockResolvedValueOnce(new Response('nope', { status: 500 }));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		await feed.load();
		await feed.refresh();

		expect(ids(feed)).toEqual(['a']);
		expect(feed.failed).toBe(true);
	});

	it('refreshes on a notification for its gallery and on a resync, and stops on request', async () => {
		const fetcher = vi.fn(async () => page([]));
		const { handlers, events } = liveEvents();
		const feed = new ActivityFeed({ galleryId: 'g1', fetcher: asFetch(fetcher) });
		const stop = feed.start(events);

		handlers.notification[0]!({ gallery_id: 'other' });
		expect(fetcher).not.toHaveBeenCalled();

		handlers.notification[0]!({ gallery_id: 'g1' });
		handlers.resync[0]!();
		expect(fetcher).toHaveBeenCalledTimes(2);

		stop();
		expect(handlers.notification).toHaveLength(0);
		expect(handlers.resync).toHaveLength(0);
	});
});
