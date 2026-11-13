import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Gallery } from '#lib/gallery/api.js';
import { useLocale } from '#lib/i18n/testing.js';
import GalleryDesignPanel from './GalleryDesignPanel.svelte';

const baseGallery: Gallery = {
	id: 'g1',
	title: 'Camille & Hugo',
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
	last_opened_at: null,
	shoot_date: '2026-06-14',
	states: ['draft'],
	created_at: '2026-06-01T10:00:00Z',
	updated_at: '2026-06-01T10:00:00Z'
};

const readyPhoto = (id: string, name = `${id}.jpg`) => ({
	id,
	original_name: name,
	size: 100,
	width: 1200,
	height: 800,
	status: 'ready' as const,
	clean_ready: true,
	favorited_at: null,
	created_at: '2026-06-01T10:00:00Z'
});

const waitingPhoto = (id: string) => ({
	id,
	original_name: `${id}.jpg`,
	size: 100,
	width: 1200,
	height: 800,
	status: 'waiting' as const,
	clean_ready: false,
	favorited_at: null,
	created_at: '2026-06-01T10:00:00Z'
});

function stubPreviewFetch(
	photos: unknown[] = [readyPhoto('p1', 'Ceremony.jpg'), readyPhoto('p2', 'Dance.jpg')]
) {
	const fetcher = vi.fn<typeof fetch>(async (url) => {
		const path = String(url);

		if (path === '/api/v1/settings') {
			return Response.json({
				studio_name: 'Studio Lune',
				mark: 'name',
				pattern: 'repeat',
				position: 8,
				size: 4,
				signature: 'name',
				signature_size: 4,
				proof_quality: 'medium',
				has_logo: false
			});
		}

		if (path.includes('?watermark=')) {
			return Response.json(
				photos.filter(
					(item): item is { id: string; width: number; height: number; status: string } =>
						typeof item === 'object' && item !== null && 'status' in item && item.status === 'ready'
				)
			);
		}

		return Response.json(photos);
	});

	vi.stubGlobal('fetch', fetcher);

	return fetcher;
}

function setup(over: Partial<Gallery> = {}, saves = true) {
	const onsave = vi.fn().mockResolvedValue(saves);
	let scheduledRun: (() => void) | null = null;
	const schedule = vi.fn((_field: string, run: () => void) => {
		scheduledRun = run;
	});
	const cancel = vi.fn(() => {
		scheduledRun = null;
	});
	const gallery = { ...baseGallery, ...over };
	const view = render(GalleryDesignPanel, {
		gallery,
		heroId: gallery.hero_id,
		focalPoint: gallery.focal_point,
		active: true,
		onsave,
		schedule,
		cancel
	});

	return {
		...view,
		onsave,
		schedule,
		cancel,
		flushFocal: () => scheduledRun?.(),
		user: userEvent.setup()
	};
}

describe('GalleryDesignPanel', () => {
	beforeEach(() => {
		stubPreviewFetch();
	});
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it('renders Cover, Typography, and Photo Grid & Spacing cards plus live Desktop/Phone preview with no axe violations', async () => {
		const { container, user } = setup();

		expect(screen.getByRole('heading', { name: 'Design' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Cover' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Typography' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Photo Grid & Spacing' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Select cover' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Remove cover' })).toBeNull();

		for (const label of ['Full bleed', 'Split editorial', 'Framed paper', 'Minimal banner']) {
			expect(screen.getByRole('button', { name: label })).toBeTruthy();
		}

		const fontGroup = screen.getByRole('group', { name: 'Heading font' });
		for (const [label, typeface] of [
			['Editorial', 'Georgia'],
			['Fine Art', 'Cormorant Garamond Variable'],
			['Timeless', 'Playfair Display Variable'],
			['Modern', 'Plus Jakarta Sans Variable']
		] as const) {
			const button = within(fontGroup).getByRole('button', { name: label });
			const specimen = button.querySelector('[style*="font-family"]') as HTMLElement | null;
			expect(specimen?.style.fontFamily).toContain(typeface);
		}

		expect(screen.getByRole('group', { name: 'Grid density' })).toBeTruthy();
		expect(screen.getByRole('group', { name: 'Spacing' })).toBeTruthy();

		const pad = screen.getByRole('button', { name: /Cover focal point/ }) as HTMLButtonElement;
		expect(pad.disabled).toBe(true);

		const desktop = screen.getByRole('button', { name: 'Desktop' });
		const phone = screen.getByRole('button', { name: 'Phone' });
		expect(desktop.getAttribute('aria-pressed')).toBe('true');
		expect(phone.getAttribute('aria-pressed')).toBe('false');

		await user.click(phone);
		expect(phone.getAttribute('aria-pressed')).toBe('true');
		expect(container.querySelector('[data-frame="phone"]')).toBeTruthy();

		expect((await axe(container)).violations).toEqual([]);
	});

	it('picks, changes, and removes the cover photo over ready proof photos only', async () => {
		stubPreviewFetch([
			readyPhoto('p1', 'Ceremony.jpg'),
			waitingPhoto('p_wait'),
			readyPhoto('p2', 'Dance.jpg')
		]);
		const { container, onsave, user } = setup();

		await user.click(screen.getByRole('button', { name: 'Select cover' }));
		const dialog = screen.getByRole('dialog', { name: 'Choose a cover photo' });
		expect(within(dialog).getByRole('button', { name: 'Use Ceremony.jpg as cover' })).toBeTruthy();
		expect(within(dialog).getByRole('button', { name: 'Use Dance.jpg as cover' })).toBeTruthy();
		expect(within(dialog).queryByRole('button', { name: /p_wait/ })).toBeNull();

		await user.click(within(dialog).getByRole('button', { name: 'Use Ceremony.jpg as cover' }));

		expect(onsave).toHaveBeenCalledWith({ hero_id: 'p1' });
		expect(screen.queryByRole('dialog', { name: 'Choose a cover photo' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Change cover' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Remove cover' })).toBeTruthy();

		const pad = screen.getByRole('button', { name: /Cover focal point/ }) as HTMLButtonElement;
		expect(pad.disabled).toBe(false);
		expect(screen.getByRole('img', { name: 'Gallery cover' }).getAttribute('src')).toBe(
			'/api/v1/galleries/g1/photos/p1/preview'
		);
		expect((await axe(container)).violations).toEqual([]);

		await user.click(screen.getByRole('button', { name: 'Remove cover' }));
		expect(onsave).toHaveBeenCalledWith({ hero_id: null });
		expect(screen.getByRole('button', { name: 'Select cover' })).toBeTruthy();
		expect(pad.disabled).toBe(true);
	});

	it('switches cover templates live in the embedded preview and reverts on failure', async () => {
		const { container, onsave, user } = setup({ hero_id: 'p1' });

		for (const [label, id] of [
			['Split editorial', 'split'],
			['Framed paper', 'framed'],
			['Minimal banner', 'banner'],
			['Full bleed', 'full_bleed']
		] as const) {
			await user.click(screen.getByRole('button', { name: label }));
			expect(container.querySelector(`[data-cover-layout="${id}"]`)).toBeTruthy();
			expect(onsave).toHaveBeenLastCalledWith({ cover_layout: id });
		}

		onsave.mockResolvedValueOnce(false);
		await user.click(screen.getByRole('button', { name: 'Split editorial' }));
		await waitFor(() =>
			expect(screen.getByRole('button', { name: 'Full bleed' }).getAttribute('aria-pressed')).toBe(
				'true'
			)
		);
		expect(container.querySelector('[data-cover-layout="full_bleed"]')).toBeTruthy();
	});

	it('updates focal point immediately on pointer drag, arrow keys, and Reset while debouncing PATCH saves', async () => {
		const { onsave, schedule, cancel, flushFocal, user } = setup({ hero_id: 'p1' });
		const pad = screen.getByRole('button', { name: 'Cover focal point (50%, 35%)' });

		vi.spyOn(pad, 'getBoundingClientRect').mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 200,
			bottom: 100,
			width: 200,
			height: 100,
			toJSON: () => ({})
		});

		await fireEvent.pointerDown(pad, { clientX: 50, clientY: 80, pointerId: 1 });
		await fireEvent.pointerMove(pad, { clientX: 60, clientY: 70, pointerId: 1 });
		await fireEvent.pointerUp(pad, { clientX: 60, clientY: 70, pointerId: 1 });

		const coverImg = screen.getByRole('img', { name: 'Gallery cover' }) as HTMLImageElement;
		expect(coverImg.style.objectPosition).toBe('30% 70%');
		expect(onsave).not.toHaveBeenCalled();
		expect(schedule).toHaveBeenCalledWith('focal_point', expect.any(Function));

		await fireEvent.keyDown(pad, { key: 'ArrowRight' });
		expect(coverImg.style.objectPosition).toBe('31% 70%');

		await fireEvent.keyDown(pad, { key: 'ArrowUp', shiftKey: true });
		expect(coverImg.style.objectPosition).toBe('31% 60%');

		flushFocal();
		await waitFor(() => expect(onsave).toHaveBeenCalledWith({ focal_point: { x: 31, y: 60 } }));

		await user.click(screen.getByRole('button', { name: 'Reset' }));
		expect(cancel).toHaveBeenCalledWith('focal_point');
		expect(onsave).toHaveBeenCalledWith({ focal_point: { x: 50, y: 35 } });
		expect(coverImg.style.objectPosition).toBe('50% 35%');
	});

	it('saves and reverts layout, typography, grid density, and spacing while updating the live preview immediately', async () => {
		const { container, onsave, user } = setup({ hero_id: 'p1' });

		await waitFor(() => expect(screen.getByRole('list', { name: 'Proofs' })).toBeTruthy());

		// Layout
		await user.click(screen.getByRole('button', { name: 'Staggered columns' }));
		expect(onsave).toHaveBeenLastCalledWith({ layout: 'masonry' });
		expect(screen.getByRole('list', { name: 'Proofs' }).getAttribute('data-layout')).toBe(
			'masonry'
		);

		onsave.mockResolvedValueOnce(false);
		await user.click(screen.getByRole('button', { name: 'Even rows' }));
		await waitFor(() =>
			expect(
				screen.getByRole('button', { name: 'Staggered columns' }).getAttribute('aria-pressed')
			).toBe('true')
		);

		// Heading typography
		const fontGroup = screen.getByRole('group', { name: 'Heading font' });
		for (const [label, id, family] of [
			['Fine Art', 'fine_art', "'Cormorant Garamond Variable', Georgia, serif"],
			['Timeless', 'timeless', "'Playfair Display Variable', Georgia, serif"],
			['Modern', 'modern', "'Plus Jakarta Sans Variable', var(--font-sans)"],
			['Editorial', 'editorial', 'Georgia, serif']
		] as const) {
			await user.click(within(fontGroup).getByRole('button', { name: label }));
			expect(onsave).toHaveBeenLastCalledWith({ heading_font: id });
			const previewRoot = container.querySelector('[data-heading-font]') as HTMLElement | null;
			expect(previewRoot?.getAttribute('data-heading-font')).toBe(id);
			expect(previewRoot?.style.getPropertyValue('--gallery-heading-font')).toBe(family);
		}

		// Studio Design heading itself never adopts --gallery-heading-font
		expect(screen.getByRole('heading', { name: 'Design' }).className).not.toContain(
			'font-[family-name:var(--gallery-heading-font)]'
		);

		onsave.mockResolvedValueOnce(false);
		await user.click(within(fontGroup).getByRole('button', { name: 'Modern' }));
		await waitFor(() =>
			expect(
				within(fontGroup).getByRole('button', { name: 'Editorial' }).getAttribute('aria-pressed')
			).toBe('true')
		);
		expect(container.querySelector('[data-heading-font]')?.getAttribute('data-heading-font')).toBe(
			'editorial'
		);

		// Grid density (verify live update in Phone frame too)
		await user.click(screen.getByRole('button', { name: 'Phone' }));
		const densityGroup = screen.getByRole('group', { name: 'Grid density' });
		await user.click(within(densityGroup).getByRole('button', { name: 'Large' }));
		expect(onsave).toHaveBeenLastCalledWith({ grid_density: 'large' });
		expect(screen.getByRole('list', { name: 'Proofs' }).getAttribute('data-density')).toBe('large');
		expect(screen.getByRole('list', { name: 'Proofs' }).className).toContain(
			'@max-[700px]:columns-1'
		);

		onsave.mockResolvedValueOnce(false);
		await user.click(within(densityGroup).getByRole('button', { name: 'Compact' }));
		await waitFor(() =>
			expect(
				within(densityGroup).getByRole('button', { name: 'Large' }).getAttribute('aria-pressed')
			).toBe('true')
		);
		expect(screen.getByRole('list', { name: 'Proofs' }).getAttribute('data-density')).toBe('large');

		// Grid spacing
		const spacingGroup = screen.getByRole('group', { name: 'Spacing' });
		await user.click(within(spacingGroup).getByRole('button', { name: 'Generous' }));
		expect(onsave).toHaveBeenLastCalledWith({ grid_spacing: 'generous' });
		expect(screen.getByRole('list', { name: 'Proofs' }).getAttribute('data-spacing')).toBe(
			'generous'
		);

		onsave.mockResolvedValueOnce(false);
		await user.click(within(spacingGroup).getByRole('button', { name: 'Compact' }));
		await waitFor(() =>
			expect(
				within(spacingGroup).getByRole('button', { name: 'Generous' }).getAttribute('aria-pressed')
			).toBe('true')
		);
		expect(screen.getByRole('list', { name: 'Proofs' }).getAttribute('data-spacing')).toBe(
			'generous'
		);
	});

	it('preserves overlapping preset selections across concurrent gallery updates and ignores stale rollbacks', async () => {
		let resolveFirstFont!: (ok: boolean) => void;
		let resolveDensity!: (ok: boolean) => void;
		const onsave = vi
			.fn()
			.mockImplementationOnce(
				() =>
					new Promise<boolean>((resolve) => {
						resolveFirstFont = resolve;
					})
			)
			.mockImplementationOnce(
				() =>
					new Promise<boolean>((resolve) => {
						resolveDensity = resolve;
					})
			)
			.mockResolvedValue(true);
		const user = userEvent.setup();
		const view = render(GalleryDesignPanel, {
			gallery: { ...baseGallery, hero_id: 'p1' },
			heroId: 'p1',
			focalPoint: { x: 50, y: 35 },
			active: true,
			onsave
		});

		await waitFor(() => expect(screen.getByRole('list', { name: 'Proofs' })).toBeTruthy());

		const fontGroup = screen.getByRole('group', { name: 'Heading font' });
		const densityGroup = screen.getByRole('group', { name: 'Grid density' });

		await user.click(within(fontGroup).getByRole('button', { name: 'Fine Art' }));
		await user.click(within(densityGroup).getByRole('button', { name: 'Large' }));

		// First PATCH resolves and replaces `gallery`, still carrying `grid_density: 'compact'`
		// while `Large` is in flight.
		resolveFirstFont(true);
		await view.rerender({
			gallery: { ...baseGallery, hero_id: 'p1', heading_font: 'fine_art', grid_density: 'compact' },
			heroId: 'p1',
			focalPoint: { x: 50, y: 35 },
			active: true,
			onsave
		});

		expect(
			within(densityGroup).getByRole('button', { name: 'Large' }).getAttribute('aria-pressed')
		).toBe('true');
		expect(screen.getByRole('list', { name: 'Proofs' }).getAttribute('data-density')).toBe('large');

		resolveDensity(true);
		await Promise.resolve();

		// Out-of-order failure of an earlier click in the same group must not overwrite a newer choice.
		let resolveStaleFont!: (ok: boolean) => void;
		onsave.mockImplementationOnce(
			() =>
				new Promise<boolean>((resolve) => {
					resolveStaleFont = resolve;
				})
		);
		await user.click(within(fontGroup).getByRole('button', { name: 'Modern' }));
		await user.click(within(fontGroup).getByRole('button', { name: 'Timeless' }));
		resolveStaleFont(false);
		await Promise.resolve();

		expect(
			within(fontGroup).getByRole('button', { name: 'Timeless' }).getAttribute('aria-pressed')
		).toBe('true');
	});

	it('reflects saved favorites in the embedded live preview once photos load', async () => {
		vi.stubGlobal('IntersectionObserver', undefined);
		stubPreviewFetch([
			{ ...readyPhoto('p1', 'Ceremony.jpg'), favorited_at: '2026-06-02T10:00:00Z' },
			readyPhoto('p2', 'Dance.jpg')
		]);
		setup({ hero_id: 'p1' });

		await waitFor(() =>
			expect(
				screen.getByRole('button', { name: 'Favorite proof 1' }).getAttribute('aria-pressed')
			).toBe('true')
		);
		expect(screen.getByRole('button', { name: /^Favorites/ }).textContent).toContain('1');
	});

	it('keeps a pending focal-point edit across concurrent gallery saves and stale failures', async () => {
		let resolveStale!: (ok: boolean) => void;
		const stalePromise = new Promise<boolean>((resolve) => {
			resolveStale = resolve;
		});
		const onsave = vi
			.fn()
			.mockImplementationOnce(() => stalePromise)
			.mockResolvedValue(true);
		let scheduledRun = null as (() => void) | null;
		const runScheduled = () => scheduledRun?.();
		const schedule = vi.fn((_field: string, run: () => void) => {
			scheduledRun = run;
		});
		const cancel = vi.fn();
		const user = userEvent.setup();
		const view = render(GalleryDesignPanel, {
			gallery: { ...baseGallery, hero_id: 'p1' },
			heroId: 'p1',
			focalPoint: { x: 50, y: 35 },
			active: true,
			onsave,
			schedule,
			cancel
		});

		const pad = screen.getByRole('button', { name: 'Cover focal point (50%, 35%)' });
		await fireEvent.keyDown(pad, { key: 'ArrowRight', shiftKey: true });
		const coverImg = () => screen.getByRole('img', { name: 'Gallery cover' }) as HTMLImageElement;
		expect(coverImg().style.objectPosition).toBe('60% 35%');

		// Fire first focal save (60, 35), then nudge again to (70, 35) while it is in flight,
		// and also rerender with a concurrent cover_layout save that still carries old focal_point (50, 35).
		runScheduled();
		await fireEvent.keyDown(pad, { key: 'ArrowRight', shiftKey: true });
		expect(coverImg().style.objectPosition).toBe('70% 35%');

		await view.rerender({
			gallery: {
				...baseGallery,
				hero_id: 'p1',
				cover_layout: 'split',
				focal_point: { x: 50, y: 35 }
			},
			heroId: 'p1',
			focalPoint: { x: 70, y: 35 },
			active: true,
			onsave,
			schedule,
			cancel
		});
		expect(coverImg().style.objectPosition).toBe('70% 35%');

		// Even if the stale (60, 35) save fails, the newer (70, 35) edit must not be clobbered.
		resolveStale(false);
		await Promise.resolve();
		expect(coverImg().style.objectPosition).toBe('70% 35%');

		// Delivered split cover CTA container wraps cleanly at narrow desktop widths.
		await view.rerender({
			gallery: {
				...baseGallery,
				hero_id: 'p1',
				cover_layout: 'split',
				states: ['shared', 'delivered']
			},
			heroId: 'p1',
			focalPoint: { x: 70, y: 35 },
			active: true,
			onsave,
			schedule,
			cancel
		});
		const ctaRow = screen.getByRole('button', { name: 'View the gallery' }).parentElement;
		expect(ctaRow?.classList.contains('flex-wrap')).toBe(true);

		// Failed photo load surfaces an alert in the cover picker instead of claiming zero uploads.
		vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
		await view.rerender({
			gallery: { ...baseGallery, id: 'g2', hero_id: null },
			heroId: null,
			focalPoint: { x: 50, y: 35 },
			active: true,
			onsave,
			schedule,
			cancel
		});
		await user.click(screen.getByRole('button', { name: 'Select cover' }));
		expect((await screen.findByRole('alert')).textContent).toMatch(/couldn’t load the preview/i);
	});
});

describe('GalleryDesignPanel in French', () => {
	useLocale('fr');

	beforeEach(() => {
		stubPreviewFetch();
	});
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it('renders localized controls and preview with no axe violations', async () => {
		const { container } = setup({ hero_id: 'p1' });

		expect(screen.getByRole('heading', { name: 'Couverture' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Typographie' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Grille photo et espacement' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Changer la couverture' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Retirer la couverture' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Plein cadre' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Éditorial scindé' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Cadre papier' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Bannière épurée' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Éditorial' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Beaux-arts' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Intemporel' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Moderne' })).toBeTruthy();
		expect(screen.getByRole('group', { name: 'Densité de la grille' })).toBeTruthy();
		expect(screen.getByRole('group', { name: 'Espacement' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Téléphone' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});
});
