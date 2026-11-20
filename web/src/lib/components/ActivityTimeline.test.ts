import { render, screen, within } from '@testing-library/svelte';
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
		const paragraphs = rows.map((row) => row.querySelector('p'));
		expect(paragraphs.map((p) => p?.textContent?.trim())).toEqual([
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
		for (const row of rows) {
			expect(row.querySelector('svg[aria-hidden="true"]')).not.toBeNull();
			expect(
				within(row).queryByRole('img', { name: /favorite|open|comment|download|email/i })
			).toBeNull();
		}
		expect(screen.getAllByRole('link', { name: 'Wedding' })[0]?.getAttribute('href')).toBe(
			'/dashboard/g1'
		);
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
		const toggle = first!.getByRole('button', { name: 'Show more photos' });
		const target = toggle.getAttribute('aria-controls')!;
		const user = userEvent.setup();

		expect(first!.getAllByRole('img')).toHaveLength(limit);
		expect(second!.getAllByRole('img')).toHaveLength(limit);
		expect(exact!.getAllByRole('img')).toHaveLength(limit);
		expect(exact!.queryByRole('button')).toBeNull();
		expect(download!.getAllByRole('img')).toHaveLength(4);
		expect(download!.queryByRole('button')).toBeNull();
		expect(screen.queryByRole('link')).toBeNull();
		expect(toggle.textContent?.trim()).toBe('');
		expect(toggle.getAttribute('aria-expanded')).toBe('false');
		expect(document.getElementById(target)).not.toBeNull();

		toggle.focus();
		await user.keyboard('{Enter}');
		expect(first!.getByRole('button', { name: 'Show fewer photos' })).toBe(toggle);
		expect(first!.getAllByRole('img')).toHaveLength(12);
		expect(second!.getAllByRole('img')).toHaveLength(limit);
		expect(toggle.getAttribute('aria-expanded')).toBe('true');
		expect(toggle.getAttribute('aria-controls')).toBe(target);

		await user.keyboard(' ');
		expect(first!.getAllByRole('img')).toHaveLength(limit);
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

	it('offers Load more only while there is a next page', async () => {
		const feed = feedWith([line('a')], 'c1');
		const more = vi.spyOn(feed, 'loadMore').mockResolvedValue();

		render(ActivityTimeline, { feed });
		await userEvent.setup().click(screen.getByRole('button', { name: 'Load more' }));

		expect(more).toHaveBeenCalledOnce();

		feed.next = null;
		await Promise.resolve();

		expect(screen.queryByRole('button', { name: 'Load more' })).toBeNull();
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
		expect(paragraph?.textContent?.trim()).toBe('Client anonyme a mis 3 photos en favori');
		expect(paragraph?.querySelector('strong')?.textContent).toBe('Client anonyme');
		expect(screen.getByRole('heading', { level: 3, name: 'Aujourd’hui' })).toBeTruthy();
	});
});
