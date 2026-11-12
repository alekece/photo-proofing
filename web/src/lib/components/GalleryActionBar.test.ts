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
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	last_opened_at: null,
	shoot_date: null,
	states: ['draft'],
	created_at: '2026-06-12T10:00:00Z',
	updated_at: '2026-06-12T10:00:00Z'
};

function setup(
	over: Partial<Gallery> = {},
	counts: { photos?: number | null; favorites?: number | null } = {}
) {
	const handlers = {
		onpreview: vi.fn(),
		onshare: vi.fn(),
		onpublish: vi.fn(),
		onrestore: vi.fn()
	};
	const view = render(GalleryActionBar, {
		gallery: { ...base, ...over },
		photoCount: counts.photos ?? null,
		favoriteCount: counts.favorites ?? null,
		...handlers
	});

	return { ...view, ...handlers, user: userEvent.setup() };
}

describe('GalleryActionBar', () => {
	it('offers Share on a draft and lists Upload as done with Share as current step', async () => {
		const { container, onpreview, onshare, user } = setup({}, { photos: 48 });

		expect(screen.getByText('48 photos', { selector: '[data-slot="step-fact"]' })).toBeTruthy();
		expect(screen.getByText('Share', { selector: '[aria-current="step"] strong' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Preview' }));
		await user.click(screen.getByRole('button', { name: 'Share' }));

		expect(onpreview).toHaveBeenCalledOnce();
		expect(onshare).toHaveBeenCalledOnce();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('offers Publish during Prepublish and accents an over-allowance selection', async () => {
		const { container, onpublish, user } = setup(
			{ share_token: 'T'.repeat(22), states: ['shared'], included_photos: 10 },
			{ photos: 48, favorites: 13 }
		);
		const fact = screen.getByText('13 selected', { selector: '[data-slot="step-fact"]' });

		expect(fact.className).toContain('text-danger');

		await user.click(screen.getByRole('button', { name: 'Publish' }));

		expect(onpublish).toHaveBeenCalledOnce();
		expect(screen.queryByRole('button', { name: 'Share' })).toBeNull();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('marks every step done once published and omits Share and Publish buttons', async () => {
		const { container } = setup(
			{ share_token: 'T'.repeat(22), states: ['delivered'] },
			{ photos: 48, favorites: 13 }
		);

		expect(container.querySelector('[aria-current="step"]')).toBeNull();
		expect(screen.getByRole('button', { name: 'Preview' })).toBeTruthy();
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
