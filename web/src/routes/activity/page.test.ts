import { render, screen, waitFor } from '@testing-library/svelte';
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

	vi.stubGlobal('fetch', fetcher);
	render(Harness, { props: { events, data: data as never } });

	return { notify: (gallery_id: string) => notify({ gallery_id }) };
}

beforeEach(() => {
	state.page.url = new URL('http://localhost/activity');
});

afterEach(() => vi.unstubAllGlobals());

describe('activity page', () => {
	useLocale('en');

	it('shows the log under the header with Activity active and the filter choices', async () => {
		open(vi.fn(async () => Response.json({ events: [line('a')], next: null })));

		expect(await screen.findByText('Alice left a comment')).toBeTruthy();
		expect(screen.getByRole('heading', { level: 1, name: 'Activity' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Activity' }).getAttribute('aria-current')).toBe(
			'page'
		);
		expect(screen.getByRole('option', { name: 'Wedding' })).toBeTruthy();
	});

	it('adds a new line live without removing the ones on screen', async () => {
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(Response.json({ events: [line('a')], next: null }))
			.mockResolvedValueOnce(
				Response.json({ events: [line('b', { kind: 'open' }), line('a')], next: null })
			);
		const { notify } = open(fetcher);

		await screen.findByText('Alice left a comment');
		notify('g1');

		expect(await screen.findByText('Alice opened the gallery')).toBeTruthy();
		expect(screen.getByText('Alice left a comment')).toBeTruthy();
		await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2));
	});
});
