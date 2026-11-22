import { act, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('#lib/gallery/export.js', async (original) => ({
	...(await original<typeof import('#lib/gallery/export.js')>()),
	startDownload: vi.fn()
}));
import { startDownload } from '#lib/gallery/export.js';
import type { FavoritesEvent, LiveEvents } from '#lib/events.js';
import type { Photo } from '#lib/gallery/photos.js';
import Harness from './FavoritesPanelHarness.test.svelte';

it('opens the photographer thread from a favorite photo', async () => {
	const user = userEvent.setup();
	const off = () => () => {};
	const events: LiveEvents = {
		onPhoto: off,
		onFavorites: off,
		onNotification: off,
		onExport: off,
		onResync: off,
		start() {},
		close() {}
	};
	vi.stubGlobal(
		'fetch',
		vi.fn(async (url: string) => {
			if (url.endsWith('/export')) return new Response(null, { status: 204 });
			if (url.endsWith('/photos/p1/comments'))
				return Response.json([
					{
						id: 'c1',
						photo_id: 'p1',
						author: 'client',
						body: 'Favorite note',
						created_at: '2026-11-10T09:00:00Z',
						edited_at: null,
						deleted_at: null
					}
				]);
			if (url.endsWith('/comments')) return Response.json([]);
			return Response.json([photo('p1', true)]);
		})
	);
	render(Harness, { props: { events } });
	await user.click(await screen.findByRole('button', { name: 'Open p1.jpg' }));
	expect(await screen.findByText('Favorite note')).toBeTruthy();
	expect(screen.getByRole('textbox').getAttribute('id')).toBeTruthy();
});

const photo = (id: string, favorite: boolean): Photo => ({
	id,
	original_name: `${id}.jpg`,
	size: 3,
	width: 300,
	height: 200,
	status: 'ready',
	favorited_at: favorite ? '2026-11-05T14:38:00Z' : null,
	created_at: '2026-11-04T10:00:00Z'
});

interface Zip {
	id: string;
	status: { kind: 'preparing' } | { kind: 'ready' } | { kind: 'failed'; code: string };
	photo_count: number;
	created_at: string;
}

const zip = (status: Zip['status']['kind'], id = 'e1', code = 'export_failed'): Zip => ({
	id,
	status: status === 'failed' ? { kind: status, code } : { kind: status },
	photo_count: 1,
	created_at: '2026-11-07T10:00:00Z'
});

function setup(
	initial: Photo[],
	ondownload?: (photos: Photo[]) => void,
	existing: Zip | null = null,
	rejection: Response | null = null
) {
	let server = initial;
	let latest = existing;
	const exportHandlers = new Set<(event: { gallery_id: string }) => void>();
	const handlers = new Set<(event: FavoritesEvent) => void>();
	const events: LiveEvents = {
		onPhoto: () => () => {},
		onFavorites: (handler) => (handlers.add(handler), () => handlers.delete(handler)),
		onExport: (handler) => (exportHandlers.add(handler), () => exportHandlers.delete(handler)),
		onNotification: () => () => {},
		onResync: () => () => {},
		start() {},
		close() {}
	};

	vi.stubGlobal(
		'fetch',
		vi.fn(async (url: string, init?: RequestInit) => {
			if (url.endsWith('/comments')) return Response.json([]);
			if (!url.endsWith('/export')) return new Response(JSON.stringify(server));
			if (init?.method === 'POST') {
				if (rejection) return rejection;

				latest = zip('preparing');

				return new Response(JSON.stringify(latest), { status: 202 });
			}

			return latest ? new Response(JSON.stringify(latest)) : new Response(null, { status: 204 });
		})
	);
	const rendered = render(Harness, { props: { events, ondownload } });

	return {
		rerender: rendered.rerender,
		container: rendered.container,
		switchGallery(id: string) {
			return rendered.rerender({ events, ondownload, galleryId: id });
		},
		finish(next: Zip) {
			latest = next;
			for (const handler of exportHandlers) handler({ gallery_id: 'g1' });
		},
		change(next: Photo[]) {
			server = next;
			for (const handler of handlers) handler({ gallery_id: 'g1' });
		}
	};
}

afterEach(() => vi.unstubAllGlobals());

describe('FavoritesPanel', () => {
	it('downloads the individual original from the viewer', async () => {
		setup([photo('p1', true)]);
		const user = userEvent.setup();
		await user.click(await screen.findByRole('button', { name: 'Open p1.jpg' }));
		await user.click(
			within(screen.getByRole('dialog')).getByRole('button', { name: 'Download p1.jpg' })
		);
		expect(startDownload).toHaveBeenCalledWith('/api/v1/galleries/g1/photos/p1/download');
	});

	it('uses a plain empty state for the read-only collection', async () => {
		setup([]);
		const heading = await screen.findByText('No favorites yet');
		expect(heading.parentElement?.className).not.toContain('repeating-linear-gradient');
	});

	it('reports rejected individual downloads instead of leaving an unhandled promise', async () => {
		setup([photo('p1', true)], () => Promise.reject(new Error('Download failed')));
		await userEvent.click(await screen.findByRole('button', { name: 'Download p1.jpg' }));
		expect((await screen.findByRole('alert')).textContent).toContain('Couldn’t download');
	});

	it('lists only favorites', async () => {
		setup([photo('p1', true), photo('p2', false), photo('p3', true)]);

		expect(await screen.findByAltText('p1.jpg')).toBeTruthy();
		expect(screen.getByAltText('p3.jpg')).toBeTruthy();
		expect(screen.queryByAltText('p2.jpg')).toBeNull();
	});

	it('dates each favorite', async () => {
		setup([{ ...photo('p1', true), favorited_at: '2026-11-09T12:00:00Z' }]);

		expect(await screen.findByText(/^Favorited Nov 9, 2026/)).toBeTruthy();
	});

	it('shows the empty state without favorites', async () => {
		setup([photo('p1', false)]);

		expect(await screen.findByText(/No favorites yet/)).toBeTruthy();
	});

	it('refreshes when the client changes the selection', async () => {
		const { change } = setup([photo('p1', false)]);

		await screen.findByText(/No favorites yet/);
		change([photo('p1', true)]);

		await waitFor(() => expect(screen.getByAltText('p1.jpg')).toBeTruthy());
	});

	it('keeps the viewed favorite and its comment draft when a newer favorite arrives', async () => {
		const first = photo('p1', true);
		const second = photo('p2', true);
		const { change } = setup([first, second]);
		const user = userEvent.setup();
		await user.click(await screen.findByRole('button', { name: 'Open p1.jpg' }));
		const viewer = within(screen.getByRole('dialog'));
		await user.click(viewer.getByRole('button', { name: /next/i }));
		await user.type(viewer.getByRole('textbox'), 'Keep the original crop');

		change([first, second, { ...photo('new', true), favorited_at: '2026-11-06T09:00:00Z' }]);

		await viewer.findByText('3 of 3');
		expect(viewer.queryByRole('img', { name: 'p2.jpg' })).not.toBeNull();
		expect(viewer.getByRole('textbox')).toHaveProperty('value', 'Keep the original crop');
		await user.click(viewer.getByRole('button', { name: /previous/i }));
		expect(viewer.getByRole('img', { name: 'p1.jpg' })).toBeTruthy();
	});

	it('closes the viewer if its favorite is removed and stays closed when it returns', async () => {
		const first = photo('p1', true);
		const second = photo('p2', true);
		const { change } = setup([first, second]);
		await userEvent.click(await screen.findByRole('button', { name: 'Open p1.jpg' }));
		change([second]);
		await waitFor(() => expect(screen.queryByRole('button', { name: 'Open p1.jpg' })).toBeNull());
		expect(screen.queryByRole('dialog')).toBeNull();

		change([first, second]);
		await screen.findByRole('button', { name: 'Open p1.jpg' });
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('says No favorites yet and offers no selection', async () => {
		setup([photo('p1', false)]);

		await screen.findByText('No favorites yet');

		expect(screen.queryByRole('button', { name: /^Select/ })).toBeNull();
	});

	it('downloads an individual original without needing a supplied callback', async () => {
		setup([photo('p1', true)]);

		await screen.findByAltText('p1.jpg');

		await userEvent.setup().click(screen.getByRole('button', { name: 'Download p1.jpg' }));
		expect(startDownload).toHaveBeenCalledWith('/api/v1/galleries/g1/photos/p1/download');
	});

	it('exports exactly the favorites selected in select mode', async () => {
		const { finish } = setup([photo('p1', true), photo('p2', true), photo('p3', true)]);
		const user = userEvent.setup();
		await screen.findByAltText('p1.jpg');
		await user.click(screen.getByRole('button', { name: 'Select p1.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Select p3.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Download 2' }));
		const request = vi.mocked(fetch).mock.calls.find(([, init]) => init?.method === 'POST');
		expect(JSON.parse(String(request?.[1]?.body))).toEqual({ photo_ids: ['p1', 'p3'] });
		finish(zip('ready'));
		await waitFor(() => expect(startDownload).toHaveBeenCalled());
	});

	it('prepares a subset ZIP even when exactly one favorite is selected', async () => {
		const { finish } = setup([photo('p1', true), photo('p2', true)]);
		const user = userEvent.setup();
		await screen.findByAltText('p1.jpg');
		await user.click(screen.getByRole('button', { name: 'Select p1.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Download 1' }));
		for (const button of await screen.findAllByRole('button', { name: /Downloading/ })) {
			expect(button).toHaveProperty('disabled', true);
		}
		expect(startDownload).not.toHaveBeenCalled();
		const request = vi.mocked(fetch).mock.calls.find(([, init]) => init?.method === 'POST');
		expect(JSON.parse(String(request?.[1]?.body))).toEqual({ photo_ids: ['p1'] });
		finish(zip('ready'));
		await waitFor(() =>
			expect(startDownload).toHaveBeenCalledWith('/api/v1/galleries/g1/export/e1/download')
		);
	});

	it('shows a disabled busy control while an individual download is being checked', async () => {
		let finish!: () => void;
		setup([photo('p1', true)], () => new Promise<void>((resolve) => (finish = resolve)));
		const user = userEvent.setup();
		await user.click(await screen.findByRole('button', { name: 'Download p1.jpg' }));
		expect(screen.getByRole('button', { name: 'Download p1.jpg' })).toHaveProperty(
			'disabled',
			true
		);
		expect(screen.getByRole('button', { name: 'Download p1.jpg' }).getAttribute('aria-busy')).toBe(
			'true'
		);
		finish();
		await waitFor(() =>
			expect(screen.getByRole('button', { name: 'Download p1.jpg' })).toHaveProperty(
				'disabled',
				false
			)
		);
	});

	it('uses the individual callback for tiles and ZIP export for select mode', async () => {
		const ondownload = vi.fn();
		const user = userEvent.setup();

		const { finish } = setup([photo('p1', true), photo('p2', true)], ondownload);
		await screen.findByAltText('p1.jpg');
		await user.click(screen.getByRole('button', { name: 'Download p2.jpg' }));

		expect(ondownload).toHaveBeenLastCalledWith([expect.objectContaining({ id: 'p2' })]);

		await user.click(screen.getByRole('button', { name: 'Select p1.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));

		expect(screen.getByText('2 selected')).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Download 2' }));

		expect(ondownload).toHaveBeenCalledTimes(1);
		const request = vi.mocked(fetch).mock.calls.find(([, init]) => init?.method === 'POST');
		expect(JSON.parse(String(request?.[1]?.body))).toEqual({ photo_ids: ['p1', 'p2'] });
		finish(zip('ready'));
		await waitFor(() => expect(startDownload).toHaveBeenCalled());

		await user.keyboard('{Escape}');

		expect(screen.queryByText('2 selected')).toBeNull();
	});

	it('requests all favorites without a body outside select mode, then downloads once ready', async () => {
		const user = userEvent.setup();
		const { finish } = setup([photo('p1', true)]);

		await screen.findByAltText('p1.jpg');
		const download = screen.getByRole('button', { name: 'Download all' });
		expect(within(screen.getByRole('banner')).getByRole('button')).toBe(download);
		await user.click(download);

		const request = vi.mocked(fetch).mock.calls.find(([, init]) => init?.method === 'POST');
		expect(request).toBeDefined();
		expect(request?.[1]?.body).toBeUndefined();
		for (const button of await screen.findAllByRole('button', { name: /Downloading/ })) {
			expect(button).toHaveProperty('disabled', true);
		}
		expect(startDownload).not.toHaveBeenCalled();

		finish(zip('ready'));

		await waitFor(() =>
			expect(startDownload).toHaveBeenCalledExactlyOnceWith(
				'/api/v1/galleries/g1/export/e1/download'
			)
		);
		expect(await screen.findByRole('button', { name: 'Download all' })).toHaveProperty(
			'disabled',
			false
		);
		expect(screen.queryByText('Download started.')).toBeNull();
	});

	it('has no separate Download ZIP button for a ready ZIP', async () => {
		setup([photo('p1', true)], undefined, zip('ready'));

		await screen.findByAltText('p1.jpg');

		expect(screen.queryByRole('link', { name: /Download ZIP/ })).toBeNull();
		expect(screen.getByRole('button', { name: 'Download all' })).toBeTruthy();
		expect(startDownload).not.toHaveBeenCalled();
	});

	it.each([true, false])(
		'offers only the contextual batch download (studio header: %s)',
		async (withHeader) => {
			const { rerender } = setup([photo('p1', true), photo('p2', true)]);
			await rerender({ withHeader });
			const user = userEvent.setup();
			await screen.findByAltText('p1.jpg');
			expect(screen.getByRole('button', { name: 'Download all' })).toBeTruthy();

			expect(document.querySelector('[data-slot="select-actions"]')).toBeNull();

			await user.click(screen.getByRole('button', { name: 'Select p1.jpg' }));

			const bar = document.querySelector<HTMLElement>('[data-slot="select-actions"]')!;

			expect(within(bar).getByRole('button', { name: 'Select all' })).toBeTruthy();
			expect(within(bar).getByRole('button', { name: 'Download 1' })).toHaveProperty(
				'disabled',
				false
			);
			expect(screen.queryByRole('button', { name: 'Download all' })).toBeNull();
			expect(screen.queryByRole('button', { name: 'Select' })).toBeNull();
			expect(bar.dataset.rail).toBe('gallery');

			await user.click(screen.getByAltText('p2.jpg'));
			expect(screen.queryByRole('dialog')).toBeNull();
			expect(within(bar).getByText('2 selected')).toBeTruthy();
			await user.click(within(bar).getByRole('button', { name: 'Unselect all' }));
			expect(within(bar).getByText('0 selected')).toBeTruthy();
			expect(within(bar).getByRole('button', { name: 'Download 0' })).toHaveProperty(
				'disabled',
				true
			);
			await user.keyboard('{Escape}');

			expect(document.querySelector('[data-slot="select-actions"]')).toBeNull();
			expect(screen.getByRole('button', { name: 'Select p1.jpg' })).toBeTruthy();
			expect(screen.getByRole('button', { name: 'Download all' })).toHaveProperty(
				'disabled',
				false
			);
			await waitFor(() =>
				expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Download all' }))
			);
			await user.click(screen.getByRole('button', { name: 'Open p1.jpg' }));
			expect(screen.getByRole('dialog')).toBeTruthy();
		}
	);

	it('keeps the staged count and focus in an inactive Favorites panel', async () => {
		const view = setup([photo('p1', true), photo('p2', true)]);
		const user = userEvent.setup();
		const circle = await screen.findByRole('button', { name: 'Select p1.jpg' });
		await user.click(circle);
		await view.rerender({ active: false });
		const otherTab = view.container.appendChild(document.createElement('button'));
		otherTab.focus();
		await user.keyboard('{Control>}a{/Control}{Meta>}a{/Meta}{Escape}');
		expect(document.activeElement).toBe(otherTab);
		await view.rerender({ active: true });
		expect(screen.getByRole('button', { name: 'Select p1.jpg' }).getAttribute('aria-pressed')).toBe(
			'true'
		);
		expect(screen.getByText('1 selected')).toBeTruthy();
	});

	it('unselects everything from Unselect all, then exits from the bar', async () => {
		setup([photo('p1', true), photo('p2', true)]);
		const user = userEvent.setup();
		await screen.findByAltText('p1.jpg');
		await user.click(screen.getByRole('button', { name: 'Select p1.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));

		expect(screen.getByRole('button', { name: 'Download 2' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Unselect all' }));

		expect(screen.getByText('0 selected')).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Exit selection' }));

		expect(screen.queryByText('0 selected')).toBeNull();
		expect(screen.getByRole('button', { name: 'Select p1.jpg' })).toBeTruthy();
	});

	it('shows why a ZIP failed and allows another try', async () => {
		setup([photo('p1', true)], undefined, zip('failed', 'e1', 'storage_full'));

		expect((await screen.findByRole('alert')).textContent).toContain('storage is full');
		expect(screen.getByRole('button', { name: 'Download all' })).toHaveProperty('disabled', false);
	});

	it('keeps an individual Favorites download error visible when retrying ZIP status', async () => {
		let rejectDownload!: (error: Error) => void;
		setup(
			[photo('p1', true)],
			() => new Promise<void>((_resolve, reject) => (rejectDownload = reject))
		);
		const originalFetch = fetch;
		let status = 'initial';
		const calls = vi.fn<typeof fetch>(async (url, init) => {
			if (String(url) !== '/api/v1/galleries/g1/export') return originalFetch(url, init);
			if (init?.method === 'POST') {
				status = 'unavailable';
				return Response.json(zip('preparing', 'favorites-pending'), { status: 202 });
			}
			if (status === 'ready') return Response.json(zip('ready', 'favorites-pending'));
			return new Response(null, { status: status === 'initial' ? 204 : 503 });
		});
		vi.stubGlobal('fetch', calls);
		const user = userEvent.setup();
		await user.click(await screen.findByRole('button', { name: 'Download p1.jpg' }));
		await user.click(await screen.findByRole('button', { name: 'Download all' }));
		const retry = await screen.findByRole('button', { name: 'Retry ZIP status' });
		await act(() => rejectDownload(new Error('Download failed')));
		expect(screen.getAllByRole('alert').map((alert) => alert.textContent)).toEqual([
			expect.stringContaining('Couldn’t check the ZIP'),
			expect.stringContaining('Couldn’t download the photo')
		]);

		calls.mockClear();
		status = 'ready';
		await user.click(retry);
		await waitFor(() =>
			expect(startDownload).toHaveBeenCalledExactlyOnceWith(
				'/api/v1/galleries/g1/export/favorites-pending/download'
			)
		);
		expect(calls.mock.calls).toEqual([
			['/api/v1/galleries/g1/export', expect.objectContaining({ method: 'GET' })]
		]);
		expect(screen.getByRole('alert').textContent).toContain('Couldn’t download the photo');
		expect(screen.queryByRole('button', { name: 'Retry ZIP status' })).toBeNull();
	});

	it('shows the translated reason when the request is refused', async () => {
		const user = userEvent.setup();

		setup(
			[photo('p1', true)],
			undefined,
			null,
			new Response(JSON.stringify({ code: 'selection_too_large' }), { status: 413 })
		);
		await screen.findByAltText('p1.jpg');
		await user.click(screen.getByRole('button', { name: 'Download all' }));

		expect((await screen.findByRole('alert')).textContent).toContain('too large');
	});

	it('offers no download without favorites', async () => {
		setup([photo('p1', false)]);

		await screen.findByText('No favorites yet');

		expect(screen.queryByRole('button', { name: /^Download/ })).toBeNull();
	});

	it.each([202, 500])('ignores an old gallery ZIP request completing with %i', async (status) => {
		const { switchGallery } = setup([photo('p1', true)]);
		await screen.findByAltText('p1.jpg');
		let finish!: (response: Response) => void;
		vi.stubGlobal('fetch', (url: string, init?: RequestInit) => {
			if (url.endsWith('/comments')) return Promise.resolve(Response.json([]));
			if (init?.method === 'POST') {
				return new Promise<Response>((resolve) => (finish = resolve));
			}
			if (url.endsWith('/export')) return Promise.resolve(new Response(null, { status: 204 }));
			return Promise.resolve(Response.json([photo('p2', true)]));
		});
		await userEvent.click(screen.getByRole('button', { name: 'Download all' }));
		await screen.findByRole('button', { name: /Downloading/ });
		await switchGallery('g2');
		await screen.findByAltText('p2.jpg');
		expect(screen.getByRole('button', { name: 'Download all' })).toHaveProperty('disabled', false);

		vi.stubGlobal('fetch', async () => new Response('{}', { status: 500 }));
		await act(async () => {
			finish(
				status === 202
					? Response.json(zip('preparing'), { status })
					: new Response('{}', { status })
			);
		});
		expect(screen.queryByRole('alert')).toBeNull();
		expect(screen.getByRole('button', { name: 'Download all' })).toHaveProperty('disabled', false);
		expect(startDownload).not.toHaveBeenCalled();
	});
});
