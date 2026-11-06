import { render, screen, within } from '@testing-library/svelte';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';

import type { GalleryOverview } from '#lib/gallery/api.js';
import GalleryCard from './GalleryCard.svelte';

const gallery = (over: Partial<GalleryOverview> = {}): GalleryOverview => ({
	id: 'g1',
	title: 'Martin Wedding',
	layout: 'grid',
	expires_at: null,
	included_photos: 30,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	states: ['shared'],
	created_at: '2026-01-01T10:00:00Z',
	updated_at: '2026-01-02T10:00:00Z',
	photo_count: 32,
	favorite_count: 12,
	...over
});

describe('GalleryCard', () => {
	it('links to the gallery with its title, one status chip and its summary line', async () => {
		const { container } = render(GalleryCard, { gallery: gallery(), query: '' });
		const link = screen.getByRole('link', { name: /Martin Wedding/ });

		expect(link.getAttribute('href')).toBe('/dashboard/g1');
		expect(within(link).getByText('Shared')).toBeTruthy();
		expect(within(link).getByText('32 photos · 12 selected')).toBeTruthy();
		// The grid supplies the list; the page test checks the whole.
		expect((await axe(container, { rules: { listitem: { enabled: false } } })).violations).toEqual(
			[]
		);
	});

	it('highlights the selection only when it goes over the included photos', () => {
		const { container, rerender } = render(GalleryCard, {
			gallery: gallery({ included_photos: 10 }),
			query: ''
		});

		expect(container.querySelector('b')?.textContent).toBe('12');
		rerender({ gallery: gallery({ included_photos: 12 }), query: '' });
		expect(container.querySelector('b')).toBeNull();
		rerender({ gallery: gallery({ included_photos: null }), query: '' });
		expect(container.querySelector('b')).toBeNull();
	});

	it('shows only the winning status when the gallery is also expired', () => {
		render(GalleryCard, {
			gallery: gallery({ states: ['shared', 'expired'], expires_at: '2026-01-01T00:00:00Z' }),
			query: ''
		});

		expect(screen.getByText('Expired')).toBeTruthy();
		expect(screen.queryByText('Shared')).toBeNull();
	});

	it('uses the hero photo, else the striped cover', () => {
		const { container, rerender } = render(GalleryCard, {
			gallery: gallery({ hero_id: 'p1' }),
			query: ''
		});

		expect(container.querySelector('img')?.getAttribute('src')).toBe(
			'/api/v1/galleries/g1/photos/p1/preview'
		);
		rerender({ gallery: gallery(), query: '' });
		expect(container.querySelector('img')).toBeNull();
	});

	it('highlights the searched text in the title', () => {
		const { container } = render(GalleryCard, { gallery: gallery(), query: 'wed' });

		expect(container.querySelector('mark')?.textContent).toBe('Wed');
		expect(screen.getByRole('heading', { name: 'Martin Wedding' })).toBeTruthy();
	});

	it('shows the untitled name muted, and a news badge when given', () => {
		render(GalleryCard, { gallery: gallery({ title: null }), query: '', news: 'New selection' });

		expect(screen.getByRole('heading', { name: 'Untitled gallery' }).className).toMatch(
			/opacity-70/
		);
		expect(screen.getByText('New selection')).toBeTruthy();
	});
});
