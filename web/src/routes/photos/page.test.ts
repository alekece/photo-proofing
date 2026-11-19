import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';
import type { WorkspacePhoto } from '#lib/gallery/schemas.js';

vi.mock('$app/navigation', () => ({
	afterNavigate: vi.fn(),
	beforeNavigate: vi.fn(),
	goto: vi.fn()
}));
const state = vi.hoisted(() => ({
	page: { url: new URL('http://localhost/photos') },
	navigating: { to: null as null | { route: { id: string } } }
}));
vi.mock('$app/state', () => state);
import { goto } from '$app/navigation';
import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
import Photos from './+page.svelte';

const photo = (over: Partial<WorkspacePhoto> = {}): WorkspacePhoto => ({
	id: 'p1',
	gallery_id: 'g1',
	gallery_title: 'Wedding',
	original_name: 'DSC_1234.jpg',
	kind: 'proof',
	revision: 0,
	width: 4,
	height: 3,
	created_at: '2026-11-01T10:00:00Z',
	...over
});

function open(photos: WorkspacePhoto[] | null) {
	return render(Photos, {
		props: {
			data: {
				identity: { account_id: 'a', workspace_id: 'w' },
				idleTimeoutMs: 2592000000,
				photos
			} as never
		}
	});
}

beforeEach(() => {
	vi.mocked(goto).mockClear();
	state.navigating.to = null;
});
afterEach(() => vi.unstubAllGlobals());

describe('/photos page', () => {
	it('renders the empty workspace state with disabled search and no axe violations', async () => {
		const { container } = open([]);

		expect(screen.getByRole('heading', { level: 1, name: 'All Photos' })).toBeTruthy();
		expect(screen.getByText('No photos yet.')).toBeTruthy();
		expect(
			(screen.getByRole('searchbox', { name: 'Search by gallery or photo' }) as HTMLInputElement)
				.disabled
		).toBe(true);
		expect((await axe(container)).violations).toEqual([]);
	});

	it('shows an inline error when the list failed to load', () => {
		open(null);
		expect(screen.getByRole('alert').textContent).toMatch(/Couldn.t load the photos/);
	});

	it('lists photos across galleries with their gallery and kind badge', () => {
		open([
			photo({ id: 'p1', gallery_title: 'Wedding', kind: 'proof' }),
			photo({ id: 'p2', gallery_id: 'g2', gallery_title: 'Portraits', kind: 'final' })
		]);
		const list = screen.getByRole('list', { name: 'All Photos' });

		expect(within(list).getByText('Wedding')).toBeTruthy();
		expect(within(list).getByText('Proof')).toBeTruthy();
		expect(within(list).getByText('Portraits')).toBeTruthy();
		expect(within(list).getByText('Final')).toBeTruthy();
	});

	it('searches by gallery title or photo id', async () => {
		const user = userEvent.setup();
		open([
			photo({ id: 'abc1', gallery_title: 'Wedding' }),
			photo({ id: 'xyz2', gallery_id: 'g2', gallery_title: 'Portraits' })
		]);
		const list = () => screen.getByRole('list', { name: 'All Photos' });

		const search = screen.getByRole('searchbox', { name: 'Search by gallery or photo' });
		await user.type(search, 'portraits');
		expect(within(list()).queryByText('Wedding')).toBeNull();
		expect(within(list()).getByText('Portraits')).toBeTruthy();

		await user.clear(search);
		await user.type(search, 'abc1');
		expect(within(list()).getByText('Wedding')).toBeTruthy();
		expect(within(list()).queryByText('Portraits')).toBeNull();

		await user.clear(search);
		await user.type(search, 'nobody-here');
		expect(screen.getByText('No photos match these filters.')).toBeTruthy();
		await user.click(screen.getByRole('button', { name: 'Clear search and filters' }));
		expect(within(list()).getByText('Wedding')).toBeTruthy();
		expect(within(list()).getByText('Portraits')).toBeTruthy();
	});

	it('filters by gallery and by type', async () => {
		const user = userEvent.setup();
		open([
			photo({ id: 'p1', gallery_id: 'g1', gallery_title: 'Wedding', kind: 'proof' }),
			photo({ id: 'p2', gallery_id: 'g2', gallery_title: 'Portraits', kind: 'final' })
		]);
		const list = () => screen.getByRole('list', { name: 'All Photos' });

		await user.selectOptions(screen.getByRole('combobox', { name: 'Gallery' }), 'g2');
		expect(within(list()).queryByText('Wedding')).toBeNull();
		expect(within(list()).getByText('Portraits')).toBeTruthy();

		await user.selectOptions(screen.getByRole('combobox', { name: 'Gallery' }), 'all');
		await user.selectOptions(screen.getByRole('combobox', { name: 'Type' }), 'final');
		expect(within(list()).queryByText('Wedding')).toBeNull();
		expect(within(list()).getByText('Portraits')).toBeTruthy();
	});

	it('sorts newest or oldest by taken date', async () => {
		const user = userEvent.setup();
		open([
			photo({ id: 'old', gallery_title: 'Older', created_at: '2026-01-01T10:00:00Z' }),
			photo({ id: 'new', gallery_title: 'Newer', created_at: '2026-06-01T10:00:00Z' })
		]);

		const titlesInOrder = () =>
			screen.getAllByRole('button', { name: /^Open/ }).map((btn) => btn.getAttribute('aria-label'));

		expect(titlesInOrder()[0]).toMatch(/Newer/);

		await user.selectOptions(screen.getByRole('combobox', { name: 'Sort' }), 'oldest');
		expect(titlesInOrder()[0]).toMatch(/Older/);
	});

	it.each(['proof', 'final'] as const)('opens a %s in its source gallery viewer', async (kind) => {
		const user = userEvent.setup();
		open([photo({ id: 'p1', gallery_id: 'g1', gallery_title: 'Wedding', kind })]);

		await user.click(screen.getByRole('button', { name: /^Open/ }));
		expect(goto).toHaveBeenCalledWith(`/dashboard/g1?photo=p1&kind=${kind}`);
	});

	it('shows and searches the original filename', async () => {
		open([photo()]);
		expect(screen.getByText('DSC_1234.jpg')).toBeTruthy();
		await userEvent
			.setup()
			.type(screen.getByRole('searchbox', { name: 'Search by gallery or photo' }), 'dsc_1234');
		expect(screen.getByRole('button', { name: /^Open.*DSC_1234/ })).toBeTruthy();
	});

	it('selects photos from their circle, offers select all, and shows only Download and Delete', async () => {
		const user = userEvent.setup();
		open([
			photo({ id: 'p1', gallery_id: 'g1', gallery_title: 'Wedding' }),
			photo({ id: 'p2', gallery_id: 'g2', gallery_title: 'Portraits', kind: 'final' })
		]);

		await user.click(screen.getByRole('button', { name: /^Select Wedding/ }));
		expect(screen.getByText('1 selected')).toBeTruthy();
		expect(goto).not.toHaveBeenCalled();
		const bar = screen
			.getByRole('button', { name: 'Download' })
			.closest('[data-slot="select-actions"]');
		expect(within(bar as HTMLElement).getByRole('button', { name: 'Delete' })).toBeTruthy();
		expect(within(bar as HTMLElement).queryByRole('button', { name: 'Favorite' })).toBeNull();

		await user.click(screen.getByRole('button', { name: 'Select all' }));
		expect(screen.getByText('2 selected')).toBeTruthy();
		await user.click(screen.getByRole('button', { name: 'Unselect all' }));
		expect(screen.getByText('0 selected')).toBeTruthy();

		await user.keyboard('{Escape}');
		expect(screen.queryByText(/selected/)).toBeNull();
	});

	it('downloads selected photos one by one via their authenticated original URL', async () => {
		const user = userEvent.setup();
		const clicks: string[] = [];
		const anchor = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (
			this: HTMLAnchorElement
		) {
			clicks.push(this.href);
		});
		const fetcher = vi.fn<typeof fetch>(async () => new Response(null, { status: 200 }));
		vi.stubGlobal('fetch', fetcher);

		open([
			photo({ id: 'p1', gallery_id: 'g1', gallery_title: 'Wedding', kind: 'proof' }),
			photo({ id: 'p2', gallery_id: 'g2', gallery_title: 'Portraits', kind: 'final' })
		]);

		await user.click(screen.getByRole('button', { name: /^Select Wedding/ }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));
		await user.click(screen.getByRole('button', { name: 'Download' }));

		await waitFor(() => expect(clicks).toHaveLength(2));
		expect(clicks.some((href) => href.includes('/galleries/g1/photos/p1/download'))).toBe(true);
		expect(
			clicks.some((href) => href.includes('/galleries/g2/photos/p2/download?finals=true'))
		).toBe(true);
		anchor.mockRestore();
	});

	it('reports when some downloads failed', async () => {
		const user = userEvent.setup();
		toasts.items = [];
		const fetcher = vi.fn<typeof fetch>(async () => new Response(null, { status: 404 }));
		vi.stubGlobal('fetch', fetcher);
		open([photo({ id: 'p1', gallery_id: 'g1' })]);

		await user.click(screen.getByRole('button', { name: /^Select/ }));
		await user.click(screen.getByRole('button', { name: 'Download' }));

		await waitFor(() =>
			expect(toasts.items.map((toast) => toast.message)).toContain(
				'Some photos couldn’t be downloaded. Please try again.'
			)
		);
	});

	it('deletes a mixed selection grouped by gallery and kind, stopping and keeping the rest selected on a failure', async () => {
		const user = userEvent.setup();
		const fetcher = vi.fn<typeof fetch>(async (url) => {
			const path = String(url);
			if (path.includes('/galleries/g2/')) return new Response('{}', { status: 500 });
			return Response.json({ deleted: 1 });
		});
		vi.stubGlobal('fetch', fetcher);

		open([
			photo({ id: 'p1', gallery_id: 'g1', gallery_title: 'Wedding', kind: 'proof' }),
			photo({ id: 'p2', gallery_id: 'g2', gallery_title: 'Portraits', kind: 'final' })
		]);
		const list = () => screen.getByRole('list', { name: 'All Photos' });

		await user.click(screen.getByRole('button', { name: /^Select Wedding/ }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));
		const dialog = screen.getByRole('dialog', { name: 'Delete 2 photos?' });
		await user.click(within(dialog).getByRole('button', { name: 'Delete' }));

		await screen.findByText('Deleted 1 of 2 photos. Please try again.');
		expect(screen.getByText('1 selected')).toBeTruthy();
		expect(within(list()).queryByText('Wedding')).toBeNull();
		expect(within(list()).getByText('Portraits')).toBeTruthy();
	});

	it('deletes a selection fully on success and clears it', async () => {
		const user = userEvent.setup();
		const fetcher = vi.fn<typeof fetch>(async () => Response.json({ deleted: 1 }));
		vi.stubGlobal('fetch', fetcher);

		open([photo({ id: 'p1', gallery_id: 'g1', gallery_title: 'Wedding' })]);

		await user.click(screen.getByRole('button', { name: /^Select/ }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));
		const dialog = screen.getByRole('dialog', { name: 'Delete 1 photo?' });
		await user.click(within(dialog).getByRole('button', { name: 'Delete' }));

		await waitFor(() => expect(screen.queryByText('Wedding')).toBeNull());
		expect(screen.queryByText(/selected/)).toBeNull();
		expect(screen.getByText('No photos yet.')).toBeTruthy();
	});
});

describe('/photos page in French', () => {
	useLocale('fr');

	it('renders localized French copy', () => {
		open([photo()]);
		expect(screen.getByRole('heading', { level: 1, name: 'Toutes les photos' })).toBeTruthy();
	});
});
