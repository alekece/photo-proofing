import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { UploadQueue } from '#lib/gallery/uploads.svelte.js';
import type { Gallery } from '#lib/gallery/api.js';
import type { LiveEvents } from '#lib/events.js';
import Harness from './GalleryEditingHarness.test.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
afterEach(() => vi.unstubAllGlobals());

describe('gallery editing integration', () => {
	it('keeps loaded panels and the open viewer when an autosave replaces the gallery object', async () => {
		const gallery: Gallery = {
			id: 'g1',
			title: 'Portraits',
			layout: 'grid',
			expires_at: null,
			included_photos: null,
			watermark_enabled: false,
			hero_id: null,
			share_token: 'token',
			last_opened_at: null,
			shoot_date: null,
			states: ['shared'],
			created_at: '2026-01-01T00:00:00Z',
			updated_at: '2026-01-01T00:00:00Z'
		};
		const photo = {
			id: 'p1',
			original_name: 'portrait.jpg',
			size: 3,
			width: 300,
			height: 200,
			status: 'ready',
			favorited_at: '2026-11-05T10:00:00Z',
			created_at: gallery.created_at,
			revision: 1
		};
		const fetcher = vi.fn<typeof fetch>(async (url) =>
			String(url).endsWith('/export')
				? new Response(null, { status: 204 })
				: String(url).endsWith('/comments')
					? Response.json([])
					: Response.json([photo])
		);
		vi.stubGlobal('fetch', fetcher);
		const queue = new UploadQueue({
			upload: async () => ({ kind: 'failed' }),
			limits: async () => ({
				concurrency: 1,
				max_size: 100,
				max_pixels: 1000
			}),
			remove: async () => {}
		});
		const off = () => () => {};
		const events: LiveEvents = {
			onPhoto: off,
			onFavorites: off,
			onExport: off,
			onNotification: () => () => {},
			onResync: off,
			start() {},
			close() {}
		};
		const props = { gallery, queue, events };
		const view = render(Harness, { props });
		await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(8));
		const user = userEvent.setup();
		await user.click(screen.getByRole('tab', { name: 'Design' }));
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await user.click(screen.getByRole('tab', { name: /^Photos/ }));
		await user.click(await screen.findByRole('button', { name: 'Open portrait.jpg' }));
		const viewer = screen.getByRole('dialog');
		await screen.findByRole('textbox');
		const loaded = fetcher.mock.calls.length;
		await view.rerender({
			...props,
			gallery: { ...gallery, title: 'New title', layout: 'masonry' }
		});

		expect(fetcher).toHaveBeenCalledTimes(loaded);
		expect(screen.getByRole('dialog')).toBe(viewer);
	});
});
