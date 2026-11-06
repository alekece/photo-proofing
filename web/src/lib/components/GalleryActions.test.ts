import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
import { goto } from '$app/navigation';

import type { Gallery } from '#lib/gallery/api.js';
import GalleryActions from './GalleryActions.svelte';

const gallery: Gallery = {
	id: 'g 1',
	title: null,
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	states: ['draft'],
	created_at: '2026-11-05T10:00:00Z',
	updated_at: '2026-11-05T10:00:00Z'
};
const archivedGallery: Gallery = { ...gallery, states: ['draft', 'archived'] };

function respond(response: Response) {
	const fetcher = vi.fn().mockResolvedValue(response);
	vi.stubGlobal('fetch', fetcher);

	return fetcher;
}

beforeEach(() => {
	vi.mocked(goto).mockClear();
	vi.unstubAllGlobals();
});

describe('GalleryActions', () => {
	it('archives the gallery and hands back the archived state', async () => {
		const fetcher = respond(new Response(JSON.stringify(archivedGallery)));
		const onchange = vi.fn();
		render(GalleryActions, { gallery, onchange });

		await userEvent.setup().click(screen.getByRole('button', { name: 'Archive' }));

		await waitFor(() => expect(onchange).toHaveBeenCalledWith(archivedGallery));
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/g%201/archive',
			expect.objectContaining({ method: 'POST' })
		);
	});

	it('offers Restore for an archived gallery', async () => {
		const fetcher = respond(new Response(JSON.stringify(gallery)));
		const onchange = vi.fn();
		render(GalleryActions, { gallery: archivedGallery, onchange });

		expect(screen.queryByRole('button', { name: 'Archive' })).toBeNull();

		await userEvent.setup().click(screen.getByRole('button', { name: 'Restore' }));

		await waitFor(() => expect(onchange).toHaveBeenCalledWith(gallery));
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/g%201/restore',
			expect.objectContaining({ method: 'POST' })
		);
	});

	it('deletes at once and returns to the dashboard', async () => {
		const fetcher = respond(new Response(null, { status: 204 }));
		render(GalleryActions, { gallery, onchange: vi.fn() });

		await userEvent.setup().click(screen.getByRole('button', { name: 'Delete' }));

		await waitFor(() => expect(goto).toHaveBeenCalledWith('/dashboard', { invalidateAll: true }));
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/g%201',
			expect.objectContaining({ method: 'DELETE' })
		);
	});

	it('stays on the page and says so when a request fails', async () => {
		respond(new Response('{}', { status: 500 }));
		const onchange = vi.fn();
		render(GalleryActions, { gallery, onchange });

		await userEvent.setup().click(screen.getByRole('button', { name: 'Delete' }));

		expect((await screen.findByRole('alert')).textContent).toMatch(/delete/i);
		expect(goto).not.toHaveBeenCalled();
		expect(onchange).not.toHaveBeenCalled();
	});

	it('sends an expired session to the login page', async () => {
		respond(new Response('{}', { status: 401 }));
		render(GalleryActions, { gallery, onchange: vi.fn() });

		await userEvent.setup().click(screen.getByRole('button', { name: 'Archive' }));

		await waitFor(() =>
			expect(goto).toHaveBeenCalledWith('/login?expired=1', { refreshAll: true })
		);
	});
});
