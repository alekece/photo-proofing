import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ afterNavigate: vi.fn(), beforeNavigate: vi.fn(), goto: vi.fn() }));
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
	it('lists galleries as cards with sort options', async () => {
		const { container } = render(Dashboard, {
			props: props([gallery(), gallery({ id: 'g2', title: 'Portraits', layout: 'masonry' })])
		});
		expect(screen.getByRole('heading', { name: 'Wedding' })).toBeTruthy();
		expect(screen.getByRole('link', { name: /Portraits/ }).getAttribute('href')).toBe('/dashboard/g2');
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

	it('shows a friendly empty state and an error state', async () => {
		const { container, unmount } = render(Dashboard, { props: props([]) });
		expect(screen.getByText('Create your first gallery')).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
		unmount();
		render(Dashboard, { props: props(null) });
		expect(screen.getByRole('alert').textContent).toMatch(/Couldn’t load/);
	});

	it('creates a gallery once and opens it', async () => {
		let resolve!: (response: Response) => void;
		const fetcher = vi.fn().mockReturnValue(new Promise<Response>((r) => (resolve = r)));
		vi.stubGlobal('fetch', fetcher);
		const user = userEvent.setup();
		render(Dashboard, { props: props([]) });
		await user.click(screen.getByRole('button', { name: 'New gallery' }));
		const busy = screen.getByRole('button', { name: 'Creating…' }) as HTMLButtonElement;
		expect(busy.disabled).toBe(true);
		resolve(new Response(JSON.stringify(gallery({ id: 'new-id' })), { status: 201 }));
		await waitFor(() => expect(goto).toHaveBeenCalledWith('/dashboard/new-id'));
		expect(fetcher.mock.calls.filter(([url]) => url === '/api/v1/galleries')).toHaveLength(1);
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries',
			expect.objectContaining({ method: 'POST', credentials: 'same-origin' })
		);
	});

	it('reports a failed creation', async () => {
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('{}', { status: 500 })));
		render(Dashboard, { props: props([]) });
		await userEvent.setup().click(screen.getByRole('button', { name: 'New gallery' }));
		expect((await screen.findByRole('alert')).textContent).toMatch(/Couldn’t create/);
		expect(goto).not.toHaveBeenCalled();
	});
});
