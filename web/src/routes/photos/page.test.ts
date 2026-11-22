import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';
import type { WorkspacePhoto } from '#lib/gallery/schemas.js';

vi.mock('#lib/events.js', async (original) => {
	const events = await original<typeof import('#lib/events.js')>();
	return { ...events, getLiveEvents: () => events.createLiveEvents() };
});

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

		expect(screen.getByRole('heading', { level: 1, name: 'Photos' })).toBeTruthy();
		expect(screen.getByText('No photos yet.')).toBeTruthy();
		expect(
			(screen.getByRole('searchbox', { name: 'Search by gallery' }) as HTMLInputElement).disabled
		).toBe(true);
		expect((await axe(container)).violations).toEqual([]);
	});

	it('shows an inline error when the list failed to load', () => {
		open(null);
		expect(screen.getByRole('alert').textContent).toMatch(/Couldn.t load the photos/);
	});

	it('lists both types across galleries without a filename or kind badge', () => {
		open([
			photo({ id: 'p1', gallery_title: 'Wedding', kind: 'proof' }),
			photo({ id: 'p2', gallery_id: 'g2', gallery_title: 'Portraits', kind: 'final' })
		]);
		const list = screen.getByRole('list', { name: 'Photos' });

		expect(within(list).getByText('Wedding')).toBeTruthy();
		expect(within(list).getByText('Portraits')).toBeTruthy();
		expect(within(list).queryByText(/^(Proof|Final|DSC_1234.jpg)$/)).toBeNull();
	});

	it('searches by gallery title only', async () => {
		const user = userEvent.setup();
		open([
			photo({ id: 'abc1', gallery_title: 'Wedding' }),
			photo({ id: 'xyz2', gallery_id: 'g2', gallery_title: 'Portraits' })
		]);
		const list = () => screen.getByRole('list', { name: 'Photos' });

		const search = screen.getByRole('searchbox', { name: 'Search by gallery' });
		await user.type(search, 'portraits');
		expect(document.activeElement).toBe(search);
		expect(within(list()).queryByText('Wedding')).toBeNull();
		expect(within(list()).getByText('Portraits')).toBeTruthy();

		for (const match of ['abc1', 'dsc_1234']) {
			await user.clear(search);
			await user.type(search, match);
			expect(screen.getByText('No photos match these filters.')).toBeTruthy();
		}

		await user.clear(search);
		await user.type(search, 'nobody-here');
		expect(screen.getByText('No photos match these filters.')).toBeTruthy();
		await user.click(screen.getByRole('button', { name: 'Clear search and filters' }));
		expect(within(list()).getByText('Wedding')).toBeTruthy();
		expect(within(list()).getByText('Portraits')).toBeTruthy();
		await user.type(search, 'missing');
		await user.click(screen.getByRole('button', { name: 'Reset' }));
		expect(screen.getByRole('searchbox', { name: 'Search by gallery' })).toBe(search);
		expect(search).toHaveProperty('value', '');
	});

	it('combines directly available gallery and type filters', async () => {
		const user = userEvent.setup();
		open([
			photo({ id: 'p1', gallery_id: 'g1', gallery_title: 'Wedding', kind: 'proof' }),
			photo({ id: 'p2', gallery_id: 'g2', gallery_title: 'Portraits', kind: 'final' }),
			photo({ id: 'p3', gallery_id: 'g2', gallery_title: 'Portraits', kind: 'proof' }),
			photo({ id: 'p4', gallery_id: 'g1', gallery_title: 'Wedding', kind: 'final' })
		]);
		const list = () => screen.getByRole('list', { name: 'Photos' });

		await user.click(screen.getByRole('button', { name: /^Gallery:/ }));
		await user.click(screen.getByRole('menuitemradio', { name: 'Portraits' }));
		expect(within(list()).queryByText('Wedding')).toBeNull();
		expect(within(list()).getAllByText('Portraits')).toHaveLength(2);

		await user.click(screen.getByRole('button', { name: /^Type:/ }));
		await user.click(screen.getByRole('menuitemradio', { name: 'Final' }));
		expect(screen.getByText('1 photo')).toBeTruthy();
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

		await user.click(screen.getByRole('button', { name: 'Sort: Newest first' }));
		await user.click(screen.getByRole('menuitemradio', { name: 'Oldest first' }));
		expect(titlesInOrder()[0]).toMatch(/Older/);
	});

	it.each(['proof', 'final'] as const)(
		'opens a %s over the filtered grid without download or delete',
		async (kind) => {
			const user = userEvent.setup();
			open([photo({ kind }), photo({ id: 'p2', gallery_title: 'Portraits' })]);
			const search = screen.getByRole('searchbox', { name: 'Search by gallery' });
			await user.type(search, 'Wedding');
			const thumbnail = screen.getByRole('button', { name: /^Open Wedding/ });
			await user.click(thumbnail);
			const viewer = screen.getByRole('dialog');
			expect(within(viewer).getByRole('img').getAttribute('src')).toBe(
				`/api/v1/galleries/g1/photos/p1/preview${kind === 'final' ? '?finals=true&' : '?'}revision=0`
			);
			expect(within(viewer).queryByRole('button', { name: /Download|Delete/ })).toBeNull();
			await user.click(within(viewer).getByRole('button', { name: 'Close viewer' }));
			expect(goto).not.toHaveBeenCalled();
			expect(screen.getByRole('button', { name: /^Open Wedding/ })).toBe(thumbnail);
			expect(screen.queryByRole('button', { name: /^Open Portraits/ })).toBeNull();
			expect(search).toHaveProperty('value', 'Wedding');
		}
	);

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

	it('downloads a mixed selection as one archive when preparation finishes', async () => {
		const user = userEvent.setup();
		const clicks: string[] = [];
		const anchor = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (
			this: HTMLAnchorElement
		) {
			clicks.push(this.href);
		});
		const posted: unknown[] = [];
		const ready = Promise.withResolvers<Response>();
		const zip = { id: 'zip', photo_count: 2, created_at: '2026-11-01T10:00:00Z' };
		const fetcher = vi.fn<typeof fetch>(async (url, init) => {
			if (String(url) === '/api/v1/photos/export/zip') return ready.promise;
			if (String(url) !== '/api/v1/photos/export') return new Response(null);
			if (init?.method === 'POST') {
				posted.push(JSON.parse(String(init.body)));
				return Response.json({ ...zip, status: { kind: 'preparing' } }, { status: 202 });
			}
			return new Response(null, { status: 204 });
		});
		vi.stubGlobal('fetch', fetcher);

		open([
			photo({ id: 'p1', gallery_id: 'g1', gallery_title: 'Wedding', kind: 'proof' }),
			photo({ id: 'p2', gallery_id: 'g2', gallery_title: 'Portraits', kind: 'final' })
		]);

		await user.click(screen.getByRole('button', { name: /^Select Wedding/ }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));
		await user.click(screen.getByRole('button', { name: 'Download' }));

		expect(posted).toEqual([{ photo_ids: ['p1', 'p2'] }]);
		expect(clicks).toEqual([]);
		ready.resolve(Response.json({ ...zip, status: { kind: 'ready' } }));
		await waitFor(() =>
			expect(clicks.map((href) => new URL(href).pathname)).toEqual([
				'/api/v1/photos/export/zip/download'
			])
		);
		anchor.mockRestore();
	});

	it('downloads one checked final directly', async () => {
		const user = userEvent.setup();
		const anchor = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
		const fetcher = vi.fn<typeof fetch>(async () => new Response(null));
		vi.stubGlobal('fetch', fetcher);
		open([photo({ id: 'p2', kind: 'final' })]);
		await user.click(screen.getByRole('button', { name: /^Select Wedding/ }));
		await user.click(screen.getByRole('button', { name: 'Download' }));
		await waitFor(() => expect(anchor).toHaveBeenCalledOnce());
		expect(
			fetcher.mock.calls
				.map(([url]) => String(url))
				.filter((url) => url.includes('/download') || url.includes('/export'))
		).toEqual(['/api/v1/galleries/g1/photos/p2/download?finals=true']);
		anchor.mockRestore();
	});

	it('reports when the original download fails', async () => {
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
		const list = () => screen.getByRole('list', { name: 'Photos' });

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
		expect(screen.getByRole('heading', { level: 1, name: 'Photos' })).toBeTruthy();
	});
});
