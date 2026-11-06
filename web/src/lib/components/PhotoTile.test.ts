import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import PhotoTile from './PhotoTile.svelte';

const base = { name: 'a.jpg', state: 'ready', thumbnailUrl: '/t', onremove: () => {} } as const;

describe('PhotoTile hero star', () => {
	it('is a toggle named after what a click does', async () => {
		const onhero = vi.fn();
		const user = userEvent.setup();
		const view = render(PhotoTile, { ...base, onhero });
		const star = screen.getByRole('button', { name: 'Set as hero' });

		expect(star.getAttribute('aria-pressed')).toBe('false');

		await user.click(star);

		expect(onhero).toHaveBeenCalledTimes(1);

		await view.rerender({ ...base, onhero, hero: true });

		expect(screen.getByRole('button', { name: 'Remove hero' }).getAttribute('aria-pressed')).toBe(
			'true'
		);
	});

	it('is reachable by keyboard', async () => {
		const onhero = vi.fn();
		const user = userEvent.setup();

		render(PhotoTile, { ...base, onhero });
		screen.getByRole('button', { name: 'Open a.jpg' }).focus();
		await user.tab();

		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Set as hero' }));

		await user.keyboard('{Enter}');

		expect(onhero).toHaveBeenCalledTimes(1);
	});

	it('is absent without a handler and on photos that are not ready', () => {
		render(PhotoTile, base);
		render(PhotoTile, { ...base, state: 'processing', onhero: () => {} });

		expect(screen.queryByRole('button', { name: /hero/ })).toBeNull();
	});
});

describe('PhotoTile states', () => {
	it('shows a failed upload with its reason, Retry and ×', async () => {
		const onretry = vi.fn();
		const onremove = vi.fn();

		render(PhotoTile, {
			name: 'a.jpg',
			state: 'upload-failed',
			error: 'Storage full',
			onremove,
			onretry
		});

		expect(screen.getByText('Upload failed')).toBeTruthy();
		expect(screen.getByText('Storage full')).toBeTruthy();

		await userEvent.click(screen.getByRole('button', { name: 'Retry a.jpg' }));
		await userEvent.click(screen.getByRole('button', { name: 'Remove a.jpg' }));

		expect(onretry).toHaveBeenCalledOnce();
		expect(onremove).toHaveBeenCalledOnce();
	});

	it.each([
		['waiting-upload', 'Waiting'],
		['uploading', 'Uploading'],
		['processing', 'Processing']
	] as const)('labels %s with a bar', (state, label) => {
		const { container } = render(PhotoTile, { ...base, state });

		expect(screen.getByText(label)).toBeTruthy();
		expect(container.querySelector('.bar')).toBeTruthy();
	});

	it('is read-only without onremove and offers a download when given one', async () => {
		const ondownload = vi.fn();

		render(PhotoTile, { name: 'a.jpg', state: 'ready', thumbnailUrl: '/t', ondownload });

		expect(screen.queryByRole('button', { name: 'Remove a.jpg' })).toBeNull();

		await userEvent.click(screen.getByRole('button', { name: 'Download a.jpg' }));

		expect(ondownload).toHaveBeenCalledOnce();
	});

	it('marks a photo the client has downloaded', () => {
		render(PhotoTile, { ...base, downloaded: true });

		expect(screen.getByRole('img', { name: 'Downloaded by the client' })).toBeTruthy();
	});
});
