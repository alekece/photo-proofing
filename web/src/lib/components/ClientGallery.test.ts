import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import type { Proof, SharedGallery } from '#lib/gallery/types.js';
import ClientGallery from './ClientGallery.svelte';

const proof = (id: string): Proof => ({
	id,
	alt: `Proof ${id}`,
	thumbnailUrl: `/thumb/${id}`,
	previewUrl: `/preview/${id}`,
	width: 300,
	height: 200
});
const shared = (hero_photo_id: string | null): SharedGallery => ({
	title: 'Wedding',
	layout: 'grid',
	expires_at: null,
	hero_photo_id
});

describe('ClientGallery', () => {
	it('wraps a long unbroken title with a utility class, not an inline style', () => {
		const title = 'x'.repeat(200);
		render(ClientGallery, {
			gallery: { title, layout: 'grid', expires_at: null, hero_photo_id: null }
		});
		const heading = screen.getByRole('heading', { level: 1 });
		expect(heading.textContent).toBe(title);
		expect(heading.classList.contains('wrap-anywhere')).toBe(true);
		expect(heading.hasAttribute('style')).toBe(false);
	});

	it('shows a Preview marker only in preview mode', () => {
		const gallery = { title: 'T', layout: 'grid' as const, expires_at: null, hero_photo_id: null };
		const { unmount } = render(ClientGallery, { gallery });
		expect(screen.queryByText('Preview')).toBeNull();
		unmount();
		render(ClientGallery, { gallery, preview: true });
		expect(screen.getByText('Preview')).toBeTruthy();
	});

	it('shows the hero photo preview as the cover', () => {
		render(ClientGallery, { gallery: shared('b'), proofs: [proof('a'), proof('b')] });

		expect(screen.getByRole('img', { name: 'Gallery cover' }).getAttribute('src')).toBe(
			'/preview/b'
		);
	});

	it('shows no cover without a hero, or when its photo is not among the proofs', () => {
		const { unmount } = render(ClientGallery, { gallery: shared(null), proofs: [proof('a')] });

		expect(screen.queryByRole('img', { name: 'Gallery cover' })).toBeNull();

		unmount();
		render(ClientGallery, { gallery: shared('gone'), proofs: [proof('a')] });

		expect(screen.queryByRole('img', { name: 'Gallery cover' })).toBeNull();
	});
});
