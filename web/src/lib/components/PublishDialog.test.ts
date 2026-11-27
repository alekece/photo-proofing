import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';
import { thumbnailUrl, type Photo } from '#lib/gallery/photos.js';
import PublishDialog from './PublishDialog.svelte';

describe('PublishDialog photo set', () => {
	it('counts finals independently of proofs when delivering', async () => {
		const load = vi.fn<typeof import('#lib/gallery/photos.js').listPhotos>(
			async (_id, _fetch, set) => (set === 'finals' ? [] : [{ id: 'p1', status: 'ready' } as Photo])
		);
		render(PublishDialog, {
			kind: 'deliver',
			galleryId: 'g1',
			load,
			onconfirm: async () => {},
			onclose: () => {}
		});

		expect(
			await screen.findByRole('dialog', { name: 'Deliver with no final photos?' })
		).toBeTruthy();
	});

	it.each([
		['deliver', 'finals'],
		['share', 'photos']
	] as const)('shows %s thumbnails from the %s set', async (kind, set) => {
		const load = vi.fn<typeof import('#lib/gallery/photos.js').listPhotos>(async () => [
			{ id: 'p1', status: 'ready' } as Photo
		]);
		const { baseElement } = render(PublishDialog, {
			kind,
			galleryId: 'g1',
			load,
			onconfirm: async () => {},
			onclose: () => {}
		});

		await screen.findByRole('dialog');
		await vi.waitFor(() => {
			const src = baseElement.querySelector('[data-slot="photo-stack"] img')?.getAttribute('src');
			expect(src).toBe(thumbnailUrl('g1', 'p1', set));
		});
	});
});

describe('PublishDialog dismissal', () => {
	const props = {
		kind: 'share',
		galleryId: 'g1',
		load: async () => [{ id: 'p1', status: 'ready' } as Photo],
		onconfirm: async () => {}
	} as const;

	it('dismisses with Cancel by default', async () => {
		const onclose = vi.fn();
		render(PublishDialog, { ...props, onclose });

		await userEvent.click(await screen.findByRole('button', { name: 'Cancel' }));

		expect(screen.queryByRole('button', { name: 'Not yet' })).toBeNull();
		expect(onclose).toHaveBeenCalled();
	});

	it('returns to the originating screen without offering the link to copy', async () => {
		const onclose = vi.fn();
		render(PublishDialog, { ...props, onclose });

		await userEvent.click(await screen.findByRole('button', { name: 'Share' }));
		expect(onclose).toHaveBeenCalledOnce();

		expect(screen.queryByRole('textbox')).toBeNull();
		expect(screen.queryByRole('button', { name: /copy/i })).toBeNull();
	});
});

describe('PublishDialog in French', () => {
	useLocale('fr');

	it('asks to publish the photos in French', async () => {
		render(PublishDialog, {
			kind: 'deliver',
			galleryId: 'g1',
			load: async () => [{ id: 'p1', status: 'ready' } as Photo],
			onconfirm: async () => {},
			onclose: () => {}
		});

		expect(await screen.findByRole('dialog', { name: 'Livrer 1 photo retouchée ?' })).toBeTruthy();
		expect(screen.queryByText(/Camille & Hugo/)).toBeNull();
		expect(screen.getByRole('button', { name: 'Livrer' })).toBeTruthy();
		expect(screen.queryByText(/Publish/)).toBeNull();
	});
});
