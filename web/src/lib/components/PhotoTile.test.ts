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
		const star = screen.getByRole('button', { name: 'Set as cover' });

		expect(star.getAttribute('aria-pressed')).toBe('false');

		await user.click(star);

		expect(onhero).toHaveBeenCalledTimes(1);

		await view.rerender({ ...base, onhero, hero: true });

		expect(screen.getByRole('button', { name: 'Remove cover' }).getAttribute('aria-pressed')).toBe(
			'true'
		);
	});

	it('is reachable by keyboard', async () => {
		const onhero = vi.fn();
		const user = userEvent.setup();

		render(PhotoTile, { ...base, onhero });
		screen.getByRole('button', { name: 'Open a.jpg' }).focus();
		await user.tab();

		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Set as cover' }));

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
	it.each(['processing', 'process-failed'] as const)(
		'keeps a prepared clean photo usable during %s',
		async (state) => {
			const onopen = vi.fn();
			const onhero = vi.fn();
			const onretry = vi.fn();
			render(PhotoTile, { ...base, state, cleanReady: true, onopen, onhero, onretry });

			expect(screen.getByRole('img', { name: 'a.jpg' }).getAttribute('src')).toBe('/t');
			await userEvent.click(screen.getByRole('button', { name: 'Open a.jpg' }));
			await userEvent.click(screen.getByRole('button', { name: 'Set as cover' }));

			expect(onopen).toHaveBeenCalledOnce();
			expect(onhero).toHaveBeenCalledOnce();

			if (state === 'process-failed') {
				await userEvent.click(screen.getByRole('button', { name: 'Retry a.jpg' }));
				expect(onretry).toHaveBeenCalledOnce();
			}
		}
	);

	it('centers processing feedback inside the aspect-ratio tile', () => {
		render(PhotoTile, { name: 'a.jpg', state: 'processing', ratio: 3 / 2 });
		expect(screen.getByText('Processing').parentElement?.className).toContain('absolute inset-0');
	});

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

	it('offers no Retry on a failed tile in select mode', () => {
		render(PhotoTile, {
			name: 'a.jpg',
			state: 'upload-failed',
			onretry: () => {},
			onselect: () => {}
		});

		expect(screen.queryByRole('button', { name: 'Retry a.jpg' })).toBeNull();
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

describe('PhotoTile upload states', () => {
	const cases = [
		['waiting-upload', 'Waiting'],
		['uploading', 'Uploading'],
		['waiting-process', 'Waiting'],
		['processing', 'Processing']
	] as const;

	it.each(cases)('shows the %s label as a status above the progress bar', (state, label) => {
		const { container } = render(PhotoTile, { name: 'a.jpg', state });
		const text = screen.getByText(label);
		const bar = container.querySelector('.bar')!.parentElement!;

		expect(text.parentElement!.className).toContain('text-xs');
		expect(text.className).toContain('font-semibold');
		expect(
			text.compareDocumentPosition(bar) & Node.DOCUMENT_POSITION_FOLLOWING,
			'the bar follows the label'
		).toBeTruthy();
		expect(text.parentElement!.className).toContain('pb-5');
	});

	it('shows a failed upload in the same type style, without a progress bar', () => {
		const { container } = render(PhotoTile, { name: 'a.jpg', state: 'upload-failed' });

		expect(screen.getByText('Upload failed').className).toContain('font-semibold');
		expect(container.querySelector('.bar')).toBeNull();
	});
});
