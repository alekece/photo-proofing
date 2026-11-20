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

const page = (events: ActivityEvent[], next: string | null = null, tail = next) =>
	Response.json({ events, next, tail });

const asFetch = (fetcher: unknown) => fetcher as typeof fetch;
const ids = (feed: ActivityFeed) => feed.events.map(({ id }) => id);
const params = (url: string) => Object.fromEntries(new URL(url, 'http://localhost').searchParams);

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

	it('keeps sort with its cursor and discards a late page from the previous order', async () => {
		let answer!: (response: Response) => void;
		const oldest = line('oldest', '2026-11-09T10:00:00Z');
		const newest = line('newest', '2026-11-10T10:00:00Z');
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([newest], 'newest-cursor'))
			.mockImplementationOnce(() => new Promise<Response>((resolve) => (answer = resolve)))
			.mockResolvedValueOnce(page([oldest], 'oldest-cursor'))
			.mockResolvedValueOnce(page([newest]));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		await feed.load();
		const pending = feed.loadMore();
		feed.sort = 'oldest';
		await feed.load();
		await feed.loadMore();
		answer(page([line('discard', '2026-11-01T10:00:00Z')]));
		await pending;

		expect(fetcher.mock.calls[2]?.[0]).toBe('/api/v1/timeline?sort=oldest');
		expect(fetcher.mock.calls[3]?.[0]).toBe('/api/v1/timeline?sort=oldest&cursor=oldest-cursor');
		expect(ids(feed)).toEqual(['oldest', 'newest']);
	});

	it('does not append a disconnected newest page to an incomplete oldest-first list', async () => {
		const oldest = line('oldest', '2026-11-09T10:00:00Z');
		const newest = line('newest', '2026-11-10T10:00:00Z');
		const updated = { ...oldest, count: 2 };
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([oldest], 'oldest-cursor'))
			.mockResolvedValueOnce(page([newest, updated], 'unrelated-cursor'))
			.mockResolvedValueOnce(page([newest], null, 'end-of-loaded-list'))
			.mockResolvedValueOnce(page([line('later', '2026-11-11T10:00:00Z')]))
			.mockResolvedValueOnce(page([line('later', '2026-11-11T10:00:00Z')]));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		feed.sort = 'oldest';
		await feed.load();
		await feed.refresh();

		expect(feed.events).toEqual([updated]);
		expect(feed.next).toBe('oldest-cursor');
		await feed.loadMore();
		await feed.refresh();
		expect(ids(feed)).toEqual(['oldest', 'newest', 'later']);
		expect(feed.next).toBeNull();
	});

	it.each(['filter', 'sort'])(
		'drops failed %s generation work without clearing the active page',
		async (change) => {
			const outgoing = Promise.withResolvers<Response>();
			const replacement = Promise.withResolvers<Response>();
			const incoming = Promise.withResolvers<Response>();
			const fetcher = vi
				.fn()
				.mockResolvedValueOnce(page([line('a', '2026-11-10T10:00:00Z')], 'a-tail'))
				.mockReturnValueOnce(outgoing.promise)
				.mockReturnValueOnce(replacement.promise)
				.mockReturnValueOnce(incoming.promise);
			const feed = new ActivityFeed({ fetcher });

			await feed.load();
			const oldPage = feed.loadMore();
			if (change === 'filter') feed.filters = { kind: 'favorite' };
			else feed.sort = 'oldest';
			const loading = feed.load();

			expect(ids(feed)).toEqual(['a']);
			expect(feed.canPage).toBe(false);
			await feed.loadMore();
			expect(fetcher).toHaveBeenCalledTimes(3);

			replacement.resolve(page([line('b', '2026-11-10T09:00:00Z')], 'b-tail'));
			await loading;
			const newPage = feed.loadMore();
			outgoing.reject(new Error('outgoing request failed'));
			await oldPage;

			expect(feed.next).toBe('b-tail');
			expect(feed.loadingMore).toBe(true);
			expect(feed.paging).toBe('loading');
			expect(feed.failed).toBe(false);
			expect(ids(feed)).toEqual(['b']);

			incoming.resolve(page([]));
			await newPage;
			expect(feed.canPage).toBe(false);
		}
	);

	it('keeps sort and filters on pages and retry, and keeps the parent newest live lookup', async () => {
		const first = line('a', '2026-11-10T10:00:00Z');
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([first]))
			.mockResolvedValueOnce(page([first], 'opaque+/='))
			.mockResolvedValueOnce(page([first]))
			.mockResolvedValueOnce(new Response(null, { status: 503 }))
			.mockResolvedValueOnce(page([]))
			.mockResolvedValueOnce(page([first]));
		const feed = new ActivityFeed({ fetcher });

		await feed.load();
		feed.sort = 'oldest';
		feed.filters = { kind: 'favorite' };
		await feed.load();
		await feed.refresh();
		await feed.loadMore();
		expect(feed.paging).toBe('failed');
		await feed.loadMore();
		feed.sort = 'newest';
		await feed.load();

		expect(fetcher.mock.calls.map(([url]) => params(url))).toEqual([
			{},
			{ kind: 'favorite', sort: 'oldest' },
			{ kind: 'favorite' },
			{ kind: 'favorite', sort: 'oldest', cursor: 'opaque+/=' },
			{ kind: 'favorite', sort: 'oldest', cursor: 'opaque+/=' },
			{ kind: 'favorite' }
		]);
	});

	it('resumes an exhausted oldest-first list without skipping past newly available pages', async () => {
		const previous = line('previous', '2026-11-09T10:00:00Z');
		const firstNew = line('first-new', '2026-11-10T10:00:00Z');
		const lastNew = line('last-new', '2026-11-10T12:00:00Z');
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([previous], null, 'end-of-loaded-list'))
			.mockResolvedValueOnce(page([lastNew], 'newest-cursor'))
			.mockImplementationOnce(async (url: string) =>
				url.includes('sort=oldest&cursor=end-of-loaded-list')
					? page([firstNew], 'more-new', 'more-new')
					: page([])
			)
			.mockResolvedValueOnce(page([lastNew], null, 'new-end'));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		feed.sort = 'oldest';
		await feed.load();
		await feed.refresh();

		expect(ids(feed)).toEqual(['previous', 'first-new']);
		expect(feed.next).toBe('more-new');
		await feed.loadMore();
		expect(ids(feed)).toEqual(['previous', 'first-new', 'last-new']);
		expect(feed.next).toBeNull();
	});

	it('catches up when a notification arrives before the last page finishes', async () => {
		const first = line('first', '2026-11-09T10:00:00Z');
		const last = line('last', '2026-11-10T10:00:00Z');
		const later = line('later', '2026-11-11T10:00:00Z');
		let answerPage!: (response: Response) => void;
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([first], 'after-first'))
			.mockImplementationOnce(() => new Promise<Response>((resolve) => (answerPage = resolve)))
			.mockResolvedValueOnce(page([later, last, first]))
			.mockImplementationOnce(async (url: string) =>
				url.includes('sort=oldest&cursor=after-last')
					? page([later], null, 'after-later')
					: page([])
			);
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		feed.sort = 'oldest';
		await feed.load();
		const paging = feed.loadMore();
		const refreshing = feed.refresh();
		answerPage(page([last], null, 'after-last'));

		await Promise.all([paging, refreshing]);
		expect(ids(feed)).toEqual(['first', 'last', 'later']);
	});

	it('reconciles an updated contact and late-arriving rows behind the oldest-first boundary', async () => {
		const last = line('last', '2026-11-10T10:00:00Z', { contact: { id: 'c1', name: 'Alice' } });
		const late = line('late', '2026-11-09T10:00:00Z');
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([last], null, 'end-of-loaded-list'))
			.mockImplementationOnce(async (url: string) =>
				url.includes('cursor=') ? page([]) : page([{ ...last, contact: null }, late])
			);
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		feed.sort = 'oldest';
		await feed.load();
		await feed.refresh();

		expect(ids(feed)).toEqual(['late', 'last']);
		expect(feed.events.at(-1)?.contact).toBeNull();
	});

	it('still reads forward from the oldest boundary when a loaded line moves past it', async () => {
		const last = line('last', '2026-11-10T10:00:00Z');
		const moved = { ...last, last_at: '2026-11-12T10:00:00Z', count: 2 };
		const unseen = line('unseen', '2026-11-11T10:00:00Z');
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([last], null, 'after-last'))
			.mockResolvedValueOnce(page([moved]))
			.mockResolvedValueOnce(page([unseen, moved], null, 'after-moved'));
		const feed = new ActivityFeed({ fetcher });

		feed.sort = 'oldest';
		await feed.load();
		await feed.refresh();

		expect(feed.events).toEqual([unseen, moved]);
		expect(feed.next).toBeNull();
	});

	it('does not restore an outdated contact name when the newest read overlaps paging', async () => {
		const first = line('first', '2026-11-09T10:00:00Z');
		const last = line('last', '2026-11-10T10:00:00Z', { contact: { id: 'c1', name: 'Alice' } });
		const renamed = { ...last, contact: { id: 'c1', name: 'Alex' } };
		let answer!: (response: Response) => void;
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([first], 'after-first'))
			.mockImplementationOnce(() => new Promise<Response>((resolve) => (answer = resolve)))
			.mockResolvedValue(page([renamed], null, 'after-last'));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		feed.sort = 'oldest';
		await feed.load();
		const refreshing = feed.refresh();
		const paging = feed.loadMore();
		answer(page([last, first]));
		await Promise.all([paging, refreshing]);

		expect(ids(feed)).toEqual(['first', 'last']);
		expect(feed.events.at(-1)?.contact?.name).toBe('Alex');
	});

	it('keeps a failed page visible through a successful oldest-first refresh until retry', async () => {
		const first = line('first', '2026-11-09T10:00:00Z');
		const last = line('last', '2026-11-10T10:00:00Z');
		let answer!: (response: Response) => void;
		const fetcher = vi.fn(async (url: string) => {
			if (!url.includes('sort=')) return page([first]);
			if (!url.includes('cursor=')) return page([first], 'after-first');

			return page([last]);
		});
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		feed.sort = 'oldest';
		await feed.load();
		fetcher.mockImplementationOnce(() => new Promise<Response>((resolve) => (answer = resolve)));
		const paging = feed.loadMore();
		const refreshing = feed.refresh();
		answer(new Response(null, { status: 503 }));
		await Promise.all([paging, refreshing]);

		expect(feed.paging).toBe('failed');
		expect(feed.failed).toBe(false);
		expect(feed.next).toBe('after-first');
		await feed.loadMore();
		expect(feed.paging).toBe('idle');
		expect(ids(feed)).toEqual(['first', 'last']);
	});

	it('keeps newest-first paging retryable when a refresh no longer has a next page', async () => {
		const first = line('first', '2026-11-10T10:00:00Z');
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([first], 'after-first'))
			.mockResolvedValueOnce(new Response(null, { status: 503 }))
			.mockResolvedValueOnce(page([first]))
			.mockResolvedValueOnce(page([]));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		await feed.load();
		await feed.loadMore();
		await feed.refresh();

		expect(feed.paging).toBe('failed');
		expect(feed.failed).toBe(false);
		expect(feed.next).toBe('after-first');
		await feed.loadMore();
		expect(feed.paging).toBe('idle');
		expect(fetcher.mock.calls[1]?.[0]).toBe(fetcher.mock.calls[3]?.[0]);
		expect(feed.next).toBeNull();
	});

	it('leaves a nonadvancing page retryable without appending its rows', async () => {
		const first = line('a', '2026-11-10T10:00:00Z');
		const second = line('b', '2026-11-09T10:00:00Z');
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([first], 'c1'))
			.mockResolvedValueOnce(page([second], 'c1'))
			.mockResolvedValueOnce(page([second]));
		const feed = new ActivityFeed({ fetcher });

		await feed.load();
		await feed.loadMore();
		expect(ids(feed)).toEqual(['a']);
		expect(feed.paging).toBe('failed');
		expect(feed.canPage).toBe(true);
		await feed.loadMore();
		expect(ids(feed)).toEqual(['a', 'b']);
		expect(feed.canPage).toBe(false);
	});

	it('recovers the requested sort on resync after a failed first page', async () => {
		const recent = line('recent', '2026-11-10T10:00:00Z');
		const earlier = line('earlier', '2026-11-09T10:00:00Z');
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([recent], 'newest-cursor'))
			.mockResolvedValueOnce(new Response(null, { status: 503 }))
			.mockResolvedValueOnce(page([earlier], 'oldest-cursor'));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		await feed.load();
		feed.sort = 'oldest';
		await feed.load();

		expect(ids(feed)).toEqual(['recent']);
		expect(feed.next).toBeNull();
		expect(feed.canPage).toBe(false);
		await feed.loadMore();
		expect(fetcher).toHaveBeenCalledTimes(2);
		await feed.refresh();
		expect(fetcher).toHaveBeenLastCalledWith('/api/v1/timeline?sort=oldest', expect.anything());
		expect(ids(feed)).toEqual(['earlier']);
		expect(feed.failed).toBe(false);
	});

	it('preserves chronological order across a fractional-second page boundary', async () => {
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([line('whole', '2026-11-10T10:00:00Z')], 'c1'))
			.mockResolvedValueOnce(
				page([
					line('z-earlier', '2026-11-10T10:00:00.123456Z'),
					line('a-later', '2026-11-10T10:00:00.123457Z')
				])
			);
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		feed.sort = 'oldest';
		await feed.load();
		await feed.loadMore();

		expect(ids(feed)).toEqual(['whole', 'z-earlier', 'a-later']);
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

	it.each([
		{ sort: 'newest', tail: null },
		{ sort: 'oldest', tail: 'after-first' },
		{ sort: 'oldest', tail: null }
	] as const)(
		'accepts an authoritative $sort refresh with tail $tail when the timestamp goes backward',
		async ({ sort, tail }) => {
			const first = line('a', '2026-11-10T10:00:00Z', {
				first_at: '2026-11-10T07:00:00Z',
				photo_ids: ['p1']
			});
			const second = line('b', '2026-11-10T09:00:00Z');
			const changed = {
				...first,
				last_at: '2026-11-10T08:00:00Z',
				count: 2,
				photo_ids: ['p1', 'p2']
			};
			const fetcher = vi
				.fn()
				.mockImplementation(async (url: string) =>
					params(url).sort === 'oldest' ? page([changed, second]) : page([second, changed])
				)
				.mockResolvedValueOnce(
					page(sort === 'oldest' ? [second, first] : [first, second], null, tail)
				);
			const feed = new ActivityFeed({ fetcher });

			feed.sort = sort;
			await feed.load();
			await feed.refresh();

			expect(feed.events).toEqual(sort === 'oldest' ? [changed, second] : [second, changed]);
		}
	);

	it.each(['refresh first', 'page first'])(
		'sequences newest-first reads and keeps the requested cursor: %s',
		async (order) => {
			const first = line('a', '2026-11-10T10:00:00Z');
			const second = line('b', first.last_at);
			const newest = line('n', '2026-11-10T13:00:00Z');
			const pending = Promise.withResolvers<Response>();
			const changed = { ...first, count: 3, last_at: '2026-11-10T09:00:00Z' };
			const refreshFirst = order === 'refresh first';
			const fetcher = vi
				.fn()
				.mockResolvedValueOnce(page([first], 'c1'))
				.mockReturnValueOnce(pending.promise)
				.mockImplementationOnce(async (url: string) =>
					url.includes('cursor=c1')
						? page([second, changed], 'c2')
						: page([newest, second, changed])
				);
			const feed = new ActivityFeed({ fetcher });
			const { handlers, events } = liveEvents();
			const refreshing = vi.spyOn(feed, 'refresh');
			const stop = feed.start(events);

			await feed.load();

			const notify = () => handlers.notification[0]!({ gallery_id: 'g1' });
			if (refreshFirst) notify();
			const paging = feed.loadMore();
			if (!refreshFirst) notify();

			expect(feed.next).toBe('c1');
			expect(fetcher.mock.calls.map(([url]) => params(url))).toEqual([
				{},
				refreshFirst ? {} : { cursor: 'c1' }
			]);
			pending.resolve(
				refreshFirst ? page([newest, first]) : page([{ ...first, count: 2 }, second], 'c2')
			);

			await Promise.all([paging, refreshing.mock.results[0]!.value]);
			expect(ids(feed)).toEqual(['n', 'b', 'a']);
			expect(feed.events.at(-1)).toEqual(changed);
			expect(feed.next).toBe('c2');
			stop();
		}
	);

	it('does not clear a live refresh error when a page succeeds', async () => {
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([line('a', '2026-11-10T10:00:00Z')], 'c1'))
			.mockResolvedValueOnce(new Response(null, { status: 503 }))
			.mockResolvedValueOnce(page([line('b', '2026-11-09T10:00:00Z')]));
		const feed = new ActivityFeed({ fetcher });

		await feed.load();
		await feed.refresh();
		await feed.loadMore();
		expect(ids(feed)).toEqual(['a', 'b']);
		expect(feed.failed).toBe(true);
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

	it('discards a refresh waiting on a superseded filter load', async () => {
		const previous = line('previous', '2026-11-09T10:00:00Z');
		const first = line('first', '2026-11-10T10:00:00Z', { kind: 'favorite' });
		const last = line('last', '2026-11-11T10:00:00Z', { contact: { id: 'c1', name: 'Alex' } });
		const stale = { ...last, contact: { id: 'c1', name: 'Alice' } };
		let answer!: (response: Response) => void;
		const fetcher = vi
			.fn()
			.mockImplementationOnce(() => new Promise<Response>((resolve) => (answer = resolve)))
			.mockResolvedValueOnce(page([first], 'after-first'))
			.mockResolvedValueOnce(page([last], null, 'after-last'))
			.mockResolvedValue(page([stale, first]));
		const feed = new ActivityFeed({ fetcher: asFetch(fetcher) });

		feed.sort = 'oldest';
		const loading = feed.load();
		const refreshing = feed.refresh();
		feed.filters = { kind: 'favorite' };
		await feed.load();
		await feed.loadMore();
		answer(page([previous]));
		await Promise.all([loading, refreshing]);

		expect(ids(feed)).toEqual(['first', 'last']);
		expect(feed.events.at(-1)?.contact?.name).toBe('Alex');
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
