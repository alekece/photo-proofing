import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('#lib/components/PhotosPanel.svelte', async () => ({
	default: (await import('#lib/components/PhotosPanelStub.test.svelte')).default
}));
vi.mock('#lib/components/FavoritesPanel.svelte', async () => ({
	default: (await import('#lib/components/FavoritesPanelStub.test.svelte')).default
}));

vi.mock('#lib/components/CommentsPanel.svelte', async () => ({
	default: (await import('#lib/components/CommentsPanelStub.test.svelte')).default
}));

import GalleryTabs from './GalleryTabs.svelte';

describe('gallery tab availability', () => {
	it('keeps Favorites and Final visible but unavailable before sharing', async () => {
		render(GalleryTabs, { galleryId: 'g1', included: null, shared: false });
		const photos = screen.getByRole('tab', { name: 'Photos' });
		const favorites = screen.getByRole('tab', { name: 'Favorites' });
		const comments = screen.getByRole('tab', { name: 'Comments' });
		const finals = screen.getByRole('tab', { name: 'Final' });

		expect((favorites as HTMLButtonElement).disabled).toBe(true);
		expect((comments as HTMLButtonElement).disabled).toBe(true);
		expect((finals as HTMLButtonElement).disabled).toBe(true);
		expect(favorites.title).toBe('Available once the gallery is shared');
		expect(finals.title).toBe('Available once the gallery is shared');
		await fireEvent.keyDown(photos, { key: 'ArrowRight' });
		expect(photos.getAttribute('aria-selected')).toBe('true');
		await fireEvent.keyDown(photos, { key: 'End' });
		expect(photos.getAttribute('aria-selected')).toBe('true');
	});

	it('enables both tabs immediately after sharing', async () => {
		const { rerender } = render(GalleryTabs, {
			galleryId: 'g1',
			included: null,
			shared: false
		});

		await rerender({ galleryId: 'g1', included: null, shared: true });
		const photos = screen.getByRole('tab', { name: 'Photos' });
		const favorites = screen.getByRole('tab', { name: 'Favorites' });
		const finals = screen.getByRole('tab', { name: 'Final' });

		expect((favorites as HTMLButtonElement).disabled).toBe(false);
		expect((finals as HTMLButtonElement).disabled).toBe(false);
		expect(favorites.getAttribute('title')).toBeNull();
		await fireEvent.keyDown(photos, { key: 'ArrowRight' });
		expect(favorites.getAttribute('aria-selected')).toBe('true');
		await fireEvent.keyDown(favorites, { key: 'End' });
		expect(finals.getAttribute('aria-selected')).toBe('true');
	});
});
