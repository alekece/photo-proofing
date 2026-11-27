import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';
import { createLiveEvents } from '#lib/events.js';
import { getNotifications, NotificationCenter } from '#lib/gallery/notifications.svelte.js';

vi.mock('#lib/gallery/notifications.svelte.js', async (importOriginal) => ({
	...(await importOriginal<object>()),
	getNotifications: vi.fn()
}));
vi.mock('$app/navigation', () => ({
	afterNavigate: vi.fn(),
	beforeNavigate: vi.fn(),
	goto: vi.fn()
}));
const state = vi.hoisted(() => ({
	page: { url: new URL('http://localhost/dashboard') },
	navigating: {
		from: { route: { id: '/dashboard' } },
		to: null as null | { route: { id: string } }
	}
}));
vi.mock('$app/state', () => state);
import { goto } from '$app/navigation';
import type { GalleryOverview } from '#lib/gallery/api.js';
import Dashboard from './+page.svelte';

const gallery = (over: Partial<GalleryOverview> = {}): GalleryOverview => ({
	id: 'g1',
	title: 'Wedding',
	favorite: false,
	layout: 'grid',
	cover_layout: 'full_bleed',
	focal_point: { x: 50, y: 35 },
	heading_font: 'editorial',
	grid_density: 'compact',
	grid_spacing: 'compact',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	shared_at: null,
	delivered_at: null,
	last_opened_at: null,
	shoot_date: null,
	states: ['draft'],
	created_at: '2026-01-01T10:00:00Z',
	updated_at: '2026-01-02T10:00:00Z',
	photo_count: 0,
	favorite_count: 0,
	final_count: 0,
	downloaded_count: 0,
	...over
});
const props = (galleries: GalleryOverview[] | null, sort = 'updated') => ({
	data: {
		identity: { account_id: 'a', workspace_id: 'w' },
		idleTimeoutMs: 2592000000,
		galleries,
		sort
	} as never
});

beforeEach(() => {
	vi.mocked(goto).mockClear();
	state.navigating.to = null;
});
afterEach(() => vi.unstubAllGlobals());

describe('dashboard', () => {
	it('shows historical unread activity quietly and pulses a dot arriving live', async () => {
		const row = (id: string) => ({
			id,
			gallery_id: id,
			title: null,
			hero_id: null,
			kind: 'open',
			count: 1,
			selected: 0,
			updated_at: '2026-11-09T14:38:00Z',
			read_at: null
		});
		const rows = [row('g1')];
		const center = new NotificationCenter(async () =>
			Response.json({ notifications: rows, unread: rows.length })
		);
		const events = createLiveEvents();
		const onNotification = vi.spyOn(events, 'onNotification');
		vi.mocked(getNotifications).mockReturnValueOnce(center);
		render(Dashboard, { props: props([gallery(), gallery({ id: 'g2', title: 'Portraits' })]) });
		const stop = center.start(events);
		const historical = await screen.findByRole('img', { name: 'Unread activity' });
		expect(historical.className).not.toContain('animate-');

		rows.push(row('g2'));
		onNotification.mock.calls[0][0]({ gallery_id: 'g2' });
		const second = within(screen.getByRole('link', { name: /Portraits/ }).closest('li')!);
		expect((await second.findByRole('img', { name: 'Unread activity' })).className).toContain(
			'motion-safe:animate-'
		);
		expect(historical.className).not.toContain('animate-');
		stop();
	});

	it('restores a pre-upgrade multi-status history entry as All statuses', async () => {
		vi.stubGlobal('scrollTo', vi.fn());
		const { component } = render(Dashboard, { props: props([gallery()]) });
		const legacy = {
			query: 'wed',
			selected: ['draft', 'shared', 'delivered', 'expired'],
			scrollY: 0
		};
		component.snapshot.restore(legacy as unknown as ReturnType<typeof component.snapshot.capture>);
		await waitFor(() => expect(screen.getByRole('button', { name: 'Status' })).toBeTruthy());
		expect(component.snapshot.capture()).toEqual({ query: 'wed', selected: null, scrollY: 0 });
	});

	it('restores gallery search and status from browser history without storing results', async () => {
		vi.stubGlobal('scrollTo', vi.fn());
		const data = props([
			gallery({ title: 'Port wedding', states: ['shared'] }),
			gallery({ id: 'g2', title: 'Port portrait', states: ['draft'] }),
			gallery({ id: 'g3', title: 'Forest wedding', states: ['shared'] })
		]);
		const { component, unmount } = render(Dashboard, { props: data });

		await userEvent.type(screen.getByRole('searchbox'), 'port');
		await userEvent.click(screen.getByRole('button', { name: /status/i }));
		await userEvent.click(screen.getByRole('menuitemradio', { name: /shared/i }));
		const state = component.snapshot.capture();
		unmount();

		const { component: returned } = render(Dashboard, { props: data });
		returned.snapshot.restore(state);

		await waitFor(() => {
			expect(screen.getByRole('searchbox')).toHaveProperty('value', 'port');
			expect(screen.getByRole('heading', { name: 'Port wedding' })).toBeTruthy();
			expect(screen.queryByRole('heading', { name: 'Port portrait' })).toBeNull();
			expect(screen.queryByRole('heading', { name: 'Forest wedding' })).toBeNull();
		});
	});

	it('lists galleries as cover cards with sort options', async () => {
		const { container } = render(Dashboard, {
			props: props([gallery(), gallery({ id: 'g2', title: 'Portraits', layout: 'masonry' })])
		});

		expect(screen.getByRole('heading', { level: 1, name: 'Galleries' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Wedding' })).toBeTruthy();
		expect(screen.getByRole('link', { name: /Portraits/ }).getAttribute('href')).toBe(
			'/dashboard/g2'
		);
		const user = userEvent.setup();

		await user.click(screen.getByRole('button', { name: 'Sort by: Recently updated' }));
		expect(screen.getAllByRole('menuitemradio').map((o) => o.textContent?.trim())).toEqual([
			'Recently updated',
			'Newest created',
			'Oldest created',
			'Title A–Z'
		]);
		await user.click(screen.getByRole('menuitemradio', { name: 'Title A–Z' }));
		expect(goto).toHaveBeenCalledWith('?sort=title', expect.anything());
		expect((await axe(container)).violations).toEqual([]);
	});

	it('closes the grid with a card that creates a gallery', async () => {
		render(Dashboard, { props: props([gallery(), gallery({ id: 'g2', title: 'Portraits' })]) });
		const user = userEvent.setup();

		expect(screen.queryByRole('button', { name: /Who’s in front of your lens next/ })).toBeNull();
		await user.click(screen.getByRole('button', { name: 'New gallery' }));
		expect(screen.getByRole('dialog', { name: 'New gallery' })).toBeTruthy();
	});

	it('filters galleries by search without adding a result count to the toolbar', async () => {
		render(Dashboard, {
			props: props([
				gallery({ id: 'a', photo_count: 32 }),
				gallery({ id: 'b', title: 'Portraits', photo_count: 1 }),
				gallery({ id: 'c', photo_count: 80, states: ['delivered', 'archived'] })
			])
		});

		expect(screen.queryByText('3 galleries')).toBeNull();
		await userEvent.type(screen.getByRole('searchbox', { name: 'Search galleries' }), 'port');
		expect(screen.getByRole('link', { name: /Portraits/ })).toBeTruthy();
		expect(screen.queryByRole('link', { name: /Wedding/ })).toBeNull();
		expect(screen.queryByText('1 gallery')).toBeNull();
		await userEvent.clear(screen.getByRole('searchbox', { name: 'Search galleries' }));
		expect(screen.queryAllByRole('link', { name: /Wedding/ })).toHaveLength(2);
	});

	it('keeps the existing galleries visible while the sort change loads', async () => {
		state.navigating.to = { route: { id: '/dashboard' } };
		const { container } = render(Dashboard, { props: props([gallery()]) });
		expect(screen.getByRole('status').textContent).toBe('Loading galleries…');
		const grid = screen.getByRole('list', { busy: true });
		expect(grid.contains(screen.getByRole('heading', { name: 'Wedding' }))).toBe(true);
		expect((await axe(container)).violations).toEqual([]);
	});

	it('shows the empty workspace with the toolbar disabled, and an error state', async () => {
		const { container, unmount } = render(Dashboard, { props: props([]) });
		expect(screen.getByText('No galleries in focus.')).toBeTruthy();
		expect(screen.queryByText('Golden hour, no clients.')).toBeNull();
		expect(
			(screen.getByRole('searchbox', { name: 'Search galleries' }) as HTMLInputElement).disabled
		).toBe(true);
		expect(
			(screen.getByRole('button', { name: 'Sort by: Recently updated' }) as HTMLButtonElement)
				.disabled
		).toBe(true);
		expect((screen.getByRole('button', { name: 'Status' }) as HTMLButtonElement).disabled).toBe(
			true
		);
		expect((await axe(container)).violations).toEqual([]);
		unmount();
		render(Dashboard, { props: props(null) });
		expect(screen.getByRole('alert').textContent).toMatch(/Couldn’t load/);
	});

	it('opens the new gallery dialog instead of creating immediately', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetcher);
		render(Dashboard, { props: props([]) });
		expect(screen.queryByRole('dialog')).toBeNull();
		await userEvent.setup().click(screen.getByRole('button', { name: 'New gallery' }));
		expect(screen.getByRole('dialog', { name: 'New gallery' })).toBeTruthy();
		expect(fetcher.mock.calls.filter(([url]) => url === '/api/v1/galleries')).toHaveLength(0);
		await userEvent.setup().click(screen.getByRole('button', { name: 'Cancel' }));
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('creates a titled gallery once and opens it', async () => {
		let resolve!: (response: Response) => void;
		const fetcher = vi.fn().mockReturnValue(new Promise<Response>((r) => (resolve = r)));
		vi.stubGlobal('fetch', fetcher);
		const user = userEvent.setup();
		render(Dashboard, { props: props([]) });
		await user.click(screen.getByRole('button', { name: 'New gallery' }));
		await user.type(screen.getByLabelText('Title'), 'Martin wedding');
		await user.click(screen.getByRole('button', { name: 'Create' }));
		const busy = screen.getByRole('button', { name: 'Creating…' }) as HTMLButtonElement;
		expect(busy.disabled).toBe(true);
		fetcher.mockResolvedValue(
			new Response(JSON.stringify(gallery({ id: 'new-id' })), { status: 200 })
		);
		resolve(new Response(JSON.stringify(gallery({ id: 'new-id' })), { status: 201 }));
		await waitFor(() => expect(goto).toHaveBeenCalledWith('/dashboard/new-id'));
		expect(fetcher.mock.calls.filter(([url]) => url === '/api/v1/galleries')).toHaveLength(1);
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries',
			expect.objectContaining({ method: 'POST', credentials: 'same-origin' })
		);
	});

	it('reports a failed creation inside the dialog', async () => {
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('{}', { status: 500 })));
		const user = userEvent.setup();
		render(Dashboard, { props: props([]) });
		await user.click(screen.getByRole('button', { name: 'New gallery' }));
		await user.type(screen.getByLabelText('Title'), 'Martin wedding');
		await user.click(screen.getByRole('button', { name: 'Create' }));
		expect((await screen.findByRole('alert')).textContent).toMatch(/Couldn’t create/);
		expect(goto).not.toHaveBeenCalled();
	});

	describe('search', () => {
		const galleries = [
			gallery({ id: 'a', title: 'Martin Wedding' }),
			gallery({ id: 'b', title: 'axb' }),
			gallery({ id: 'c', title: 'a.b' }),
			gallery({ id: 'd', title: null })
		];
		const search = async (text: string) => {
			render(Dashboard, { props: props(galleries) });
			await userEvent
				.setup()
				.type(screen.getByRole('searchbox', { name: 'Search galleries' }), text);
		};
		const titles = () =>
			screen
				.queryAllByRole('heading', { level: 2 })
				.filter((h) => h.closest('a'))
				.map((h) => h.textContent);

		it('filters cards by title, ignoring case', async () => {
			await search('martin');
			expect(titles()).toEqual(['Martin Wedding']);
		});

		it('treats regex characters literally', async () => {
			await search('a.b');
			expect(titles()).toEqual(['a.b']);
		});

		it('matches untitled galleries by their displayed name', async () => {
			await search('untitled');
			expect(titles()).toEqual(['Untitled gallery']);
		});

		it('shows a no-match state and keeps the search box', async () => {
			await search('zzz');
			expect(screen.getByRole('heading', { name: 'No gallery called “zzz”' })).toBeTruthy();
			expect(screen.getByRole('searchbox', { name: 'Search galleries' })).toBeTruthy();
		});
	});

	describe('states', () => {
		const choose = async (user: ReturnType<typeof userEvent.setup>, name: RegExp) => {
			await user.click(screen.getByRole('button', { name: /^Status|Shared|Draft|Archived/ }));
			await user.click(screen.getByRole('menuitemradio', { name }));
		};

		it('shows expired galleries as Expired on their cards and in the expired filter', async () => {
			render(Dashboard, {
				props: props([
					gallery({ id: 'a', title: 'One', states: ['shared'] }),
					gallery({
						id: 'b',
						title: 'Two',
						states: ['shared', 'expired'],
						expires_at: '2026-01-01'
					})
				])
			});

			expect(
				within(screen.getByRole('link', { name: /One/ }).closest('li')!).getByText('Shared')
			).toBeTruthy();
			expect(
				within(screen.getByRole('link', { name: /Two/ }).closest('li')!).getByText('Expired')
			).toBeTruthy();
			await choose(userEvent.setup(), /Expired/);
			expect(screen.getByRole('link', { name: /Two/ })).toBeTruthy();
			expect(screen.queryByRole('link', { name: /One/ })).toBeNull();
		});

		it('includes archives by default and lets the photographer isolate them', async () => {
			const user = userEvent.setup();
			render(Dashboard, {
				props: props([
					gallery({ id: 'a', title: 'Live' }),
					gallery({ id: 'b', title: 'Old', states: ['delivered', 'archived'] })
				])
			});

			expect(screen.getByRole('link', { name: /Live/ })).toBeTruthy();
			expect(screen.getByRole('link', { name: /Old/ })).toBeTruthy();
			await choose(user, /Archived/);
			expect(screen.queryByRole('link', { name: /Live/ })).toBeNull();
			expect(screen.getByRole('link', { name: /Old/ })).toBeTruthy();
		});

		it('shows empty results without reset buttons and can return to all statuses', async () => {
			const user = userEvent.setup();
			render(Dashboard, {
				props: props([gallery({ title: 'Old', states: ['delivered', 'archived'] })])
			});

			await choose(user, /Shared/);
			expect(
				screen.getByRole('heading', { name: 'No galleries with these statuses' })
			).toBeTruthy();
			expect(screen.queryByRole('button', { name: /clear.*(status|filters)|reset/i })).toBeNull();
			await choose(user, /All statuses/);
			expect(screen.getByRole('heading', { name: 'Old' })).toBeTruthy();
		});

		it('highlights the search match in a card title', async () => {
			const { container } = render(Dashboard, { props: props([gallery({ title: 'Wedding' })]) });

			await userEvent
				.setup()
				.type(screen.getByRole('searchbox', { name: 'Search galleries' }), 'edd');
			expect(container.querySelector('mark')?.textContent).toBe('edd');
		});

		it('keeps the workspace-empty screen only when there are no galleries at all', () => {
			render(Dashboard, { props: props([]) });

			expect(screen.getByText('No galleries in focus.')).toBeTruthy();
		});
	});

	describe('bulk selection, direct edits, delete, and Escape layering', () => {
		function stubPatchAndDeleteFetch(stored: Map<string, GalleryOverview>) {
			const fetcher = vi.fn<typeof fetch>(async (url, init) => {
				const path = String(url);
				if (init?.method === 'PATCH') {
					const body = JSON.parse(String(init.body));
					const id = path.split('/').pop()!;
					const next = { ...stored.get(id)!, ...body };
					stored.set(id, next);
					return Response.json(next);
				}
				if (init?.method === 'DELETE') {
					return new Response(null, { status: 204 });
				}
				return new Response('{}', { status: 200 });
			});
			vi.stubGlobal('fetch', fetcher);
			return fetcher;
		}

		const cardTitles = () =>
			screen
				.queryAllByRole('heading', { level: 2 })
				.filter((h) => h.closest('a'))
				.map((h) => h.textContent);

		it('selects a card from its circle without navigating, and offers select all from the fixed bar', async () => {
			const stored = new Map<string, GalleryOverview>([
				['g1', gallery({ id: 'g1', title: 'Wedding' })],
				['g2', gallery({ id: 'g2', title: 'Portraits' })]
			]);
			render(Dashboard, { props: props([...stored.values()]) });
			const user = userEvent.setup();

			await user.click(screen.getByRole('button', { name: 'Select Portraits' }));
			expect(screen.getByText('1 selected')).toBeTruthy();
			expect(goto).not.toHaveBeenCalled();
			expect(screen.getByRole('link', { name: /Portraits/ }).getAttribute('href')).toBe(
				'/dashboard/g2'
			);
			// Clears the clamped workspace rail, not the gallery studio rail.
			const bar = document.querySelector<HTMLElement>('[data-slot="select-actions"]')!;
			expect(bar.className).toContain('sm:left-[calc(50%+var(--workspace-rail-width)/2)]');
			expect(bar.className).not.toMatch(/md:left-/);

			await user.click(screen.getByRole('button', { name: 'Select all' }));
			expect(screen.getByText('2 selected')).toBeTruthy();
			expect(screen.getByRole('button', { name: 'Unselect all' })).toBeTruthy();

			await user.click(screen.getByRole('button', { name: 'Unselect all' }));
			expect(screen.queryByRole('button', { name: 'Select all' })).toBeNull();

			await user.click(screen.getByRole('button', { name: 'Select Portraits' }));
			await user.click(screen.getByRole('button', { name: 'Select Portraits' }));
			expect(screen.queryByRole('button', { name: 'Select all' })).toBeNull();
		});

		it('keeps favorite changes in bulk selection instead of on each cover', async () => {
			const stored = new Map<string, GalleryOverview>([
				['g1', gallery({ id: 'g1', title: 'Wedding' })]
			]);
			const fetcher = stubPatchAndDeleteFetch(stored);
			const user = userEvent.setup();
			render(Dashboard, { props: props([...stored.values()]) });

			expect(screen.queryByRole('button', { name: /gallery to top/ })).toBeNull();
			await user.click(screen.getByRole('button', { name: 'Select Wedding' }));
			await user.click(screen.getByRole('button', { name: 'Favorite' }));

			await waitFor(() =>
				expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'PATCH')).toHaveLength(1)
			);
			expect(fetcher).toHaveBeenCalledWith(
				'/api/v1/galleries/g1',
				expect.objectContaining({ method: 'PATCH', body: JSON.stringify({ favorite: true }) })
			);
			expect(screen.getByText('1 selected')).toBeTruthy();
			expect(screen.getByRole('img', { name: 'Favorited gallery' })).toBeTruthy();
		});

		it('saves a favorite immediately without reordering or requiring Apply', async () => {
			const stored = new Map<string, GalleryOverview>([
				['g1', gallery({ id: 'g1', title: 'Wedding', updated_at: '2026-01-02T10:00:00Z' })],
				['g2', gallery({ id: 'g2', title: 'Portraits', updated_at: '2026-01-01T10:00:00Z' })]
			]);
			const fetcher = stubPatchAndDeleteFetch(stored);
			const user = userEvent.setup();
			const { container } = render(Dashboard, { props: props([...stored.values()]) });

			expect(cardTitles()).toEqual(['Wedding', 'Portraits']);

			await user.click(screen.getByRole('button', { name: 'Select Portraits' }));
			await user.click(screen.getByRole('button', { name: 'Favorite' }));

			await waitFor(() =>
				expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'PATCH')).toHaveLength(1)
			);
			expect(await screen.findByRole('button', { name: 'Unfavorite' })).toBeTruthy();
			expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull();
			expect((await axe(container)).violations).toEqual([]);

			expect(fetcher).toHaveBeenCalledWith(
				'/api/v1/galleries/g2',
				expect.objectContaining({ method: 'PATCH', body: JSON.stringify({ favorite: true }) })
			);
			// The sort snapshot only changes on refresh or an explicit sort; favoriting never reorders.
			expect(cardTitles()).toEqual(['Wedding', 'Portraits']);
			await user.keyboard('{Escape}');
			expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull();
			expect(screen.getByRole('img', { name: 'Favorited gallery' })).toBeTruthy();
		});

		it('persists a validated edit without a second Apply', async () => {
			const stored = new Map<string, GalleryOverview>([
				['g1', gallery({ id: 'g1', title: 'Wedding' })],
				['g2', gallery({ id: 'g2', title: 'Portraits' })]
			]);
			const fetcher = stubPatchAndDeleteFetch(stored);
			const user = userEvent.setup();
			render(Dashboard, { props: props([...stored.values()]) });

			await user.click(screen.getByRole('button', { name: 'Select Portraits' }));
			await user.click(screen.getByRole('button', { name: 'Quick edit' }));
			const titleField = screen.getByDisplayValue('Portraits');
			await user.clear(titleField);
			await user.type(titleField, 'Editorial Portraits');
			await user.click(screen.getByRole('button', { name: 'Save' }));

			expect(cardTitles()).toEqual(['Wedding', 'Editorial Portraits']);
			expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull();

			await user.keyboard('{Escape}');

			expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull();
			expect(cardTitles()).toEqual(['Wedding', 'Editorial Portraits']);
			expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'PATCH')).toHaveLength(1);
		});

		it('does not reopen quick edit on the next selection after a rename leaves the search', async () => {
			const rows = [gallery(), gallery({ id: 'g2', title: 'Portrait' })];
			stubPatchAndDeleteFetch(new Map(rows.map((g) => [g.id, g])));
			const user = userEvent.setup();
			render(Dashboard, { props: props(rows) });
			const search = screen.getByRole('searchbox');
			await user.type(search, 'Wedding');
			await user.click(screen.getByRole('button', { name: 'Select Wedding' }));
			await user.click(screen.getByRole('button', { name: 'Quick edit' }));
			const title = within(screen.getByRole('dialog')).getByDisplayValue('Wedding');
			await user.clear(title);
			await user.type(title, 'Ceramics');
			await user.click(screen.getByRole('button', { name: 'Save' }));
			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
			await user.clear(search);
			await user.click(screen.getByRole('button', { name: 'Select Portrait' }));
			expect(screen.queryByRole('dialog')).toBeNull();
		});

		it('keeps the failed id selected with its draft intact, and clears only the succeeded one', async () => {
			const stored = new Map<string, GalleryOverview>([
				['g1', gallery({ id: 'g1', title: 'Wedding' })],
				['g2', gallery({ id: 'g2', title: 'Portraits' })]
			]);
			let fail = true;
			const fetcher = vi.fn<typeof fetch>(async (url, init) => {
				const path = String(url);
				if (init?.method === 'PATCH') {
					if (fail && path.endsWith('/g1')) return new Response('{}', { status: 500 });
					const body = JSON.parse(String(init.body));
					const id = path.split('/').pop()!;
					const next = { ...stored.get(id)!, ...body };
					stored.set(id, next);
					return Response.json(next);
				}
				return new Response('{}', { status: 200 });
			});
			vi.stubGlobal('fetch', fetcher);
			const user = userEvent.setup();
			render(Dashboard, { props: props([...stored.values()]) });

			await user.click(screen.getByRole('button', { name: 'Select Wedding' }));
			await user.click(screen.getByRole('button', { name: 'Select all' }));
			await user.click(screen.getByRole('button', { name: 'Quick edit' }));
			const first = screen.getByDisplayValue('Wedding');
			const second = screen.getByDisplayValue('Portraits');
			await user.type(first, ' updated');
			await user.type(second, ' updated');
			await user.click(screen.getByRole('button', { name: 'Save' }));

			await waitFor(() =>
				expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'PATCH')).toHaveLength(2)
			);
			// Wedding (g1) failed and stays selected with its draft; Portraits (g2) succeeded and cleared.
			expect(screen.getByText('1 selected')).toBeTruthy();
			expect(
				screen.getByRole('button', { name: 'Select Wedding' }).getAttribute('aria-pressed')
			).toBe('true');
			expect(
				screen
					.getByRole('button', { name: 'Select Portraits updated' })
					.getAttribute('aria-pressed')
			).toBe('false');
			expect(screen.getByDisplayValue('Wedding updated')).toBeTruthy();
			fail = false;
			await user.click(screen.getByRole('button', { name: 'Save' }));
			expect(cardTitles()).toEqual(['Wedding updated', 'Portraits updated']);
			expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'PATCH')).toHaveLength(3);
		});

		it('stacks every selected cover in the delete confirmation', async () => {
			const galleries = Array.from({ length: 6 }, (_, index) =>
				gallery({ id: `g${index}`, title: `Series ${index}`, hero_id: `cover${index}` })
			);
			render(Dashboard, { props: props(galleries) });
			const user = userEvent.setup();

			await user.click(screen.getByRole('button', { name: 'Select Series 0' }));
			await user.click(screen.getByRole('button', { name: 'Select all' }));
			await user.click(screen.getByRole('button', { name: 'Delete' }));
			const covers = Array.from(screen.getByRole('dialog').querySelectorAll('img'), (img) =>
				img.getAttribute('src')
			);
			expect(covers.sort()).toEqual(
				galleries.map((g) => `/api/v1/galleries/${g.id}/photos/${g.hero_id}/thumbnail`)
			);
		});

		it.each(['Favorite', 'Delete'])('retries %s for the failed gallery only', async (action) => {
			const attempts: string[] = [];
			vi.stubGlobal(
				'fetch',
				vi.fn<typeof fetch>(async (url, init) => {
					if (init?.method !== 'PATCH' && init?.method !== 'DELETE') return Response.json({});
					const id = String(url).split('/').pop()!;
					attempts.push(id);
					if (attempts.length === 2) return new Response('{}', { status: 500 });
					return init.method === 'DELETE'
						? new Response(null, { status: 204 })
						: Response.json(gallery({ id, title: id, favorite: true }));
				})
			);
			const user = userEvent.setup();
			render(Dashboard, { props: props(['a', 'b'].map((id) => gallery({ id, title: id }))) });

			await user.click(screen.getByRole('button', { name: 'Select a' }));
			await user.click(screen.getByRole('button', { name: 'Select all' }));
			await user.click(screen.getByRole('button', { name: action }));
			if (action === 'Delete') {
				await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: action }));
				const alert = await within(screen.getByRole('dialog')).findByRole('alert');
				await user.click(within(alert).getByRole('button', { name: 'Dismiss' }));
			}
			expect(screen.getByText('1 selected')).toBeTruthy();
			expect(screen.getByRole('button', { name: 'Select b' }).getAttribute('aria-pressed')).toBe(
				'true'
			);
			const controls = action === 'Delete' ? within(screen.getByRole('dialog')) : screen;
			await user.click(controls.getByRole('button', { name: action }));
			expect(attempts).toEqual(['a', 'b', 'b']);
			if (action === 'Delete') expect(cardTitles()).toEqual([]);
			else expect(screen.getAllByRole('img', { name: 'Favorited gallery' })).toHaveLength(2);
		});

		it('deletes after confirmation and layers Escape over the dialog, the sort popover, then selection', async () => {
			const stored = new Map<string, GalleryOverview>([
				['g1', gallery({ id: 'g1', title: 'Wedding' })],
				['g2', gallery({ id: 'g2', title: 'Portraits' })]
			]);
			stubPatchAndDeleteFetch(stored);
			const user = userEvent.setup();
			const { container } = render(Dashboard, { props: props([...stored.values()]) });

			await user.click(screen.getByRole('button', { name: 'Select Portraits' }));
			expect(screen.getByText('1 selected')).toBeTruthy();
			expect((await axe(container)).violations).toEqual([]);

			await user.click(screen.getByRole('button', { name: 'Delete' }));
			expect(screen.getByRole('dialog', { name: 'Delete 1 gallery?' })).toBeTruthy();
			await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Delete' }));

			await waitFor(() => expect(cardTitles()).toEqual(['Wedding']));
			expect(screen.queryByText(/selected/)).toBeNull();

			await user.click(screen.getByRole('button', { name: 'Select Wedding' }));
			expect(screen.getByText('1 selected')).toBeTruthy();
			await user.click(screen.getByRole('button', { name: 'Sort by: Recently updated' }));
			expect(screen.getAllByRole('menuitemradio')).toHaveLength(4);
			await user.keyboard('{Escape}');
			expect(screen.queryByRole('menuitemradio')).toBeNull();
			expect(screen.getByText('1 selected')).toBeTruthy();

			await user.keyboard('{Escape}');
			expect(screen.queryByText('1 selected')).toBeNull();
		});
	});
});

describe('dashboard in French', () => {
	useLocale('fr');

	it('labels galleries, search, sort and selection in French', async () => {
		const user = userEvent.setup();
		const { container } = render(Dashboard, {
			props: props([gallery(), gallery({ id: 'g2', title: 'Portraits' })])
		});

		expect(screen.getByRole('heading', { level: 1, name: 'Galeries' })).toBeTruthy();
		const search = screen.getByRole('searchbox', { name: 'Rechercher une galerie' });
		expect(screen.getByRole('button', { name: /Modifiées récemment/ })).toBeTruthy();
		expect(screen.queryByText(/Who’s in front/)).toBeNull();

		await user.type(search, 'port');
		expect(screen.queryByRole('link', { name: /Wedding/ })).toBeNull();

		await user.click(screen.getByRole('button', { name: 'Sélectionner Portraits' }));
		expect(screen.getByText('1 sélectionnée')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Modification rapide' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});
});
