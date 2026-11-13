import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Proof } from '#lib/gallery/types.js';
import Page from './+page.svelte';

const gallery = {
	title: 'Wedding',
	layout: 'grid' as const,
	cover_layout: 'full_bleed' as const,
	focal_point: { x: 50, y: 35 },
	heading_font: 'editorial' as const,
	grid_density: 'compact' as const,
	grid_spacing: 'compact' as const,
	shoot_date: null,
	expires_at: null,
	included_photos: null,
	delivered: false,
	hero_id: null
};
const proof: Proof = {
	id: 'p1',
	alt: 'Proof 1',
	thumbnailUrl: 'data:,thumbnail',
	previewUrl: 'data:,preview',
	width: 300,
	height: 200
};

const contextMenu = (target: Element) => {
	const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });

	target.dispatchEvent(event);

	return event.defaultPrevented;
};

describe('client gallery page', () => {
	const fetcher = vi.fn();

	beforeEach(() => {
		fetcher.mockReset();
		fetcher.mockImplementation(async (url: string) =>
			url.endsWith('/comments') ? Response.json([]) : new Response(null, { status: 204 })
		);
		vi.stubGlobal('fetch', fetcher);
	});
	afterEach(() => vi.unstubAllGlobals());

	it('reports one open when a gallery is shown', () => {
		render(Page, { data: { gallery, proofs: [], favorites: [] }, params: { token: 'tok' } });

		const opens = fetcher.mock.calls.filter(([url]) => url.endsWith('/open'));
		expect(opens).toHaveLength(1);
		expect(opens[0][0]).toBe('/api/v1/galleries/shared/tok/open');
	});

	it('reports nothing for an unavailable link', () => {
		render(Page, { data: { gallery: null, proofs: [], favorites: [] }, params: { token: 'tok' } });

		expect(fetcher).not.toHaveBeenCalled();
	});

	it('still shows the gallery when the report fails', () => {
		fetcher.mockRejectedValue(new TypeError('offline'));

		const { getByRole } = render(Page, {
			data: { gallery, proofs: [], favorites: [] },
			params: { token: 'tok' }
		});

		expect(getByRole('heading', { level: 1 }).textContent).toContain('Wedding');
	});

	it('prevents the context menu on the cover overlay and grid photos', () => {
		render(Page, {
			data: {
				gallery: { ...gallery, hero_id: 'p1' },
				proofs: [proof],
				coverUrl: 'data:,clean-cover',
				favorites: []
			},
			params: { token: 'tok' }
		});

		const cover = screen.getByRole('img', { name: 'Gallery cover' });

		expect(contextMenu(cover.nextElementSibling!)).toBe(true);
		expect(contextMenu(screen.getByRole('img', { name: 'Proof 1' }))).toBe(true);
	});

	it('prevents the context menu on the full-screen viewer photo', async () => {
		const user = userEvent.setup();
		render(Page, {
			data: { gallery, proofs: [proof], favorites: [] },
			params: { token: 'tok' }
		});

		await user.click(screen.getByRole('button', { name: 'Open proof 1 of 1' }));

		const image = screen.getByRole('dialog').querySelector('img')!;

		expect(contextMenu(image)).toBe(true);
	});

	it('prevents the context menu when its target is the photo-opening button', () => {
		render(Page, {
			data: { gallery, proofs: [proof], favorites: [] },
			params: { token: 'tok' }
		});

		expect(contextMenu(screen.getByRole('button', { name: 'Open proof 1 of 1' }))).toBe(true);
	});

	it('keeps the normal context menu for non-image content', () => {
		render(Page, {
			data: { gallery, proofs: [proof], favorites: [] },
			params: { token: 'tok' }
		});

		expect(contextMenu(screen.getByRole('heading', { level: 1, name: 'Wedding' }))).toBe(false);
	});

	it('scopes touch-callout protection to the public gallery wrapper', () => {
		render(Page, {
			data: { gallery, proofs: [proof], favorites: [] },
			params: { token: 'tok' }
		});

		expect(
			screen.getByRole('main').parentElement?.classList.contains('client-gallery-protected')
		).toBe(true);
	});
});
