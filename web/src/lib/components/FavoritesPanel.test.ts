import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
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

function setup(
	initial: Photo[],
	included: number | null = 1,
	ondownload?: (photos: Photo[]) => void
) {
	let server = initial;
	const handlers = new Set<(event: FavoritesEvent) => void>();
	const events: LiveEvents = {
		onPhoto: () => () => {},
		onFavorites: (handler) => (handlers.add(handler), () => handlers.delete(handler)),
		onResync: () => () => {},
		start() {},
		close() {}
	};

	vi.stubGlobal(
		'fetch',
		vi.fn(async () => new Response(JSON.stringify(server)))
	);
	render(Harness, { props: { events, included, ondownload } });

	return {
		change(next: Photo[]) {
			server = next;
			for (const handler of handlers) handler({ gallery_id: 'g1' });
		}
	};
}

afterEach(() => vi.unstubAllGlobals());

describe('FavoritesPanel', () => {
	it('lists only favorites and counts the extras', async () => {
		setup([photo('p1', true), photo('p2', false), photo('p3', true)]);

		expect(await screen.findByAltText('p1.jpg')).toBeTruthy();
		expect(screen.getByAltText('p3.jpg')).toBeTruthy();
		expect(screen.queryByAltText('p2.jpg')).toBeNull();
		expect(screen.getByText(/2 selected · 1 included/)).toBeTruthy();
		expect(screen.getByText('1 extra')).toBeTruthy();
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

		expect(screen.getByRole('button', { name: 'Download' })).toHaveProperty('disabled', true);
		expect(screen.queryByRole('button', { name: 'Download p1.jpg' })).toBeNull();
	});

	it('downloads one photo, or the selection from select mode', async () => {
		const ondownload = vi.fn();
		const user = userEvent.setup();

		setup([photo('p1', true), photo('p2', true)], 5, ondownload);
		await screen.findByAltText('p1.jpg');
		await user.click(screen.getByRole('button', { name: 'Download p2.jpg' }));

		expect(ondownload).toHaveBeenLastCalledWith([expect.objectContaining({ id: 'p2' })]);

		await user.click(screen.getByRole('button', { name: 'Select' }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));

		expect(screen.getByText('2 selected')).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Download' }));

		expect(ondownload).toHaveBeenLastCalledWith([
			expect.objectContaining({ id: 'p1' }),
			expect.objectContaining({ id: 'p2' })
		]);

		await user.click(screen.getByRole('button', { name: 'Cancel selection' }));

		expect(screen.queryByText('2 selected')).toBeNull();
	});

	it('shows the extras in bold accent', async () => {
		setup([photo('p1', true), photo('p2', true)]);

		expect((await screen.findByText('1 extra')).className).toContain('text-danger');
	});
});
