import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LiveEvents } from '#lib/events.js';
import { useLocale } from '#lib/i18n/testing.js';

vi.mock('$app/navigation', () => ({ afterNavigate: vi.fn(), goto: vi.fn() }));
const state = vi.hoisted(() => ({
	page: { url: new URL('http://localhost/activity') },
	navigating: { to: null }
}));
vi.mock('$app/state', () => state);

import Harness from './ActivityPageHarness.test.svelte';

const data = {
	identity: { account_id: 'a', workspace_id: 'w' },
	idleTimeoutMs: 2592000000
};

const line = (id: string, over = {}) => ({
	id,
	gallery_id: 'g1',
	gallery_title: 'Wedding',
	contact: { id: 'c1', name: 'Alice' },
	kind: 'comment',
	count: 1,
	photo_ids: [],
	detail: null,
	first_at: new Date().toISOString(),
	last_at: new Date().toISOString(),
	...over
});

function open(fetcher: typeof fetch) {
	let notify!: (event: { gallery_id: string }) => void;
	const events = {
		onNotification: (handler: typeof notify) => {
			notify = handler;

			return () => {};
		},
		onResync: () => () => {}
	} as unknown as LiveEvents;

	vi.stubGlobal('fetch', (url: string, init?: RequestInit) =>
		url === '/api/v1/auth/renew'
			? Promise.resolve(new Response(null, { status: 204 }))
			: fetcher(url, init)
	);
	render(Harness, { props: { events, data: data as never } });

	return { notify: (gallery_id: string) => notify({ gallery_id }) };
}

beforeEach(() => {
	state.page.url = new URL('http://localhost/activity');
	vi.stubGlobal('matchMedia', (query: string) => ({
		matches: query === '(prefers-reduced-motion: reduce)',
		addEventListener: vi.fn(),
		removeEventListener: vi.fn()
	}));
});

afterEach(() => vi.unstubAllGlobals());

describe('activity page', () => {
	useLocale('en');

	it('shows the activity feed and searches without resetting the date filter', async () => {
		const fetcher = vi.fn(async () => Response.json({ events: [line('a')], next: null }));
		open(fetcher);

		const row = await screen.findByRole('listitem');
		const sentence = row.querySelector('p')!;

		expect(sentence.querySelector('strong')?.textContent).toBe('Alice');
		expect(within(row).getByRole('link', { name: 'Wedding' }).getAttribute('href')).toBe(
			'/dashboard/g1'
		);
		expect(sentence.textContent?.trim()).toBe('Alice left a comment');
		expect(screen.getByRole('heading', { level: 1, name: 'Activity' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Gallery' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Visitor' })).toBeNull();
		const user = userEvent.setup();
		await user.click(screen.getByRole('button', { name: 'Date' }));
		await user.click(screen.getByRole('menuitemradio', { name: 'Last 7 days' }));
		const search = screen.getByRole('searchbox', { name: 'Search by gallery or visitor' });
		await user.type(search, 'Alice');
		await waitFor(() =>
			expect(fetcher).toHaveBeenLastCalledWith(
				expect.stringMatching(/\/timeline\?(?:range=7d&q=Alice|q=Alice&range=7d)$/),
				expect.anything()
			)
		);
		await user.clear(search);
		expect(fetcher).toHaveBeenLastCalledWith('/api/v1/timeline?range=7d', expect.anything());
	});

	it('keeps the photo panel on its activity when a live update moves it to another day', async () => {
		const photo_ids = Array.from({ length: 12 }, (_, i) => `p${i}`);
		const favorite = line('favorite', {
			kind: 'favorite',
			count: 12,
			photo_ids,
			last_at: new Date(Date.now() - 86400000 * 2).toISOString()
		});
		const updated = { ...favorite, count: 13, last_at: new Date().toISOString() };
		const retained = line('retained', {
			last_at: new Date(Date.now() - 86400000 * 3).toISOString()
		});
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(Response.json({ events: [favorite, retained], next: null }))
			.mockResolvedValueOnce(
				Response.json({
					events: [updated, line('a-new', { kind: 'open', last_at: updated.last_at })],
					next: null
				})
			);
		const { notify } = open(fetcher);

		await userEvent.click(await screen.findByRole('button', { name: '2 more photos' }));
		notify('g1');

		await waitFor(() =>
			expect(
				screen.getAllByRole('listitem').map((row) => row.querySelector('p')?.textContent?.trim())
			).toEqual(['Alice favorited 13 photos', 'Alice opened the gallery', 'Alice left a comment'])
		);
		const panel = screen.getByRole('region', { name: '2 more photos' });
		expect(within(panel).getAllByRole('img')).toHaveLength(2);
		expect(panel.closest('li')?.textContent).toContain('Alice favorited 13 photos');
	});

	it('announces an early search with no matches without moving focus', async () => {
		const initial = Promise.withResolvers<Response>();
		open(
			vi.fn<typeof fetch>(async (url) =>
				String(url).includes('?q=') ? Response.json({ events: [], next: null }) : initial.promise
			)
		);
		const announcement = document.querySelector('[aria-live="polite"][aria-atomic="true"]');
		expect(announcement).not.toBeNull();
		const search = screen.getByRole('searchbox', { name: 'Search by gallery or visitor' });
		await userEvent.type(search, 'no match');

		await waitFor(() =>
			expect(announcement?.textContent?.trim()).toBe('No activity matches this search.')
		);
		initial.resolve(Response.json({ events: [line('a')], next: null }));
		expect(document.activeElement).toBe(search);
	});

	it('changes sort from the existing menu without reusing the failed outgoing cursor', async () => {
		const intersections: (() => void)[] = [];
		vi.stubGlobal(
			'IntersectionObserver',
			class {
				constructor(private callback: IntersectionObserverCallback) {}
				observe(target: Element) {
					intersections.push(() =>
						this.callback(
							[{ target, isIntersecting: true } as IntersectionObserverEntry],
							this as unknown as IntersectionObserver
						)
					);
				}
				disconnect() {}
			}
		);
		const favorite = line('favorite', { kind: 'favorite' });
		const replacement = Promise.withResolvers<Response>();
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(Response.json({ events: [line('a')], next: null }))
			.mockResolvedValueOnce(Response.json({ events: [favorite], next: 'outgoing' }))
			.mockResolvedValueOnce(new Response(null, { status: 503 }))
			.mockReturnValueOnce(replacement.promise)
			.mockResolvedValueOnce(Response.json({ events: [], next: null }));
		open(fetcher);

		await screen.findByRole('listitem');
		expect(fetcher.mock.calls[0]?.[0]).toBe('/api/v1/timeline');
		await userEvent.click(screen.getByRole('button', { name: 'Type' }));
		await userEvent.click(screen.getByRole('menuitemradio', { name: 'Favorites' }));
		await waitFor(() => expect(intersections).toHaveLength(1));
		intersections[0]!();
		await screen.findByRole('button', { name: 'Retry' });

		await userEvent.click(screen.getByRole('button', { name: /sort by.*newest first/i }));
		await userEvent.click(screen.getByRole('menuitemradio', { name: 'Oldest first' }));
		expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull();
		intersections[0]!();
		expect(fetcher).toHaveBeenCalledTimes(4);
		const params = (call: number) =>
			Object.fromEntries(new URL(fetcher.mock.calls[call]![0], 'http://localhost').searchParams);
		expect(params(3)).toEqual({ kind: 'favorite', sort: 'oldest' });

		replacement.resolve(Response.json({ events: [favorite], next: 'incoming' }));
		await waitFor(() => expect(intersections).toHaveLength(2));
		intersections[1]!();
		expect(params(4)).toEqual({ kind: 'favorite', sort: 'oldest', cursor: 'incoming' });
	});
});
