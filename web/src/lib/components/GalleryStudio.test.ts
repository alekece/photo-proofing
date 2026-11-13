import { createRawSnippet } from 'svelte';
import { fireEvent, render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
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
vi.mock('#lib/components/GalleryContactsPanel.svelte', async () => ({
	default: (await import('#lib/components/GalleryContactsPanelStub.test.svelte')).default
}));

import type { Gallery } from '#lib/gallery/api.js';
import GalleryStudio from './GalleryStudio.svelte';

const base: Gallery = {
	id: 'g1',
	title: 'Camille & Lucas',
	layout: 'grid',
	cover_layout: 'full_bleed',
	focal_point: { x: 50, y: 35 },
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	last_opened_at: null,
	shoot_date: null,
	states: ['draft'],
	created_at: '2026-06-12T10:00:00Z',
	updated_at: '2026-06-12T10:00:00Z'
};

const heading = createRawSnippet(() => ({ render: () => '<h1>Camille &amp; Lucas</h1>' }));

function setup(over: Partial<Gallery> = {}, shareToken: string | null = over.share_token ?? null) {
	const handlers = {
		onsave: vi.fn().mockResolvedValue(true),
		schedule: vi.fn(),
		cancel: vi.fn(),
		onpreview: vi.fn(),
		onshare: vi.fn(),
		onpublish: vi.fn(),
		onarchive: vi.fn(),
		onrestore: vi.fn(),
		ondelete: vi.fn()
	};
	const gallery = { ...base, ...over };
	const view = render(GalleryStudio, {
		gallery,
		shareToken,
		origin: 'https://pp.test',
		heading,
		...handlers
	});

	return { ...view, gallery, ...handlers, user: userEvent.setup() };
}

describe('GalleryStudio', () => {
	it('keeps the cover img node across title edits and opens Design when the cover is clicked', async () => {
		const view = setup({ hero_id: 'p1' });
		const coverButton = screen.getByRole('button', { name: 'Edit cover and design' });
		const image = coverButton.querySelector('img');

		expect(image?.getAttribute('src')).toBe('/api/v1/galleries/g1/photos/p1/preview');

		await view.rerender({
			gallery: { ...base, hero_id: 'p1', title: 'Renamed' },
			shareToken: null,
			origin: 'https://pp.test',
			heading,
			onsave: view.onsave,
			schedule: view.schedule,
			cancel: view.cancel,
			onpreview: view.onpreview,
			onshare: view.onshare,
			onpublish: view.onpublish,
			onarchive: view.onarchive,
			onrestore: view.onrestore,
			ondelete: view.ondelete
		});
		expect(coverButton.querySelector('img')).toBe(image);

		await view.user.click(coverButton);
		expect(screen.getByRole('tab', { name: 'Design' }).getAttribute('aria-selected')).toBe('true');
		expect(screen.getByRole('heading', { name: 'Design' })).toBeTruthy();

		const pad = screen.getByRole('button', { name: 'Cover focal point (50%, 35%)' });
		await fireEvent.keyDown(pad, { key: 'ArrowLeft', shiftKey: true });
		expect((image as HTMLImageElement).style.objectPosition).toBe('40% 35%');

		// An unrelated gallery save replacing `gallery` before the debounced focal save lands
		// must not snap the live focal point back to (50, 35).
		await view.rerender({
			gallery: { ...base, hero_id: 'p1', title: 'Renamed again', focal_point: { x: 50, y: 35 } },
			shareToken: null,
			origin: 'https://pp.test',
			heading,
			onsave: view.onsave,
			schedule: view.schedule,
			cancel: view.cancel,
			onpreview: view.onpreview,
			onshare: view.onshare,
			onpublish: view.onpublish,
			onarchive: view.onarchive,
			onrestore: view.onrestore,
			ondelete: view.ondelete
		});
		expect((image as HTMLImageElement).style.objectPosition).toBe('40% 35%');
	});

	it('renders compact CoverBackdrop for an untitled gallery without a cover and passes axe', async () => {
		const { container } = setup({ title: null, hero_id: null });

		expect(screen.getByTestId('no-hero')).toBeTruthy();
		expect(screen.getByText('Draft')).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('formats shoot_date at local noon and shows Last opened only once shared', () => {
		const unshared = setup({ shoot_date: '2026-06-14' });

		expect(unshared.container.textContent).toContain('Shot');
		expect(unshared.container.textContent).toContain('14 Jun 2026');
		expect(unshared.container.textContent).not.toContain('12 Jun 2026');
		expect(screen.queryByText(/Last opened/)).toBeNull();
		unshared.unmount();

		const shared = setup({ share_token: 'T'.repeat(22), states: ['shared'] });
		expect(shared.container.textContent).toContain('Last opened');
		expect(shared.container.textContent).toContain('Not yet');
		expect(screen.getByText('Prepublish')).toBeTruthy();
	});

	it('skips disabled collection tabs during keyboard navigation before sharing and enables them after', async () => {
		const view = setup({ share_token: null, states: ['draft'] });
		const photos = screen.getByRole('tab', { name: /^Photos/ });
		const favorites = screen.getByRole('tab', { name: /^Favorites/ }) as HTMLButtonElement;
		const comments = screen.getByRole('tab', { name: /^Comments/ }) as HTMLButtonElement;
		const finals = screen.getByRole('tab', { name: /^Final/ }) as HTMLButtonElement;
		const design = screen.getByRole('tab', { name: 'Design' });
		const contacts = screen.getByRole('tab', { name: /^Contacts/ }) as HTMLButtonElement;
		const settings = screen.getByRole('tab', { name: 'Settings' });

		expect(favorites.disabled).toBe(true);
		expect(comments.disabled).toBe(true);
		expect(finals.disabled).toBe(true);
		expect(contacts.disabled).toBe(false);
		expect(favorites.title).toBe('Available once the gallery is shared');

		await fireEvent.keyDown(photos, { key: 'ArrowDown' });
		expect(design.getAttribute('aria-selected')).toBe('true');

		await fireEvent.keyDown(design, { key: 'ArrowRight' });
		expect(contacts.getAttribute('aria-selected')).toBe('true');

		await fireEvent.keyDown(contacts, { key: 'ArrowRight' });
		expect(settings.getAttribute('aria-selected')).toBe('true');

		await fireEvent.keyDown(settings, { key: 'ArrowDown' });
		expect(photos.getAttribute('aria-selected')).toBe('true');

		await fireEvent.keyDown(photos, { key: 'End' });
		expect(settings.getAttribute('aria-selected')).toBe('true');

		await view.rerender({
			gallery: { ...base, share_token: 'T'.repeat(22), states: ['shared'] },
			shareToken: 'T'.repeat(22),
			origin: 'https://pp.test',
			heading,
			onsave: view.onsave,
			schedule: view.schedule,
			cancel: view.cancel,
			onpreview: view.onpreview,
			onshare: view.onshare,
			onpublish: view.onpublish,
			onarchive: view.onarchive,
			onrestore: view.onrestore,
			ondelete: view.ondelete
		});

		expect(favorites.disabled).toBe(false);
		expect(comments.disabled).toBe(false);
		expect(finals.disabled).toBe(false);
		await fireEvent.keyDown(photos, { key: 'ArrowDown' });
		expect(favorites.getAttribute('aria-selected')).toBe('true');
	});
});
