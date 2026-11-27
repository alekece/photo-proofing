import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
import type { ActivityEvent } from '#lib/activityLog/api.js';
import { useLocale } from '#lib/i18n/testing.js';
import ActivityTimeline from './ActivityTimeline.svelte';

const at = (day: number, hour: number) => new Date(2026, 10, day, hour).toISOString();

function line(id: string, over: Partial<ActivityEvent> = {}): ActivityEvent {
	return {
		id,
		gallery_id: 'g1',
		gallery_title: 'Wedding',
		contact: { id: 'c1', name: 'Alice' },
		kind: 'favorite',
		count: 3,
		photo_ids: ['p1', 'p2'],
		detail: null,
		first_at: at(10, 9),
		last_at: at(10, 10),
		...over
	};
}

function feedWith(events: ActivityEvent[], next: string | null = null) {
	const feed = new ActivityFeed({ fetcher: vi.fn() as unknown as typeof fetch });

	feed.events = events;
	feed.next = next;
	feed.loaded = true;

	return feed;
}

const page = (events: ActivityEvent[], next: string | null) => Response.json({ events, next });

function stubObserver() {
	const observers: Observer[] = [];

	class Observer {
		target!: Element;
		disconnect = vi.fn();

		constructor(
			private callback: IntersectionObserverCallback,
			readonly options: IntersectionObserverInit
		) {
			observers.push(this);
		}

		observe(target: Element) {
			this.target = target;
		}

		intersect(isIntersecting = true) {
			this.callback(
				[{ target: this.target, isIntersecting } as IntersectionObserverEntry],
				this as unknown as IntersectionObserver
			);
		}
	}

	vi.stubGlobal('IntersectionObserver', Observer);
	return observers;
}

beforeEach(() => {
	vi.useFakeTimers({ toFake: ['Date'] });
	vi.setSystemTime(new Date(2026, 10, 10, 15));
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('ActivityTimeline', () => {
	useLocale('en');

	it.each([true, false])(
		'requeues just the failed recipient without resetting the feed (workspace: %s)',
		async (showGallery) => {
			const failed = line('failed', { kind: 'email_failed', contact: null, can_retry: true });
			const kept = line('other');
			const fetcher = vi.fn(async (_url: string, init?: RequestInit) =>
				init?.method === 'POST' ? new Response(null, { status: 204 }) : page([], null)
			);
			const feed = new ActivityFeed({
				galleryId: showGallery ? undefined : 'g1',
				fetcher: fetcher as typeof fetch
			});
			feed.events = [failed, kept];
			feed.loaded = true;
			render(ActivityTimeline, { feed, showGallery });
			expect(screen.queryByRole('link', { name: /invitation could not be sent/ })).toBeNull();
			await userEvent.click(screen.getByRole('button', { name: 'Retry' }));
			expect(
				fetcher.mock.calls.filter(([, init]) => init?.method === 'POST').map(([url]) => url)
			).toEqual(['/api/v1/galleries/g1/timeline/failed/retry']);
			expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull();
			expect(feed.events.map((e) => e.id)).toContain('other');
			expect(screen.getByText('An invitation could not be sent.')).toBeTruthy();
		}
	);

	it('words each kind, names the client and links the gallery', () => {
		render(ActivityTimeline, {
			feed: feedWith([
				line('a'),
				line('b', { kind: 'unfavorite', count: 1, contact: null, photo_ids: [] }),
				line('c', { kind: 'comment', count: 2 }),
				line('d', { kind: 'download', count: 4 }),
				line('e', { kind: 'download', count: 0 }),
				line('f', { kind: 'open', count: 1 }),
				line('g', {
					kind: 'email',
					count: 1,
					detail: 'invitation',
					contact: { id: 'c1', name: '<b>Alice</b>' }
				})
			])
		});

		const rows = screen.getAllByRole('listitem');
		expect(rows[0]?.querySelector('svg')?.getAttribute('class')).toContain('text-danger');
		const paragraphs = rows.map((row) => row.querySelector('p'));
		expect(paragraphs.map((p) => p?.textContent?.trim().replace(/\s+/g, ' '))).toEqual([
			'Alice favorited 3 photos',
			'Anonymous client un-picked a photo',
			'Alice left 2 comments',
			'Alice downloaded 4 photos',
			'Alice downloaded the ZIP',
			'Alice opened the gallery',
			'Invitation email sent to <b>Alice</b>'
		]);
		expect(paragraphs.map((p) => p?.querySelector('strong')?.textContent)).toEqual([
			'Alice',
			'Anonymous client',
			'Alice',
			'Alice',
			'Alice',
			'Alice',
			'<b>Alice</b>'
		]);
		expect(paragraphs[6]?.querySelector('b')).toBeNull();
		for (const [index, row] of rows.entries()) {
			const gallery = within(row).getByRole('link', { name: 'Wedding' });
			expect(paragraphs[index]?.compareDocumentPosition(gallery)).toBe(
				Node.DOCUMENT_POSITION_FOLLOWING
			);
			expect(row.querySelector('time')).toBeNull();
			expect(row.querySelector('svg[aria-hidden="true"]')).not.toBeNull();
			expect(
				within(row).queryByRole('img', { name: /favorite|open|comment|download|email/i })
			).toBeNull();
		}
		expect(screen.getAllByRole('link', { name: 'Wedding' })[0]?.getAttribute('href')).toBe(
			'/dashboard/g1'
		);
	});

	it('leads each line to the tab and thread that hold what it describes', () => {
		render(ActivityTimeline, {
			feed: feedWith([
				line('a'),
				line('b', {
					kind: 'comment',
					count: 2,
					photo_ids: ['p9', 'p9', 'p3'],
					detail: 'Warmer, please'
				}),
				line('c', { kind: 'download', count: 2 }),
				line('d', { kind: 'email', photo_ids: [] }),
				line('e', { kind: 'open', count: 1, photo_ids: [] })
			])
		});

		const rows = screen.getAllByRole('listitem');
		const hrefs = rows.map((row) =>
			within(row)
				.getAllByRole('link')
				.map((link) => link.getAttribute('href'))
		);

		expect(hrefs).toEqual([
			['/dashboard/g1?section=favorites', '/dashboard/g1'],
			['/dashboard/g1?section=comments&thread=p9', '/dashboard/g1'],
			['/dashboard/g1?section=final', '/dashboard/g1'],
			['/dashboard/g1?section=contacts', '/dashboard/g1'],
			['/dashboard/g1', '/dashboard/g1']
		]);
		expect(within(rows[1]!).getByText(/Warmer, please/)).toBeTruthy();
		expect(within(rows[1]!).getAllByRole('img')).toHaveLength(1);
		expect(within(rows[0]!).queryByText(/Warmer, please/)).toBeNull();
	});

	it('leaves an open line unlinked inside a gallery’s own feed', () => {
		render(ActivityTimeline, {
			feed: feedWith([line('a', { kind: 'open', count: 1, photo_ids: [] })]),
			showGallery: false
		});

		expect(screen.queryByRole('link')).toBeNull();
	});

	it('shows the photos of a line as thumbnails', () => {
		render(ActivityTimeline, { feed: feedWith([line('a')]) });

		const photos = screen.getAllByRole('img', { name: 'Photo' });

		expect(photos.map((photo) => photo.getAttribute('src'))).toEqual([
			expect.stringContaining('/galleries/g1/photos/p1'),
			expect.stringContaining('/galleries/g1/photos/p2')
		]);
	});

	it.each([10, 6])('collapses at %i photos and only toggles its own line', async (limit) => {
		vi.stubGlobal('matchMedia', (query: string) => ({
			matches: query === '(prefers-reduced-motion: reduce)' || limit === 6,
			addEventListener: vi.fn(),
			removeEventListener: vi.fn()
		}));
		const photos = Array.from({ length: 12 }, (_, i) => `p${i}`);
		render(ActivityTimeline, {
			feed: feedWith([
				line('a', { photo_ids: [...photos, photos[0]!] }),
				line('b', { kind: 'unfavorite', photo_ids: photos }),
				line('c', { photo_ids: photos.slice(0, limit) }),
				line('d', { kind: 'download', photo_ids: photos.slice(0, 4) })
			]),
			showGallery: false
		});
		const [first, second, exact, download] = screen
			.getAllByRole('listitem')
			.map((row) => within(row));
		const toggle = first!.getByRole('button', { name: `${12 - limit} more photos` });
		const user = userEvent.setup();

		expect(first!.getAllByRole('img')).toHaveLength(limit);
		expect(second!.getAllByRole('img')).toHaveLength(limit);
		expect(exact!.getAllByRole('img')).toHaveLength(limit);
		expect(exact!.queryByRole('button')).toBeNull();
		expect(download!.getAllByRole('img')).toHaveLength(4);
		expect(download!.queryByRole('button')).toBeNull();
		expect(screen.queryByRole('link', { name: 'Wedding' })).toBeNull();
		expect(toggle.textContent?.trim()).toBe(`+${12 - limit}`);
		expect(toggle.getAttribute('aria-expanded')).toBe('false');
		expect(first!.queryByRole('region')).toBeNull();

		toggle.focus();
		await user.keyboard('{Enter}');
		const panel = first!.getByRole('region', { name: `${12 - limit} more photos` });
		expect(
			within(panel)
				.getAllByRole('img')
				.map((img) => img.getAttribute('src'))
		).toEqual(photos.slice(limit).map((id) => `/api/v1/galleries/g1/photos/${id}/thumbnail`));
		expect(second!.getAllByRole('img')).toHaveLength(limit);
		expect(toggle.getAttribute('aria-expanded')).toBe('true');

		await user.tab();
		expect(document.activeElement).toBe(panel);
		await user.keyboard('{Escape}');
		expect(first!.getAllByRole('img')).toHaveLength(limit);
		expect(document.activeElement).toBe(toggle);
	});

	it('keeps the focused phone disclosure ahead of a long expanded grid', async () => {
		vi.stubGlobal('matchMedia', () => ({
			matches: true,
			addEventListener: vi.fn(),
			removeEventListener: vi.fn()
		}));
		const photos = Array.from({ length: 60 }, (_, i) => `p${i}`);
		render(ActivityTimeline, {
			feed: feedWith([line('a', { photo_ids: photos })]),
			showGallery: false
		});
		const user = userEvent.setup();
		await user.tab();
		expect(document.activeElement?.tagName).toBe('A');
		await user.tab();
		const toggle = screen.getByRole('button', { name: '54 more photos' });
		expect(document.activeElement).toBe(toggle);

		await user.keyboard('{Enter}');
		const panel = screen.getByRole('region', { name: '54 more photos' });
		expect(within(panel).getAllByRole('img')).toHaveLength(54);
		expect(document.activeElement).toBe(toggle);

		await user.keyboard(' ');
		expect(screen.getAllByRole('img')).toHaveLength(6);
		expect(document.activeElement).toBe(toggle);
	});

	it('groups lines under Today, Yesterday and a dated heading', () => {
		render(ActivityTimeline, {
			feed: feedWith([
				line('a'),
				line('b', { last_at: at(9, 18) }),
				line('c', { last_at: at(2, 12) })
			])
		});

		expect(screen.getAllByRole('heading', { level: 3 }).map((h) => h.textContent)).toEqual([
			'Today',
			'Yesterday',
			expect.stringContaining('2 Nov 2026')
		]);
	});

	it('observes 400px early, appends each cursor once, and stops when exhausted', async () => {
		const observers = stubObserver();
		const next = Promise.withResolvers<Response>();
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([line('a')], 'c1'))
			.mockReturnValueOnce(next.promise)
			.mockResolvedValueOnce(page([line('c')], null));
		const feed = new ActivityFeed({ fetcher });
		await feed.load();
		const { unmount } = render(ActivityTimeline, { feed });

		expect(observers[0]?.options).toMatchObject({ root: null, rootMargin: '0px 0px 400px 0px' });
		expect(screen.queryByRole('button', { name: 'Load more' })).toBeNull();
		observers[0]!.intersect(false);
		expect(fetcher).toHaveBeenCalledTimes(1);
		observers[0]!.intersect();
		observers[0]!.intersect();
		expect(fetcher).toHaveBeenCalledTimes(2);
		expect(fetcher.mock.calls[1]?.[0]).toBe('/api/v1/timeline?cursor=c1');
		expect(await screen.findByRole('status')).toHaveProperty(
			'textContent',
			expect.stringContaining('Loading activity')
		);
		expect(observers[0]!.disconnect).toHaveBeenCalledOnce();

		next.resolve(page([line('b')], 'c2'));
		await waitFor(() => expect(observers).toHaveLength(2));
		observers[0]!.intersect();
		expect(fetcher).toHaveBeenCalledTimes(2);
		observers[1]!.intersect();
		await waitFor(() => expect(screen.getAllByRole('listitem')).toHaveLength(3));
		expect(fetcher.mock.calls[2]?.[0]).toBe('/api/v1/timeline?cursor=c2');
		expect(observers[1]!.disconnect).toHaveBeenCalledOnce();
		expect(observers).toHaveLength(2);
		unmount();
		observers[1]!.intersect();
		expect(fetcher).toHaveBeenCalledTimes(3);
	});

	it('keeps a failed sentinel local until keyboard Retry appends the same cursor', async () => {
		const observers = stubObserver();
		const retrying = Promise.withResolvers<Response>();
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([line('a')], 'c1'))
			.mockResolvedValueOnce(new Response(null, { status: 503 }))
			.mockReturnValueOnce(retrying.promise);
		const feed = new ActivityFeed({ fetcher });
		await feed.load();
		render(ActivityTimeline, { feed });

		expect(observers).toHaveLength(1);
		observers[0]!.intersect();
		expect((await screen.findByRole('alert')).textContent).toContain(
			'Could not load more activity'
		);
		expect(screen.getAllByRole('listitem')).toHaveLength(1);
		observers[0]!.intersect();
		expect(observers).toHaveLength(1);
		expect(fetcher).toHaveBeenCalledTimes(2);

		const retry = screen.getByRole('button', { name: 'Retry' });
		retry.focus();
		await userEvent.keyboard('{Enter}');
		expect(fetcher.mock.calls[2]?.[0]).toBe(fetcher.mock.calls[1]?.[0]);
		expect(document.activeElement?.contains(screen.getByRole('status'))).toBe(true);

		retrying.resolve(page([line('b')], null));
		await waitFor(() => expect(screen.getAllByRole('listitem')).toHaveLength(2));
		expect(screen.queryByRole('alert')).toBeNull();
		await waitFor(() =>
			expect(document.activeElement?.contains(screen.getAllByRole('listitem')[0]!)).toBe(true)
		);
		expect(document.activeElement).not.toBe(document.body);
	});

	it('discards queued intersection callbacks during failed replacement and on unmount', async () => {
		const observers = stubObserver();
		const replacement = Promise.withResolvers<Response>();
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(page([line('a')], 'old'))
			.mockReturnValueOnce(replacement.promise)
			.mockResolvedValueOnce(page([line('b')], 'new'));
		const feed = new ActivityFeed({ fetcher });
		await feed.load();
		const { unmount } = render(ActivityTimeline, { feed });

		expect(observers).toHaveLength(1);
		feed.sort = 'oldest';
		const loading = feed.load();
		observers[0]!.intersect();
		expect(screen.getAllByRole('listitem')).toHaveLength(1);
		replacement.resolve(new Response(null, { status: 503 }));
		await loading;
		observers[0]!.intersect();
		expect(fetcher).toHaveBeenCalledTimes(2);
		expect(observers[0]!.disconnect).toHaveBeenCalledOnce();

		await userEvent.click(screen.getByRole('button', { name: 'Retry' }));
		await waitFor(() => expect(observers).toHaveLength(2));
		observers[0]!.intersect();
		expect(fetcher).toHaveBeenCalledTimes(3);
		unmount();
		observers[1]!.intersect();
		expect(fetcher).toHaveBeenCalledTimes(3);
		expect(observers[1]!.disconnect).toHaveBeenCalledOnce();
	});

	it('explains an empty log and an empty filter result differently', () => {
		const empty = feedWith([]);
		const { rerender } = render(ActivityTimeline, { feed: empty });

		expect(screen.getByText('No activity yet')).toBeTruthy();

		rerender({ feed: empty, filtered: true });

		expect(screen.getByText('Nothing matches these filters.')).toBeTruthy();
	});

	it('keeps the list on screen while it reloads or fails to refresh', () => {
		const feed = feedWith([line('a')]);

		feed.loading = true;
		feed.failed = true;
		render(ActivityTimeline, { feed });

		const list = screen.getByRole('list');

		expect(within(list).getAllByRole('listitem')).toHaveLength(1);
		expect(screen.getByRole('alert').textContent).toContain('could not be loaded');
	});

	it('lets the photographer retry a failed replacement without using its stale cursor', async () => {
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(Response.json({ events: [line('a')], next: 'c1' }))
			.mockResolvedValueOnce(new Response(null, { status: 503 }))
			.mockResolvedValueOnce(Response.json({ events: [line('b')], next: null }));
		const feed = new ActivityFeed({ fetcher });

		await feed.load();
		feed.sort = 'oldest';
		await feed.load();
		render(ActivityTimeline, { feed });

		expect(screen.queryByRole('button', { name: 'Load more' })).toBeNull();
		await userEvent.setup().click(screen.getByRole('button', { name: 'Retry' }));
		expect(await screen.findAllByRole('listitem')).toHaveLength(1);
		expect(feed.events.map(({ id }) => id)).toEqual(['b']);
	});

	it('has no axe violations', async () => {
		const { container } = render(ActivityTimeline, { feed: feedWith([line('a')], 'c1') });

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('ActivityTimeline in French', () => {
	useLocale('fr');

	it('words a line and the day headings in French', () => {
		render(ActivityTimeline, { feed: feedWith([line('a', { contact: null })]) });

		const paragraph = screen.getByRole('listitem').querySelector('p');
		expect(paragraph?.textContent?.trim().replace(/\s+/g, ' ')).toBe(
			'Client anonyme a mis 3 photos en favori'
		);
		expect(paragraph?.querySelector('strong')?.textContent).toBe('Client anonyme');
		expect(screen.getByRole('heading', { level: 3, name: 'Aujourd’hui' })).toBeTruthy();
	});

	it('localizes a failed cursor and its retry action', async () => {
		const observers = stubObserver();
		const feed = new ActivityFeed({
			fetcher: vi
				.fn()
				.mockResolvedValueOnce(page([line('a')], 'c1'))
				.mockResolvedValueOnce(new Response(null, { status: 503 }))
		});
		await feed.load();
		render(ActivityTimeline, { feed });
		expect(observers).toHaveLength(1);
		observers[0]!.intersect();
		expect((await screen.findByRole('alert')).textContent).toBe(
			'Impossible de charger la suite de l’activité.'
		);
		expect(screen.getByRole('button', { name: 'Réessayer' })).toBeTruthy();
	});
});
