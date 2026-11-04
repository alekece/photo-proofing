import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ProofThumbnail from './ProofThumbnail.svelte';

const proof = {
	id: 'p1',
	alt: 'Proof 1',
	thumbnailUrl: 'data:,thumbnail',
	previewUrl: 'data:,preview',
	width: 200,
	height: 600
};
const thumbnail = (layout: 'grid' | 'masonry') => {
	render(ProofThumbnail, { proof, layout, position: 1, total: 1, onopen: () => {} });

	return screen.getByRole('img');
};

describe('ProofThumbnail', () => {
	it('shows the thumbnail with its real size', () => {
		const img = thumbnail('grid');

		expect(img.getAttribute('src')).toBe('data:,thumbnail');
		expect(img.getAttribute('width')).toBe('200');
		expect(img.getAttribute('height')).toBe('600');
	});

	it('crops to 3:2 in the grid and keeps the photo shape in masonry', () => {
		expect(thumbnail('grid').classList.contains('aspect-3/2')).toBe(true);
		document.body.innerHTML = '';
		expect(thumbnail('masonry').classList.contains('aspect-3/2')).toBe(false);
	});
});
