import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';

import type { Gallery } from '#lib/gallery/api.js';
import GalleryActionBar from './GalleryActionBar.svelte';

const base: Gallery = {
	id: 'g1',
	title: 'Camille & Lucas',
	layout: 'grid',
	cover_layout: 'full_bleed',
	focal_point: { x: 50, y: 35 },
	heading_font: 'editorial',
	grid_density: 'compact',
	grid_spacing: 'compact',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	shared_at: null,
	delivered_at: null,
	last_opened_at: null,
	shoot_date: null,
	states: ['draft'],
	created_at: '2026-06-12T10:00:00Z',
	updated_at: '2026-06-12T10:00:00Z'
};

function setup(
	over: Partial<Gallery> = {},
	counts: {
		photos?: number | null;
		favorites?: number | null;
		finals?: number | null;
		favoritesCollected?: boolean;
	} = {}
) {
	const handlers = {
		onupload: vi.fn(),
		onshare: vi.fn(),
		oncollect: vi.fn(),
		onretouch: vi.fn(),
		ondelivery: vi.fn(),
		onrestore: vi.fn()
	};
	const view = render(GalleryActionBar, {
		gallery: { ...base, ...over },
		photoCount: counts.photos ?? null,
		favoriteCount: counts.favorites ?? null,
		finalCount: counts.finals ?? null,
		favoritesCollected: counts.favoritesCollected ?? false,
		...handlers
	});

	return { ...view, ...handlers, user: userEvent.setup() };
}

describe('GalleryActionBar', () => {
	it('uses Journey steps as shortcuts, not lifecycle actions', async () => {
		const { container, onupload, onshare, oncollect, onretouch, ondelivery, rerender, user } =
			setup({}, { photos: 48 });

		expect(
			screen.getByText('Share the gallery', { selector: '[aria-current="step"] strong' })
		).toBeTruthy();
		expect(screen.getByText('Deliver the gallery')).toBeTruthy();
		expect(screen.getByText('Import retouched photos')).toBeTruthy();
		expect(screen.getByText('Download favorites')).toBeTruthy();
		expect(
			screen
				.getByRole('button', { name: 'Go to Share the gallery' })
				.querySelector('[aria-hidden="true"]')?.className
		).toContain('bg-terracotta-soft');

		expect(screen.queryByRole('button', { name: 'Preview' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Go to Download favorites' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Go to Deliver the gallery' })).toBeNull();
		await user.click(screen.getByRole('button', { name: 'Go to Upload photos' }));
		await user.click(screen.getByRole('button', { name: 'Go to Share the gallery' }));

		expect(onupload).toHaveBeenCalledOnce();
		expect(onshare).toHaveBeenCalledOnce();
		await rerender({ gallery: { ...base, share_token: 'T'.repeat(22), states: ['shared'] } });
		await user.click(screen.getByRole('button', { name: 'Go to Share the gallery' }));
		await user.click(screen.getByRole('button', { name: 'Go to Download favorites' }));
		await user.click(screen.getByRole('button', { name: 'Go to Import retouched photos' }));
		await user.click(screen.getByRole('button', { name: 'Go to Deliver the gallery' }));
		expect(oncollect).toHaveBeenCalledOnce();
		expect(onretouch).toHaveBeenCalledOnce();
		expect(ondelivery).toHaveBeenCalledOnce();
		expect(onshare).toHaveBeenCalledTimes(2);
		expect(screen.queryByRole('button', { name: 'Publish' })).toBeNull();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('moves through Collect, Import retouched photos and Deliver as work becomes observable', async () => {
		const shared: Gallery = { ...base, share_token: 'T'.repeat(22), states: ['shared'] };
		const { container, rerender } = setup(shared, { photos: 48, favorites: 13 });

		expect(
			screen.getByText('Download favorites', { selector: '[aria-current="step"] strong' })
		).toBeTruthy();

		await rerender({
			gallery: shared,
			photoCount: 48,
			favoriteCount: 13,
			finalCount: 0,
			favoritesCollected: true
		});
		expect(
			screen.getByText('Import retouched photos', { selector: '[aria-current="step"] strong' })
		).toBeTruthy();

		await rerender({
			gallery: shared,
			photoCount: 48,
			favoriteCount: 13,
			finalCount: 1,
			favoritesCollected: false
		});
		expect(
			screen.getByText('Deliver the gallery', { selector: '[aria-current="step"] strong' })
		).toBeTruthy();

		await rerender({
			gallery: shared,
			photoCount: 48,
			favoriteCount: 13,
			finalCount: 1,
			favoritesCollected: true
		});
		expect(
			screen.getByText('Deliver the gallery', { selector: '[aria-current="step"] strong' })
		).toBeTruthy();

		await rerender({
			gallery: { ...shared, states: ['delivered'] },
			photoCount: 48,
			favoriteCount: 13,
			finalCount: 1,
			favoritesCollected: true
		});
		expect(container.querySelector('[aria-current="step"]')).toBeNull();
	});

	it('keeps later steps pending while the gallery is not shared', () => {
		setup(base, {
			photos: 7,
			favorites: 4,
			finals: 3,
			favoritesCollected: true
		});

		expect(
			screen.getByText('Share the gallery', { selector: '[aria-current="step"] strong' })
		).toBeTruthy();
		expect(screen.getAllByText('done')).toHaveLength(1);
	});

	it('dates the Share and Deliver steps once they happened', () => {
		setup(
			{
				share_token: 'T'.repeat(22),
				states: ['delivered'],
				shared_at: '2026-06-14T09:00:00Z',
				delivered_at: '2026-06-20T09:00:00Z'
			},
			{ photos: 48, finals: 3 }
		);

		expect(
			screen.getByText('Shared on 14 Jun 2026', { selector: '[data-slot="step-fact"]' })
		).toBeTruthy();
		expect(
			screen.getByText('Delivered on 20 Jun 2026', { selector: '[data-slot="step-fact"]' })
		).toBeTruthy();
	});

	it('omits Publish and invitation actions from the stage header', async () => {
		const { container } = setup(
			{ share_token: 'T'.repeat(22), states: ['shared'], included_photos: 10 },
			{ photos: 48, favorites: 13 }
		);
		const fact = screen.getByText('13 selected', { selector: '[data-slot="step-fact"]' });

		expect(fact.className).toContain('text-danger');

		expect(screen.queryByRole('button', { name: 'Publish' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Share' })).toBeNull();
		expect(screen.getByRole('navigation', { name: 'Gallery progress' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Cancel' })).toBeNull();
		expect(screen.queryByRole('button', { name: /Send invitation/ })).toBeNull();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('marks every step done once published and omits Share and Publish buttons', async () => {
		const { container } = setup(
			{ share_token: 'T'.repeat(22), states: ['delivered'] },
			{ photos: 48, favorites: 13 }
		);

		expect(container.querySelector('[aria-current="step"]')).toBeNull();
		expect(screen.queryByRole('button', { name: 'Preview' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Share' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Publish' })).toBeNull();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('hides the journey wizard and offers Restore when archived', async () => {
		const { container, onrestore, user } = setup({
			share_token: 'T'.repeat(22),
			states: ['shared', 'archived']
		});

		expect(screen.queryByRole('navigation', { name: 'Gallery progress' })).toBeNull();

		await user.click(screen.getByRole('button', { name: 'Restore' }));

		expect(onrestore).toHaveBeenCalledOnce();
		expect((await axe(container)).violations).toEqual([]);
	});
});
