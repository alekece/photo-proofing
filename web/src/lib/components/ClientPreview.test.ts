import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Gallery } from '#lib/gallery/api.js';
import ClientPreview from './ClientPreview.svelte';

const gallery = {
	id: 'g1',
	title: 'Wedding',
	layout: 'grid',
	expires_at: '2020-01-01T00:00:00Z',
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
	const fetcher = vi.fn<typeof fetch>(
		async (url) =>
			new Response(
				JSON.stringify(
					String(url) === '/api/v1/settings'
						? {
								studio_name: '',
								mark: 'name',
								pattern: 'repeat',
								position: 8,
								size: 2,
								signature: 'name',
								proof_quality: 'medium',
								has_logo: false
							}
						: String(url).includes('?watermark=')
							? photos.filter(
									(photo) =>
										typeof photo === 'object' &&
										photo !== null &&
										'status' in photo &&
										photo.status === 'ready'
								)
							: photos
				)
			)
	);
	vi.stubGlobal('fetch', fetcher);

	return fetcher;
};

afterEach(() => vi.unstubAllGlobals());

describe('ClientPreview', () => {
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

	it('uses saved studio signature and owner logo without a client credential', async () => {
		vi.stubGlobal(
			'fetch',
			async (url: RequestInfo | URL) =>
				new Response(
					JSON.stringify(
						String(url) === '/api/v1/settings'
							? {
									studio_name: 'Studio',
									mark: 'name',
									pattern: 'repeat',
									position: 8,
									size: 2,
									signature: 'both',
									proof_quality: 'medium',
									has_logo: true
								}
							: []
					)
				)
		);
		render(ClientPreview, { gallery, onclose: () => {} });

		await screen.findByText('Your photos are on their way.');
		expect(screen.getAllByText('Studio').length).toBeGreaterThan(0);
		expect(
			document.querySelector('[data-testid="studio-signature"] img')?.getAttribute('src')
		).toBe('/api/v1/settings/logo');
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
			'/api/v1/galleries/g1/photos?watermark=false',
			'/api/v1/settings'
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
});
