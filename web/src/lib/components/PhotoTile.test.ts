import { fireEvent, render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import PhotoTile from './PhotoTile.svelte';

const base = { name: 'a.jpg', state: 'ready', thumbnailUrl: '/t', onremove: () => {} } as const;

it('keeps the collection circle and active tile body separately operable by keyboard', async () => {
	const user = userEvent.setup();
	const onselect = vi.fn();
	const onopen = vi.fn();
	const props = { ...base, onselect, onopen };
	const view = render(PhotoTile, { ...props, collectionSelect: 'idle' });
	const circle = screen.getByRole('button', { name: 'Select a.jpg' });
	circle.focus();
	await user.keyboard(' ');
	expect(onselect).toHaveBeenCalledOnce();
	expect(onopen).not.toHaveBeenCalled();

	await view.rerender({ ...props, collectionSelect: 'active', selected: true });
	expect(circle.getAttribute('aria-pressed')).toBe('true');
	screen.getByRole('button', { name: 'a.jpg' }).focus();
	await user.keyboard('{Enter}');
	expect(onselect).toHaveBeenCalledTimes(2);
	expect(onopen).not.toHaveBeenCalled();

	await view.rerender({ ...props, collectionSelect: undefined, persistentSelect: true });
	await user.click(screen.getByRole('button', { name: 'Open a.jpg' }));
	expect(onopen).toHaveBeenCalledOnce();
	expect(onselect).toHaveBeenCalledTimes(2);
});

describe('PhotoTile states', () => {
	it.each(['processing', 'process-failed'] as const)(
		'keeps a prepared clean photo usable during %s',
		async (state) => {
			const onopen = vi.fn();
			const onretry = vi.fn();
			render(PhotoTile, { ...base, state, cleanReady: true, onopen, onretry });

			expect(screen.getByRole('img', { name: 'a.jpg' }).getAttribute('src')).toBe('/t');
			await userEvent.click(screen.getByRole('button', { name: 'Open a.jpg' }));
			expect(onopen).toHaveBeenCalledOnce();

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

	it('ripples the select circle only when a photo becomes selected', async () => {
		const user = userEvent.setup();
		const onselect = vi.fn();
		const view = render(PhotoTile, { ...base, onselect });
		const circle = screen.getByRole('checkbox', { name: /select a\.jpg/i });

		await user.click(circle);
		await view.rerender({ ...base, onselect, selected: true });

		expect(screen.getAllByTestId('click-ripple')).toHaveLength(1);
		expect(circle.parentElement?.contains(screen.getByTestId('click-ripple'))).toBe(true);
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

	it('opens and offers a persistent select circle at the same time', async () => {
		const onopen = vi.fn();
		const onselect = vi.fn();
		const user = userEvent.setup();

		render(PhotoTile, {
			...base,
			onopen,
			onselect,
			persistentSelect: true,
			source: 'Wedding'
		});

		await user.click(screen.getByRole('button', { name: 'Open a.jpg' }));
		expect(onopen).toHaveBeenCalledOnce();
		expect(onselect).not.toHaveBeenCalled();

		await user.click(screen.getByRole('button', { name: /select a\.jpg/i }));
		expect(onselect).toHaveBeenCalledOnce();
		expect(onopen).toHaveBeenCalledOnce();

		expect(screen.getByText('Wedding')).toBeTruthy();
	});

	it('marks the persistent select circle pressed once selected', async () => {
		const view = render(PhotoTile, {
			...base,
			onopen: () => {},
			onselect: () => {},
			persistentSelect: true
		});
		const circle = screen.getByRole('button', { name: /select a\.jpg/i });

		expect(circle.getAttribute('aria-pressed')).toBe('false');

		await view.rerender({
			...base,
			onopen: () => {},
			onselect: () => {},
			persistentSelect: true,
			selected: true
		});

		expect(circle.getAttribute('aria-pressed')).toBe('true');
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

	it('dates a photo the client has downloaded or favorited', async () => {
		const view = render(PhotoTile, {
			...base,
			caption: { kind: 'downloaded', at: '2026-11-09T12:00:00Z' }
		});

		expect(screen.getByText(/^Downloaded Nov 9, 2026/)).toBeTruthy();

		await view.rerender({ ...base, caption: { kind: 'favorited', at: '2026-11-09T12:00:00Z' } });

		expect(screen.getByText(/^Favorited Nov 9, 2026/)).toBeTruthy();
	});

	it('shows no date until the photo is ready', () => {
		render(PhotoTile, {
			...base,
			state: 'processing',
			caption: { kind: 'favorited', at: '2026-11-09T12:00:00Z' }
		});

		expect(screen.queryByText(/Favorited/)).toBeNull();
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

describe('PhotoTile loading polish', () => {
	it('shimmers while a photo is not ready yet', () => {
		const { container } = render(PhotoTile, { name: 'a.jpg', state: 'uploading' });

		expect(container.querySelector('.animate-shimmer')).not.toBeNull();
	});

	it('fades the image in once it has loaded', async () => {
		render(PhotoTile, base);
		const image = screen.getByRole('img', { name: 'a.jpg' });

		expect(image.className).toContain('opacity-0');

		await fireEvent.load(image);

		expect(image.className).not.toContain('opacity-0');
		expect(image.className).toContain('animate-tile-in');
	});

	it('shows reprocessing like an upload: the label above the bar, over the old image', () => {
		const { container } = render(PhotoTile, { ...base, state: 'processing', cleanReady: true });
		const text = screen.getByText('Processing');
		const bar = container.querySelector('.bar')!.parentElement!;

		expect(screen.getByRole('img', { name: 'a.jpg' })).toBeTruthy();
		expect(text.compareDocumentPosition(bar) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
		expect(container.querySelector('.bg-panel\\/95')).toBeNull();
	});

	it('keeps Retry when reprocessing fails', async () => {
		const onretry = vi.fn();
		const { container } = render(PhotoTile, {
			...base,
			state: 'process-failed',
			cleanReady: true,
			onretry
		});

		await userEvent.click(screen.getByRole('button', { name: 'Retry a.jpg' }));

		expect(onretry).toHaveBeenCalledOnce();
		expect(container.querySelector('.bar')).toBeNull();
	});

	it.each([
		['favorited', 'text-danger'],
		['downloaded', 'bg-dark']
	] as const)('draws the %s caption icon with its own contrast treatment', (kind, color) => {
		const { container } = render(PhotoTile, {
			...base,
			caption: { kind, at: '2026-11-09T12:00:00Z' }
		});
		const icon = container.querySelector('p svg')!;

		expect(icon.getAttribute('width')).toBe('16');
		expect(icon.getAttribute('class')).toContain(color);
	});

	it('shows a thumbnail that failed to load instead of hiding it', async () => {
		render(PhotoTile, base);
		const img = screen.getByRole('img');

		await fireEvent.error(img);

		expect(img.className).not.toContain('opacity-0');
	});
});
