import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ClientGallery from './ClientGallery.svelte';

describe('ClientGallery', () => {
	it('wraps a long unbroken title with a utility class, not an inline style', () => {
		const title = 'x'.repeat(200);
		render(ClientGallery, {
			gallery: { title, layout: 'grid', expires_at: null }
		});
		const heading = screen.getByRole('heading', { level: 1 });
		expect(heading.textContent).toBe(title);
		expect(heading.classList.contains('wrap-anywhere')).toBe(true);
		expect(heading.hasAttribute('style')).toBe(false);
	});

	it('shows a Preview marker only in preview mode', () => {
		const gallery = { title: 'T', layout: 'grid' as const, expires_at: null };
		const { unmount } = render(ClientGallery, { gallery });
		expect(screen.queryByText('Preview')).toBeNull();
		unmount();
		render(ClientGallery, { gallery, preview: true });
		expect(screen.getByText('Preview')).toBeTruthy();
	});
});
