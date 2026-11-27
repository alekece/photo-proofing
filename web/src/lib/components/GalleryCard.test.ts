import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';

import type { GalleryOverview } from '#lib/gallery/api.js';
import GalleryCard from './GalleryCard.svelte';

const gallery = (over: Partial<GalleryOverview> = {}): GalleryOverview => ({
	id: 'g1',
	title: 'Martin Wedding',
	favorite: false,
	layout: 'grid',
	cover_layout: 'full_bleed',
	focal_point: { x: 50, y: 35 },
	heading_font: 'editorial',
	grid_density: 'compact',
	grid_spacing: 'compact',
	expires_at: null,
	included_photos: 30,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	shared_at: null,
	delivered_at: null,
	last_opened_at: null,
	shoot_date: null,
	states: ['shared'],
	created_at: '2026-01-01T10:00:00Z',
	updated_at: '2026-01-02T10:00:00Z',
	photo_count: 32,
	favorite_count: 12,
	final_count: 0,
	downloaded_count: 0,
	...over
});

describe('GalleryCard', () => {
	it('keeps title and summary in the caption while status stays above it', async () => {
		const { container } = render(GalleryCard, { gallery: gallery(), query: '' });
		const link = screen.getByRole('link', { name: /Martin Wedding/ });
		const caption = container.querySelector<HTMLElement>('[data-slot="gallery-card-caption"]')!;
		const status = container.querySelector<HTMLElement>('[data-slot="gallery-card-status"]')!;

		expect(link.getAttribute('href')).toBe('/dashboard/g1');
		expect(link.contains(container.querySelector('[data-slot="gallery-card-image"]'))).toBe(true);
		expect(link.contains(caption)).toBe(true);
		expect(within(caption).getByRole('heading', { name: 'Martin Wedding' })).toBeTruthy();
		expect(within(caption).getByText('32 photos, 12 selected')).toBeTruthy();
		expect(within(caption).queryByText('Shared')).toBeNull();
		expect(caption.contains(status)).toBe(false);
		expect(within(status).getByText('Shared')).toBeTruthy();
		expect(caption.className).toContain('glass-on-image');
		expect(caption.dataset.slot).toBe('gallery-card-caption');
		expect(caption.dataset.surface).toBe('glass');
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

	it('shows Expired in place of the lifecycle status', () => {
		render(GalleryCard, {
			gallery: gallery({
				states: ['delivered', 'expired'],
				expires_at: '2026-01-01',
				access_code: '042819',
				favorite: true
			}),
			query: ''
		});

		const status = screen.getByText('Expired');
		const statusRow = status.parentElement!;
		expect(
			within(statusRow).getByRole('img', { name: 'Protected by an access code' })
		).toBeTruthy();
		expect(within(statusRow).getByRole('img', { name: 'Favorited gallery' })).toBeTruthy();
		expect(screen.queryByText('Delivered')).toBeNull();
	});

	it('uses the hero photo with its focal point, else the striped cover', () => {
		const { container, rerender } = render(GalleryCard, {
			gallery: gallery({ hero_id: 'p1', focal_point: { x: 18, y: 72 } }),
			query: ''
		});

		const img = container.querySelector('img') as HTMLImageElement | null;
		expect(img?.getAttribute('src')).toBe('/api/v1/galleries/g1/photos/p1/preview');
		expect(img?.style.objectPosition).toBe('18% 72%');
		rerender({ gallery: gallery(), query: '' });
		expect(container.querySelector('img')).toBeNull();
	});

	it('highlights the searched text in the title', () => {
		const { container } = render(GalleryCard, { gallery: gallery(), query: 'wed' });

		expect(container.querySelector('mark')?.textContent).toBe('Wed');
		expect(screen.getByRole('heading', { name: 'Martin Wedding' })).toBeTruthy();
	});

	it('shows one accessible unread indicator, without displaying activity labels', () => {
		render(GalleryCard, {
			gallery: gallery({ title: null }),
			query: '',
			unread: true
		});

		expect(screen.getByRole('heading', { name: 'Untitled gallery' }).className).toMatch(
			/opacity-70/
		);
		expect(screen.getAllByRole('img', { name: 'Unread activity' })).toHaveLength(1);
	});

	it('shows inline Lock when access_code is set and Expired replaces the lifecycle status with zero axe violations', async () => {
		const { container, rerender } = render(GalleryCard, {
			gallery: gallery({ access_code: '042819' }),
			query: ''
		});

		expect(screen.getByRole('img', { name: 'Protected by an access code' })).toBeTruthy();
		expect((await axe(container, { rules: { listitem: { enabled: false } } })).violations).toEqual(
			[]
		);

		await rerender({
			gallery: gallery({
				access_code: '042819',
				states: ['shared', 'expired'],
				expires_at: '2026-01-01'
			}),
			query: ''
		});
		expect(screen.getByRole('img', { name: 'Protected by an access code' })).toBeTruthy();
		expect(screen.getByText('Expired')).toBeTruthy();
		expect(screen.queryByText('Shared')).toBeNull();

		await rerender({
			gallery: gallery({ access_code: null, states: ['shared', 'archived'] }),
			query: ''
		});
		expect(screen.queryByRole('img', { name: 'Protected by an access code' })).toBeNull();
		expect(screen.getByText('Archived')).toBeTruthy();
		expect((await axe(container, { rules: { listitem: { enabled: false } } })).violations).toEqual(
			[]
		);
	});

	it('renders an inline Star when favorited and a sibling selection button with zero axe violations', async () => {
		const onselect = vi.fn();
		const user = userEvent.setup();
		const { container, rerender } = render(GalleryCard, {
			gallery: gallery({ favorite: true }),
			query: '',
			selected: false,
			selecting: false,
			onselect
		});

		expect(screen.getByRole('img', { name: 'Favorited gallery' })).toBeTruthy();
		expect(
			(screen.getByRole('img', { name: 'Favorited gallery' }) as HTMLElement).dataset.slot
		).toBe('gallery-favorite');

		const selectBtn = screen.getByRole('button', { name: 'Select Martin Wedding' });
		const link = screen.getByRole('link', { name: /Martin Wedding/ });
		expect(link.contains(selectBtn)).toBe(false);
		expect(selectBtn.getAttribute('aria-pressed')).toBe('false');

		await user.click(selectBtn);
		expect(onselect).toHaveBeenCalledOnce();

		await rerender({
			gallery: gallery({ favorite: true }),
			query: '',
			selected: true,
			selecting: true,
			onselect
		});
		expect(selectBtn.getAttribute('aria-pressed')).toBe('true');
		expect((await axe(container, { rules: { listitem: { enabled: false } } })).violations).toEqual(
			[]
		);
	});

	it('shows favorite state as a permanent yellow indicator, never as an individual action', async () => {
		const { container, rerender } = render(GalleryCard, {
			gallery: gallery({ favorite: false })
		});

		expect(screen.queryByRole('button', { name: /gallery to top/ })).toBeNull();
		expect(screen.queryByRole('img', { name: 'Favorited gallery' })).toBeNull();

		await rerender({ gallery: gallery({ favorite: true }) });
		const favorited = screen.getByRole('img', { name: 'Favorited gallery' });
		expect(favorited.querySelector('svg')?.getAttribute('class')).toContain('text-hero');
		expect(favorited.querySelector('svg')?.getAttribute('fill')).toBe('currentColor');
		expect((await axe(container, { rules: { listitem: { enabled: false } } })).violations).toEqual(
			[]
		);
	});
});
