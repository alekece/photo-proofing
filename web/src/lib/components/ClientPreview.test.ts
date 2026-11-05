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
	share_token: null
} as Gallery;

const photo = (id: string, status = 'ready') => ({
	id,
	original_name: `${id}.jpg`,
	size: 1,
	width: 300,
	height: 200,
	status,
	created_at: '2026-01-01T10:00:00Z'
});

const stub = (photos: unknown[]) => {
	const fetcher = vi.fn<typeof fetch>(async () => new Response(JSON.stringify(photos)));
	vi.stubGlobal('fetch', fetcher);

	return fetcher;
};

afterEach(() => vi.unstubAllGlobals());

describe('ClientPreview', () => {
	it('shows an unshared, expired gallery with its ready photos, from owner routes only', async () => {
		const fetcher = stub([photo('a'), photo('b', 'processing')]);
		render(ClientPreview, { gallery, onclose: () => {} });

		expect(screen.getByRole('dialog', { name: 'Client preview' })).toBeTruthy();
		await screen.findByRole('button', { name: /open proof 1 of 1/i });
		expect(screen.getByText('Preview')).toBeTruthy();
		expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Wedding');

		const urls = fetcher.mock.calls.map(([url]) => String(url));
		expect(urls).toEqual(['/api/v1/galleries/g1/photos']);
		expect(screen.getByRole('img').getAttribute('src')).toBe(
			'/api/v1/galleries/g1/photos/a/thumbnail'
		);
	});

	it('previews a gallery with no photos', async () => {
		stub([]);
		render(ClientPreview, { gallery, onclose: () => {} });

		expect(await screen.findByText(/hasn't added photos yet/i)).toBeTruthy();
	});

	it('says so when the photos cannot be loaded', async () => {
		vi.stubGlobal('fetch', async () => new Response('{}', { status: 500 }));
		render(ClientPreview, { gallery, onclose: () => {} });

		expect((await screen.findByRole('alert')).textContent).toMatch(/couldn.t load/i);
	});

	it('switches between Desktop and Mobile views', async () => {
		stub([]);
		const user = userEvent.setup();
		render(ClientPreview, { gallery, onclose: () => {} });
		const desktop = screen.getByRole('button', { name: 'Desktop view' });
		const mobile = screen.getByRole('button', { name: 'Mobile view' });

		expect(desktop.getAttribute('aria-pressed')).toBe('true');
		expect(mobile.getAttribute('aria-pressed')).toBe('false');
		expect(document.querySelector('[data-frame="desktop"]')).not.toBeNull();

		mobile.focus();
		await user.keyboard('{Enter}');

		expect(mobile.getAttribute('aria-pressed')).toBe('true');
		expect(desktop.getAttribute('aria-pressed')).toBe('false');
		expect(document.querySelector('[data-frame="mobile"]')).not.toBeNull();
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
		const desktop = screen.getByRole('button', { name: 'Desktop view' });

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

	it('keeps the viewer inside the frame in Mobile view and over the page in Desktop view', async () => {
		stub([photo('a')]);
		const user = userEvent.setup();
		render(ClientPreview, { gallery, onclose: () => {} });
		await screen.findByRole('button', { name: /open proof 1 of 1/i });

		await user.click(screen.getByRole('button', { name: /open proof 1 of 1/i }));
		const desktopFrame = document.querySelector('[data-frame="desktop"]') as HTMLElement;
		const viewer = () => screen.getByRole('dialog', { name: 'Proof viewer' });
		expect(desktopFrame.contains(viewer())).toBe(true);
		expect(viewer().classList.contains('absolute')).toBe(false);
		await user.keyboard('{Escape}');

		await user.click(screen.getByRole('button', { name: 'Mobile view' }));
		await user.click(await screen.findByRole('button', { name: /open proof 1 of 1/i }));
		const frame = document.querySelector('[data-frame="mobile"]') as HTMLElement;
		expect(frame.contains(viewer())).toBe(true);
		expect(viewer().classList.contains('absolute')).toBe(true);
	});
});
