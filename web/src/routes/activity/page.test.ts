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
	idleTimeoutMs: 2592000000,
	galleries: [{ id: 'g1', title: 'Wedding' }],
	contacts: [{ id: 'c1', name: 'Alice' }]
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

function open(fetcher: ReturnType<typeof vi.fn>) {
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

	it('shows the log under the header with Activity active and the filter choices', async () => {
		open(vi.fn(async () => Response.json({ events: [line('a')], next: null })));

		expect((await screen.findByRole('listitem')).querySelector('p')?.textContent?.trim()).toBe(
			'Alice left a comment'
		);
		expect(screen.getByRole('heading', { level: 1, name: 'Activity' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Activity' }).getAttribute('aria-current')).toBe(
			'page'
		);
		await userEvent.click(screen.getByRole('button', { name: 'Filters' }));
		expect(screen.getByRole('option', { name: 'Wedding' })).toBeTruthy();
	});

	it('keeps expansion with its line across a live day change and filtered replacement', async () => {
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
			)
			.mockResolvedValueOnce(
				Response.json({ events: [updated, { ...favorite, id: 'other' }], next: null })
			);
		const { notify } = open(fetcher);

		await userEvent.click(await screen.findByRole('button', { name: 'Show more photos' }));
		notify('g1');

		const expanded = () => screen.getByRole('button', { name: 'Show fewer photos' }).closest('li')!;
		await waitFor(() =>
			expect(
				screen.getAllByRole('listitem').map((row) => row.querySelector('p')?.textContent?.trim())
			).toEqual(['Alice favorited 13 photos', 'Alice opened the gallery', 'Alice left a comment'])
		);
		expect(within(expanded()).getAllByRole('img')).toHaveLength(12);

		await userEvent.click(screen.getByRole('button', { name: 'Filters' }));
		const favorites = screen.getByRole('option', { name: 'Favorites' });
		await userEvent.selectOptions(favorites.closest('select')!, favorites);
		const collapsed = await screen.findByRole('button', { name: 'Show more photos' });
		expect(within(collapsed.closest('li')!).getAllByRole('img')).toHaveLength(10);
		expect(within(expanded()).getAllByRole('img')).toHaveLength(12);
	});
});
