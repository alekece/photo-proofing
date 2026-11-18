import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Gallery } from '#lib/gallery/api.js';
import { DEFAULT_SETTINGS } from '#lib/settings/settings.js';
import ClientPreview from './ClientPreview.svelte';

const gallery = {
	id: 'g1',
	title: 'Wedding',
	layout: 'grid',
	expires_at: '2020-01-01',
	share_token: null,
	states: ['draft'],
	watermark_enabled: false
} as Gallery;

const photo = (id: string, status = 'ready') => ({
	id,
	original_name: `${id}.jpg`,
	size: 1,
	width: 300,
	height: 200,
	status,
	favorited_at: null,
	created_at: '2026-01-01T10:00:00Z'
});

const stub = (photos: unknown[]) => {
	const fetcher = vi.fn<typeof fetch>(async (url) => {
		const path = String(url);
		if (path === '/api/v1/settings') {
			throw new Error('ClientPreview must not fetch /api/v1/settings');
		}
		return new Response(
			JSON.stringify(
				path.includes('?watermark=')
					? photos.filter(
							(photo) =>
								typeof photo === 'object' &&
								photo !== null &&
								'status' in photo &&
								photo.status === 'ready'
						)
					: photos
			)
		);
	});
	vi.stubGlobal('fetch', fetcher);

	return fetcher;
};

afterEach(() => vi.unstubAllGlobals());

describe('ClientPreview', () => {
	it('shows a delivered gallery with its finals only, as the client sees it', async () => {
		const fetcher = vi.fn<typeof fetch>(async (url) => {
			const path = String(url);

			if (path === '/api/v1/settings') {
				throw new Error('ClientPreview must not fetch /api/v1/settings');
			}
			if (path.endsWith('?finals=true')) return Response.json([photo('f1'), photo('f2')]);

			return Response.json([photo('a'), photo('b'), photo('c')]);
		});
		vi.stubGlobal('fetch', fetcher);
		render(ClientPreview, {
			gallery: { ...gallery, states: ['shared', 'delivered'], hero_id: 'a' },
			heroRevision: 3,
			onclose: () => {}
		});

		await screen.findAllByRole('button', { name: /download/i });
		expect(screen.queryByRole('button', { name: /favorite/i })).toBeNull();
		const thumbnails = screen
			.getAllByRole('img')
			.filter((image) => image.getAttribute('alt') !== 'Gallery cover');

		expect(thumbnails.map((image) => image.getAttribute('src'))).toEqual([
			'/api/v1/galleries/g1/photos/f1/thumbnail?finals=true',
			'/api/v1/galleries/g1/photos/f2/thumbnail?finals=true'
		]);
		await userEvent.setup().click(screen.getByRole('button', { name: 'Slideshow' }));
		expect(screen.getByRole('button', { name: 'Pause slideshow' })).toBeTruthy();
		await userEvent.setup().click(screen.getByRole('button', { name: 'Close viewer' }));
		expect(screen.getByRole('img', { name: 'Gallery cover' }).getAttribute('src')).toBe(
			'/api/v1/galleries/g1/photos/a/preview?revision=3'
		);
		expect(fetcher.mock.calls.map(([url]) => String(url))).toEqual([
			'/api/v1/galleries/g1/photos?finals=true'
		]);
	});

	it('refreshes mounted client media when the gallery switch changes', async () => {
		const fetcher = stub([photo('a')]);
		const onclose = () => {};
		const view = render(ClientPreview, { gallery, onclose });

		await waitFor(() =>
			expect(screen.getByRole('img').getAttribute('src')).toContain('watermark=false')
		);
		await view.rerender({ gallery: { ...gallery, watermark_enabled: true }, onclose });

		await waitFor(() =>
			expect(screen.getByRole('img').getAttribute('src')).toContain('watermark=true')
		);
		expect(fetcher.mock.calls.filter(([url]) => String(url).includes('?watermark='))).toHaveLength(
			2
		);
	});

	it('keeps the clean hero when its marked copy is not ready', async () => {
		stub([{ ...photo('hero', 'processing'), clean_ready: true }]);
		render(ClientPreview, {
			gallery: { ...gallery, hero_id: 'hero', watermark_enabled: true },
			onclose: () => {}
		});

		await screen.findByText('Your photos are on their way.');
		expect(screen.getByRole('img', { name: 'Gallery cover' }).getAttribute('src')).toBe(
			'/api/v1/galleries/g1/photos/hero/preview'
		);
	});

	it('uses SSR studio settings prop and owner logo without fetching /api/v1/settings', async () => {
		const fetcher = stub([]);
		render(ClientPreview, {
			gallery,
			settings: {
				...DEFAULT_SETTINGS,
				studioName: 'Studio',
				signature: 'both',
				logo: '/api/v1/settings/logo'
			},
			onclose: () => {}
		});

		await screen.findByText('Your photos are on their way.');
		expect(screen.getAllByText('Studio').length).toBeGreaterThan(0);
		expect(
			document.querySelector('[data-testid="studio-signature"] img')?.getAttribute('src')
		).toBe('/api/v1/settings/logo');
		expect(fetcher.mock.calls.some(([url]) => String(url) === '/api/v1/settings')).toBe(false);
	});

	it('shows an unshared, expired gallery with its ready photos, from owner routes only', async () => {
		const fetcher = stub([photo('a'), photo('b', 'processing')]);
		render(ClientPreview, { gallery, onclose: () => {} });

		expect(screen.getByRole('dialog', { name: 'Client preview' })).toBeTruthy();
		await screen.findByRole('button', { name: /open proof 1 of 1/i });
		expect(screen.getByText('Preview')).toBeTruthy();
		expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Wedding');

		const urls = fetcher.mock.calls.map(([url]) => String(url));
		expect(urls).toEqual([
			'/api/v1/galleries/g1/photos',
			'/api/v1/galleries/g1/photos?watermark=false'
		]);
		expect(screen.getByRole('img').getAttribute('src')).toBe(
			'/api/v1/galleries/g1/photos/a/thumbnail?watermark=false'
		);
	});

	it('previews a gallery with no photos', async () => {
		stub([]);
		render(ClientPreview, { gallery, onclose: () => {} });

		expect(await screen.findByText('Your photos are on their way.')).toBeTruthy();
	});

	it('says so when the photos cannot be loaded', async () => {
		vi.stubGlobal('fetch', async () => new Response('{}', { status: 500 }));
		render(ClientPreview, { gallery, onclose: () => {} });

		expect((await screen.findByRole('alert')).textContent).toMatch(/couldn.t load/i);
	});

	it('switches between Desktop and Phone views', async () => {
		stub([]);
		const user = userEvent.setup();
		render(ClientPreview, { gallery, onclose: () => {} });
		const desktop = screen.getByRole('button', { name: 'Desktop' });
		const mobile = screen.getByRole('button', { name: 'Phone' });

		expect(desktop.getAttribute('aria-pressed')).toBe('true');
		expect(mobile.getAttribute('aria-pressed')).toBe('false');
		expect(document.querySelector('[data-frame="desktop"]')).not.toBeNull();

		mobile.focus();
		await user.keyboard('{Enter}');

		expect(mobile.getAttribute('aria-pressed')).toBe('true');
		expect(desktop.getAttribute('aria-pressed')).toBe('false');
		expect(document.querySelector('[data-frame="phone"]')).not.toBeNull();
	});

	it('closes with the Close button and with Escape', async () => {
		stub([]);
		const user = userEvent.setup();
		const onclose = vi.fn();
		render(ClientPreview, { gallery, onclose });

		await user.click(screen.getByRole('button', { name: 'Close preview' }));
		expect(onclose).toHaveBeenCalledTimes(1);

		await user.keyboard('{Escape}');
		expect(onclose).toHaveBeenCalledTimes(2);
	});

	it('keeps Tab inside the overlay', async () => {
		stub([]);
		const user = userEvent.setup();
		render(ClientPreview, { gallery, onclose: () => {} });
		const close = screen.getByRole('button', { name: 'Close preview' });
		const desktop = screen.getByRole('button', { name: 'Desktop' });

		close.focus();
		await user.tab();
		expect(document.activeElement).toBe(desktop);

		await user.tab({ shift: true });
		expect(document.activeElement).toBe(close);
	});

	it('opens the viewer without dismissing the preview on its Escape', async () => {
		stub([photo('a')]);
		const user = userEvent.setup();
		const onclose = vi.fn();
		render(ClientPreview, { gallery, onclose });

		await user.click(await screen.findByRole('button', { name: /open proof 1 of 1/i }));
		await waitFor(() => screen.getByRole('dialog', { name: 'Proof viewer' }));
		await user.keyboard('{Escape}');

		expect(onclose).not.toHaveBeenCalled();
		expect(screen.queryByRole('dialog', { name: 'Proof viewer' })).toBeNull();
	});

	it('opens the photo viewer over the whole screen in both views', async () => {
		stub([photo('a')]);
		const user = userEvent.setup();
		render(ClientPreview, { gallery, onclose: () => {} });
		const viewer = () => screen.getByRole('dialog', { name: 'Proof viewer' });

		await user.click(await screen.findByRole('button', { name: /open proof 1 of 1/i }));
		expect(viewer()).toBeTruthy();
		await user.keyboard('{Escape}');

		await user.click(screen.getByRole('button', { name: 'Phone' }));
		await user.click(await screen.findByRole('button', { name: /open proof 1 of 1/i }));
		expect(viewer()).toBeTruthy();
	});

	describe('inline variant', () => {
		it('renders an embedded stage without a dialog, fixed height or its own scrollbar', async () => {
			stub([]);
			render(ClientPreview, { gallery, variant: 'inline', onexpand: () => {} });

			await screen.findByText('Your photos are on their way.');
			expect(screen.queryByRole('dialog')).toBeNull();
			expect(screen.queryByRole('button', { name: 'Close preview' })).toBeNull();

			const stage = document.querySelector('[data-frame="desktop"]')?.parentElement as HTMLElement;

			expect(stage.className).not.toMatch(/overflow-auto/);
			expect(stage.className).not.toMatch(/h-dvh|max-h-dvh|h-\[/);
		});

		it('offers an Open full preview action instead of Close, and keeps switching views', async () => {
			stub([]);
			const user = userEvent.setup();
			const onexpand = vi.fn();
			render(ClientPreview, { gallery, variant: 'inline', onexpand });

			await user.click(screen.getByRole('button', { name: 'Open full preview' }));
			expect(onexpand).toHaveBeenCalledTimes(1);

			const mobile = screen.getByRole('button', { name: 'Phone' });

			await user.click(mobile);
			expect(mobile.getAttribute('aria-pressed')).toBe('true');
			expect(document.querySelector('[data-frame="phone"]')).not.toBeNull();
		});

		it('does not lock background scroll or steal focus, unlike the modal variant', async () => {
			stub([]);
			const input = document.createElement('input');

			document.body.appendChild(input);
			input.focus();
			render(ClientPreview, { gallery, variant: 'inline', onexpand: () => {} });

			await screen.findByText('Your photos are on their way.');
			expect(document.activeElement).toBe(input);
			expect(document.body.style.overflow).not.toBe('hidden');
			input.remove();
		});
	});
});
