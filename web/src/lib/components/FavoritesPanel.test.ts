import { render, screen, waitFor } from '@testing-library/svelte';
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
	render(Harness, { props: { events, ondownload } });

	return {
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

	it('keeps Download unavailable until the download seam is given', async () => {
		setup([photo('p1', true)]);

		await screen.findByAltText('p1.jpg');

		expect(screen.queryByRole('button', { name: 'Download p1.jpg' })).toBeNull();
	});

	it('downloads one photo, or the selection from select mode', async () => {
		const ondownload = vi.fn();
		const user = userEvent.setup();

		setup([photo('p1', true), photo('p2', true)], ondownload);
		await screen.findByAltText('p1.jpg');
		await user.click(screen.getByRole('button', { name: 'Download p2.jpg' }));

		expect(ondownload).toHaveBeenLastCalledWith([expect.objectContaining({ id: 'p2' })]);

		await user.click(screen.getByRole('button', { name: 'Select' }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));

		expect(screen.getByText('2 selected')).toBeTruthy();

		await user.click(screen.getByRole('button', { name: /^Download$/ }));

		expect(ondownload).toHaveBeenLastCalledWith([
			expect.objectContaining({ id: 'p1' }),
			expect.objectContaining({ id: 'p2' })
		]);

		await user.click(screen.getByRole('button', { name: 'Cancel selection' }));

		expect(screen.queryByText('2 selected')).toBeNull();
	});

	it('prepares the selection ZIP, then downloads it once ready', async () => {
		const user = userEvent.setup();
		const { finish } = setup([photo('p1', true)]);

		await screen.findByAltText('p1.jpg');
		await user.click(screen.getByRole('button', { name: 'Download selection' }));

		expect(await screen.findByText(/Preparing the ZIP/)).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Download selection' })).toHaveProperty(
			'disabled',
			true
		);
		expect(startDownload).not.toHaveBeenCalled();

		finish(zip('ready'));

		await waitFor(() =>
			expect(startDownload).toHaveBeenCalledExactlyOnceWith(
				'/api/v1/galleries/g1/export/e1/download'
			)
		);
		expect(await screen.findByText('Download started.')).toBeTruthy();
	});

	it('offers a ready ZIP found on load without downloading it', async () => {
		setup([photo('p1', true)], undefined, zip('ready'));

		const link = await screen.findByRole('link', { name: /Download ZIP/ });

		expect(link.getAttribute('href')).toBe('/api/v1/galleries/g1/export/e1/download');
		expect(startDownload).not.toHaveBeenCalled();
	});

	it('shows why a ZIP failed and allows another try', async () => {
		setup([photo('p1', true)], undefined, zip('failed', 'e1', 'The server is out of disk space.'));

		expect((await screen.findByRole('alert')).textContent).toContain('out of disk space');
		expect(screen.getByRole('button', { name: 'Download selection' })).toHaveProperty(
			'disabled',
			false
		);
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
		await user.click(screen.getByRole('button', { name: 'Download selection' }));

		expect((await screen.findByRole('alert')).textContent).toContain('too large');
	});

	it('offers no ZIP without favorites', async () => {
		setup([photo('p1', false)]);

		await screen.findByText('No favorites yet');

		expect(screen.getByRole('button', { name: 'Download selection' })).toHaveProperty(
			'disabled',
			true
		);
	});
});
