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
const props = (galleries: Gallery[] | null, sort = 'updated', states: string[] = []) => ({
	data: {
		identity: { account_id: 'a', workspace_id: 'w' },
		idleTimeoutMs: 2592000000,
		galleries,
		sort,
		states
	} as never
});

beforeEach(() => {
	vi.mocked(goto).mockClear();
	state.navigating.to = null;
});
afterEach(() => vi.unstubAllGlobals());

describe('dashboard', () => {
	it('lists galleries as cards with sort options', async () => {
		const { container } = render(Dashboard, {
			props: props([gallery(), gallery({ id: 'g2', title: 'Portraits', layout: 'masonry' })])
		});
		expect(screen.getByRole('heading', { name: 'Wedding' })).toBeTruthy();
		expect(screen.getByRole('link', { name: /Portraits/ }).getAttribute('href')).toBe(
			'/dashboard/g2'
		);
		expect(container.textContent).toMatch(/Masonry · Edited/);
		expect(screen.getAllByRole('option').map((o) => o.textContent)).toEqual([
			'Last edited',
			'Date created',
			'Title'
		]);
		await userEvent.setup().selectOptions(screen.getByLabelText('Sort by'), 'title');
		expect(goto).toHaveBeenCalledWith('?sort=title', expect.anything());
		expect((await axe(container)).violations).toEqual([]);
	});

	it('shows the hero preview on the card, else the placeholder', () => {
		const { container } = render(Dashboard, {
			props: props([gallery({ hero_id: 'p1' }), gallery({ id: 'g2', title: 'Plain' })])
		});
		const images = [...container.querySelectorAll('img')];

		expect(images.map((img) => img.getAttribute('src'))).toEqual([
			'/api/v1/galleries/g1/photos/p1/preview'
		]);
		expect(screen.getAllByText('Your hero photo will appear here')).toHaveLength(1);
	});

	it('shows the untitled text for a gallery without a title', () => {
		render(Dashboard, { props: props([gallery({ title: null })]) });
		expect(screen.getByRole('heading', { name: 'Untitled gallery' }).className).toMatch(
			/text-muted-foreground/
		);
	});

	it('shows loading placeholders while the sort change loads', async () => {
		state.navigating.to = { route: { id: '/dashboard' } };
		const { container } = render(Dashboard, { props: props([gallery()]) });
		expect(screen.getByRole('status').textContent).toBe('Loading galleries…');
		expect(screen.queryByRole('heading', { name: 'Wedding' })).toBeNull();
		expect(container.querySelectorAll('.pp-breathe')).toHaveLength(3);
		expect((await axe(container)).violations).toEqual([]);
	});

	it('shows the golden-hour empty state without a search box, and an error state', async () => {
		const { container, unmount } = render(Dashboard, { props: props([]) });
		expect(screen.getByRole('heading', { name: 'Golden hour, no clients.' })).toBeTruthy();
		expect(screen.queryByRole('searchbox', { name: 'Search galleries' })).toBeNull();
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
		await user.click(screen.getByRole('button', { name: 'Create gallery' }));
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
		await user.click(screen.getByRole('button', { name: 'Create gallery' }));
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
		const titles = () => screen.queryAllByRole('heading', { level: 2 }).map((h) => h.textContent);

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
			expect(screen.getByRole('heading', { name: 'No galleries match' })).toBeTruthy();
			expect(screen.getByRole('searchbox', { name: 'Search galleries' })).toBeTruthy();
		});
	});

	describe('states', () => {
		it('shows the progress chip and the flags on each card', () => {
			render(Dashboard, {
				props: props([
					gallery({ id: 'a', title: 'One', states: ['shared'] }),
					gallery({ id: 'b', title: 'Two', states: ['delivered', 'archived', 'expired'] })
				])
			});
			const one = within(screen.getByRole('link', { name: /One/ }));
			const two = within(screen.getByRole('link', { name: /Two/ }));

			expect(one.getByText('Shared')).toBeTruthy();
			expect(one.queryByText('Archived')).toBeNull();
			expect(two.getByText('Delivered')).toBeTruthy();
			expect(two.getByText('Archived')).toBeTruthy();
			expect(two.getByText('Expired')).toBeTruthy();
		});

		it('puts the chosen states in the URL beside the sort', async () => {
			render(Dashboard, { props: props([gallery()], 'title', ['shared']) });
			await userEvent.setup().click(screen.getByRole('checkbox', { name: 'Expired' }));
			expect(goto).toHaveBeenLastCalledWith(
				'?sort=title&state=shared&state=expired',
				expect.anything()
			);
		});

		it('keeps the chosen states when the sort changes', async () => {
			render(Dashboard, { props: props([gallery()], 'updated', ['archived']) });
			await userEvent.setup().selectOptions(screen.getByLabelText('Sort by'), 'created');
			expect(goto).toHaveBeenLastCalledWith('?sort=created&state=archived', expect.anything());
		});

		it('shows the no-results screen, not the empty dashboard, when the filter matches nothing', async () => {
			const { container } = render(Dashboard, { props: props([], 'updated', ['archived']) });

			expect(screen.getByRole('heading', { name: 'No galleries match' })).toBeTruthy();
			expect(screen.queryByRole('heading', { name: 'Golden hour, no clients.' })).toBeNull();
			expect(screen.getByRole('group', { name: 'State' })).toBeTruthy();
			expect((await axe(container)).violations).toEqual([]);
			await userEvent
				.setup()
				.click(screen.getByRole('button', { name: 'Clear search and filter' }));
			expect(goto).toHaveBeenLastCalledWith('?sort=updated', expect.anything());
		});

		it('combines search with the filter and clears both', async () => {
			render(Dashboard, {
				props: props([gallery({ title: 'Wedding' })], 'updated', ['draft'])
			});
			const user = userEvent.setup();

			await user.type(screen.getByRole('searchbox', { name: 'Search galleries' }), 'zzz');
			expect(screen.getByRole('heading', { name: 'No galleries match' })).toBeTruthy();
			await user.click(screen.getByRole('button', { name: 'Clear search and filter' }));
			expect(
				(screen.getByRole('searchbox', { name: 'Search galleries' }) as HTMLInputElement).value
			).toBe('');
			expect(goto).toHaveBeenLastCalledWith('?sort=updated', expect.anything());
		});

		it('keeps the golden-hour empty state for a workspace without galleries', () => {
			render(Dashboard, { props: props([]) });
			expect(screen.getByRole('heading', { name: 'Golden hour, no clients.' })).toBeTruthy();
			expect(screen.queryByRole('group', { name: 'State' })).toBeNull();
		});
	});
});
