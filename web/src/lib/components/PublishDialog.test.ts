import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import type { Photo } from '#lib/gallery/photos.js';
import PublishDialog from './PublishDialog.svelte';

describe('PublishDialog photo set', () => {
	it('counts finals independently of proofs when delivering', async () => {
		const load = vi.fn<typeof import('#lib/gallery/photos.js').listPhotos>(
			async (_id, _fetch, set) => (set === 'finals' ? [] : [{ id: 'p1', status: 'ready' } as Photo])
		);
		render(PublishDialog, {
			kind: 'deliver',
			galleryId: 'g1',
			title: 'Wedding',
			load,
			onconfirm: async () => {},
			onclose: () => {}
		});

		expect(await screen.findByRole('dialog', { name: 'Deliver with no photos?' })).toBeTruthy();
	});
});
