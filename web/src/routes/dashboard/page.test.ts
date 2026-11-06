import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({
	afterNavigate: vi.fn(),
	beforeNavigate: vi.fn(),
	goto: vi.fn()
}));
const state = vi.hoisted(() => ({ navigating: { to: null as null | { route: { id: string } } } }));
vi.mock('$app/state', () => state);
import { goto } from '$app/navigation';
import type { Gallery } from '#lib/gallery/api.js';
import Dashboard from './+page.svelte';

const gallery = (over: Partial<Gallery> = {}): Gallery => ({
	id: 'g1',
	title: 'Wedding',
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	states: ['draft'],
	created_at: '2026-01-01T10:00:00Z',
	updated_at: '2026-01-02T10:00:00Z',
	...over
});
const props = (galleries: Gallery[] | null, sort = 'updated') => ({
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
	it('greets the photographer and lists galleries as cover cards with sort options', async () => {
		const { container } = render(Dashboard, {
			props: props([gallery(), gallery({ id: 'g2', title: 'Portraits', layout: 'masonry' })])
		});

		expect(screen.getByRole('heading', { level: 1 }).textContent).toMatch(
			/^Good (morning|afternoon|evening)/
		);
		expect(screen.getByRole('heading', { name: 'Wedding' })).toBeTruthy();
		expect(screen.getByRole('link', { name: /Portraits/ }).getAttribute('href')).toBe(
			'/dashboard/g2'
		);
		const user = userEvent.setup();

		await user.click(screen.getByRole('button', { name: 'Recently updated' }));
		expect(screen.getAllByRole('radio').map((o) => o.textContent?.trim())).toEqual([
			'Recently updated',
			'Newest created',
			'Title A–Z'
		]);
		await user.click(screen.getByRole('radio', { name: 'Title A–Z' }));
		expect(goto).toHaveBeenCalledWith('?sort=title', expect.anything());
		expect((await axe(container)).violations).toEqual([]);
	});

	it('closes the grid with a card that creates a gallery, and totals in the footer', async () => {
		render(Dashboard, { props: props([gallery(), gallery({ id: 'g2', title: 'Portraits' })]) });
		const user = userEvent.setup();

		expect(screen.getByText('2 galleries')).toBeTruthy();
		await user.click(screen.getByRole('button', { name: /Who’s in front of your lens next/ }));
		expect(screen.getByRole('dialog', { name: 'New gallery' })).toBeTruthy();
	});

	it('shows loading placeholders while the sort change loads', async () => {
		state.navigating.to = { route: { id: '/dashboard' } };
		const { container } = render(Dashboard, { props: props([gallery()]) });
		expect(screen.getByRole('status').textContent).toBe('Loading galleries…');
		expect(screen.queryByRole('heading', { name: 'Wedding' })).toBeNull();
		expect(container.querySelectorAll('.pp-breathe')).toHaveLength(4);
		expect((await axe(container)).violations).toEqual([]);
	});

	it('shows the empty workspace with the toolbar disabled, and an error state', async () => {
		const { container, unmount } = render(Dashboard, { props: props([]) });
		expect(screen.getByText('Nothing in frame yet.')).toBeTruthy();
		expect(screen.queryByText('Golden hour, no clients.')).toBeNull();
		expect(
			(screen.getByRole('searchbox', { name: 'Search galleries' }) as HTMLInputElement).disabled
		).toBe(true);
		expect(
			(screen.getByRole('button', { name: 'Recently updated' }) as HTMLButtonElement).disabled
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
		await userEvent.setup().click(screen.getByRole('button', { name: '+ New gallery' }));
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
		await user.click(screen.getByRole('button', { name: '+ New gallery' }));
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
		await user.click(screen.getByRole('button', { name: '+ New gallery' }));
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
		const choose = async (user: ReturnType<typeof userEvent.setup>, ...names: RegExp[]) => {
			await user.click(screen.getByRole('button', { name: /^Status|Shared|Draft|Archived/ }));
			for (const name of names) await user.click(screen.getByRole('checkbox', { name }));
		};

		it('shows the one chip of each card, with expired winning', () => {
			render(Dashboard, {
				props: props([
					gallery({ id: 'a', title: 'One', states: ['shared'] }),
					gallery({
						id: 'b',
						title: 'Two',
						states: ['shared', 'expired'],
						expires_at: '2026-01-01T00:00:00Z'
					})
				])
			});

			expect(within(screen.getByRole('link', { name: /One/ })).getByText('Shared')).toBeTruthy();
			expect(within(screen.getByRole('link', { name: /Two/ })).getByText('Expired')).toBeTruthy();
		});

		it('hides archived galleries until the filter asks for them', async () => {
			const user = userEvent.setup();
			render(Dashboard, {
				props: props([
					gallery({ id: 'a', title: 'Live' }),
					gallery({ id: 'b', title: 'Old', states: ['delivered', 'archived'] })
				])
			});

			expect(screen.queryByRole('link', { name: /Old/ })).toBeNull();
			expect(screen.getByText('2 galleries')).toBeTruthy();
			await choose(user, /Archived/);
			expect(screen.getByRole('link', { name: /Old/ })).toBeTruthy();
		});

		it('tells which statuses hide matches and clears search and status together', async () => {
			const user = userEvent.setup();
			render(Dashboard, {
				props: props([gallery({ title: 'Old', states: ['delivered', 'archived'] })])
			});

			expect(
				screen.getByRole('heading', { name: 'No galleries with these statuses' })
			).toBeTruthy();
			await user.type(screen.getByRole('searchbox', { name: 'Search galleries' }), 'zzz');
			await user.click(screen.getByRole('button', { name: 'Clear search and status' }));
			expect(
				(screen.getByRole('searchbox', { name: 'Search galleries' }) as HTMLInputElement).value
			).toBe('');
			expect(
				screen.getByRole('heading', { name: 'No galleries with these statuses' })
			).toBeTruthy();
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

			expect(screen.getByText('Nothing in frame yet.')).toBeTruthy();
		});
	});
});
