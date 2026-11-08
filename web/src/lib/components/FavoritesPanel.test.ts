import { act, render, screen, waitFor } from '@testing-library/svelte';
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

const photo = (id: string, favorite: boolean): Photo => ({
	id,
	original_name: `${id}.jpg`,
	size: 3,
	width: 300,
	height: 200,
	status: 'ready',
	favorite,
	created_at: '2026-11-04T10:00:00Z'
});

interface Zip {
	id: string;
	status: { kind: 'preparing' } | { kind: 'ready' } | { kind: 'failed'; reason: string };
	photo_count: number;
	created_at: string;
}

const zip = (status: Zip['status']['kind'], id = 'e1', reason = 'The ZIP failed.'): Zip => ({
	id,
	status: status === 'failed' ? { kind: status, reason } : { kind: status },
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
		onResync: () => () => {},
		start() {},
		close() {}
	};

	vi.stubGlobal(
		'fetch',
		vi.fn(async (url: string, init?: RequestInit) => {
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
	it('offers the same individual download in the viewer', async () => {
		setup([photo('p1', true)]);
		const user = userEvent.setup();
		await user.click(await screen.findByRole('button', { name: 'Open p1.jpg' }));
		expect(
			screen.getByRole('dialog').querySelector('button[aria-label="Download p1.jpg"]')
		).not.toBeNull();
	});

	it('retains the striped empty state shared with the other tabs', async () => {
		setup([]);
		const heading = await screen.findByText('No favorites yet');
		expect(heading.parentElement?.className).not.toContain('border-solid');
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

	it('says No favorites yet and offers no selection', async () => {
		setup([photo('p1', false)]);

		await screen.findByText('No favorites yet');

		expect(screen.getByRole('button', { name: 'Select' })).toHaveProperty('disabled', true);
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
		await user.click(screen.getByRole('button', { name: 'Select' }));
		await user.click(screen.getByRole('checkbox', { name: 'Select p1.jpg' }));
		await user.click(screen.getByRole('checkbox', { name: 'Select p3.jpg' }));
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
		await user.click(screen.getByRole('button', { name: 'Select' }));
		await user.click(screen.getByRole('checkbox', { name: 'Select p1.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Download 1' }));
		expect(await screen.findByRole('button', { name: /Downloading/ })).toHaveProperty(
			'disabled',
			true
		);
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

		await user.click(screen.getByRole('button', { name: 'Select' }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));

		expect(screen.getByText('2 selected')).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Download 2' }));

		expect(ondownload).toHaveBeenCalledTimes(1);
		const request = vi.mocked(fetch).mock.calls.find(([, init]) => init?.method === 'POST');
		expect(JSON.parse(String(request?.[1]?.body))).toEqual({ photo_ids: ['p1', 'p2'] });
		finish(zip('ready'));
		await waitFor(() => expect(startDownload).toHaveBeenCalled());

		await user.click(screen.getByRole('button', { name: 'Select', pressed: true }));

		expect(screen.queryByText('2 selected')).toBeNull();
	});

	it('prepares the selection ZIP, then downloads it once ready', async () => {
		const user = userEvent.setup();
		const { finish } = setup([photo('p1', true)]);

		await screen.findByAltText('p1.jpg');
		await user.click(screen.getByRole('button', { name: 'Download all' }));

		expect(await screen.findByRole('button', { name: /Downloading/ })).toBeTruthy();
		expect(screen.getByRole('button', { name: /Downloading/ })).toHaveProperty('disabled', true);
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

	it('keeps the toolbar buttons and shows select mode as a pressed Select', async () => {
		setup([photo('p1', true), photo('p2', true)]);
		const user = userEvent.setup();
		await screen.findByAltText('p1.jpg');
		const select = screen.getByRole('button', { name: 'Select' });
		const download = screen.getByRole('button', { name: 'Download all' });

		expect(select.getAttribute('aria-pressed')).toBe('false');
		expect(screen.queryByRole('button', { name: 'Unselect all' })).toBeNull();

		await user.click(select);

		expect(screen.getByRole('button', { name: 'Select' })).toBe(select);
		expect(screen.getByRole('button', { name: 'Download all' })).toBe(download);
		expect(select.getAttribute('aria-pressed')).toBe('true');
		expect(screen.getByRole('button', { name: 'Unselect all' })).toHaveProperty('disabled', true);

		await user.click(select);

		expect(select.getAttribute('aria-pressed')).toBe('false');
		expect(screen.queryByText('0 selected')).toBeNull();
	});

	it('unselects everything and stays in select mode', async () => {
		setup([photo('p1', true), photo('p2', true)]);
		const user = userEvent.setup();
		await screen.findByAltText('p1.jpg');
		await user.click(screen.getByRole('button', { name: 'Select' }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));

		expect(screen.getByRole('button', { name: 'Download 2' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Unselect all' }));

		expect(screen.getByText('0 selected')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Unselect all' })).toHaveProperty('disabled', true);
		expect(screen.getByRole('button', { name: 'Select' }).getAttribute('aria-pressed')).toBe(
			'true'
		);
		expect(screen.getByRole('button', { name: 'Download all' })).toBeTruthy();
	});

	it('shows why a ZIP failed and allows another try', async () => {
		setup([photo('p1', true)], undefined, zip('failed', 'e1', 'The server is out of disk space.'));

		expect((await screen.findByRole('alert')).textContent).toContain('out of disk space');
		expect(screen.getByRole('button', { name: 'Download all' })).toHaveProperty('disabled', false);
	});

	it('shows the server’s reason when the request is refused', async () => {
		const user = userEvent.setup();

		setup(
			[photo('p1', true)],
			undefined,
			null,
			new Response(JSON.stringify({ error: 'the selection is too large' }), { status: 413 })
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
