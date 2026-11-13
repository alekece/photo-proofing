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

	it('renders Cover card, Look card, and live Desktop/Phone preview with no axe violations', async () => {
		const { container, user } = setup();

		expect(screen.getByRole('heading', { name: 'Design' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Cover' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Look' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Select cover' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Remove cover' })).toBeNull();

		for (const label of ['Full bleed', 'Split editorial', 'Framed paper', 'Minimal banner']) {
			expect(screen.getByRole('button', { name: label })).toBeTruthy();
		}

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

	it('saves and reverts the Look layout', async () => {
		const { onsave, user } = setup();

		await user.click(screen.getByRole('button', { name: 'Staggered columns' }));
		expect(onsave).toHaveBeenCalledWith({ layout: 'masonry' });

		onsave.mockResolvedValueOnce(false);
		await user.click(screen.getByRole('button', { name: 'Even rows' }));
		await waitFor(() =>
			expect(
				screen.getByRole('button', { name: 'Staggered columns' }).getAttribute('aria-pressed')
			).toBe('true')
		);
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
		expect(screen.getByRole('button', { name: 'Changer la couverture' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Retirer la couverture' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Plein cadre' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Éditorial scindé' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Cadre papier' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Bannière épurée' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Téléphone' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});
});
