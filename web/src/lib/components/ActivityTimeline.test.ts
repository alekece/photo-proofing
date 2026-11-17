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

afterEach(() => vi.useRealTimers());

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
				line('g', { kind: 'email', count: 1, detail: 'invitation' })
			])
		});

		expect(screen.getByText('Alice favorited 3 photos')).toBeTruthy();
		expect(screen.getByText('Anonymous client un-picked a photo')).toBeTruthy();
		expect(screen.getByText('Alice left 2 comments')).toBeTruthy();
		expect(screen.getByText('Alice downloaded 4 photos')).toBeTruthy();
		expect(screen.getByText('Alice downloaded the ZIP')).toBeTruthy();
		expect(screen.getByText('Alice opened the gallery')).toBeTruthy();
		expect(screen.getByText('Invitation email sent to Alice')).toBeTruthy();
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

	it('has no axe violations', async () => {
		const { container } = render(ActivityTimeline, { feed: feedWith([line('a')], 'c1') });

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('ActivityTimeline in French', () => {
	useLocale('fr');

	it('words a line and the day headings in French', () => {
		render(ActivityTimeline, { feed: feedWith([line('a', { contact: null })]) });

		expect(screen.getByText('Client anonyme a mis 3 photos en favori')).toBeTruthy();
		expect(screen.getByRole('heading', { level: 3, name: 'Aujourd’hui' })).toBeTruthy();
	});
});
