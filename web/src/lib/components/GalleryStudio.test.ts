import { createRawSnippet } from 'svelte';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('#lib/components/PhotosPanel.svelte', async () => ({
	default: (await import('#lib/components/PhotosPanelStub.test.svelte')).default
}));
vi.mock('#lib/components/FavoritesPanel.svelte', async () => ({
	default: (await import('#lib/components/FavoritesPanelStub.test.svelte')).default
}));
vi.mock('#lib/components/CommentsPanel.svelte', async () => ({
	default: (await import('#lib/components/CommentsPanelStub.test.svelte')).default
}));
vi.mock('#lib/components/GalleryActivityPanel.svelte', async () => ({
	default: (await import('#lib/components/GalleryActivityPanelStub.test.svelte')).default
}));
vi.mock('#lib/components/GalleryContactsPanel.svelte', async () => ({
	default: (await import('#lib/components/GalleryContactsPanelStub.test.svelte')).default
}));

import type { Gallery } from '#lib/gallery/api.js';
import { DEFAULT_SETTINGS } from '#lib/settings/settings.js';
import GalleryStudio from './GalleryStudio.svelte';

const base: Gallery = {
	id: 'g1',
	title: 'Camille & Lucas',
	favorite: false,
	layout: 'grid',
	cover_layout: 'full_bleed',
	focal_point: { x: 50, y: 35 },
	heading_font: 'editorial',
	grid_density: 'compact',
	grid_spacing: 'compact',
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

afterEach(() => vi.unstubAllGlobals());

function setup(
	over: Partial<Gallery> = {},
	shareToken: string | null = over.share_token ?? null,
	requestedPhoto?: { id: string; kind: 'proof' | 'final' }
) {
	const handlers = {
		onsave: vi.fn().mockResolvedValue(true),
		schedule: vi.fn(),
		cancel: vi.fn(),
		onbeforepreview: vi.fn(),
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
		requestedPhoto,
		heading,
		...handlers
	});

	return { ...view, gallery, ...handlers, user: userEvent.setup() };
}

describe('GalleryStudio', () => {
	it.each(['proof', 'final'] as const)(
		'selects the source %s panel for a requested photo',
		(kind) => {
			setup({}, null, { id: 'p2', kind });
			expect(screen.getByRole('tabpanel').id).toBe(
				kind === 'final' ? 'panel-final' : 'panel-photos'
			);
		}
	);

	it('opens the gallery activity once shared, and not before', async () => {
		setup({ share_token: null, states: ['draft'] });

		expect((screen.getByRole('tab', { name: 'Activity' }) as HTMLButtonElement).disabled).toBe(
			true
		);

		cleanup();

		const view = setup({ share_token: 'tok', states: ['shared'] }, 'tok');
		const tab = screen.getByRole('tab', { name: 'Activity' });

		await view.user.click(tab);

		expect(tab.getAttribute('aria-selected')).toBe('true');
		expect(screen.getByText('Activity of g1')).toBeTruthy();
	});

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
			onbeforepreview: view.onbeforepreview,
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
		expect(screen.getByRole('heading', { name: 'Typography' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Photo Grid & Spacing' })).toBeTruthy();

		const pad = screen.getByRole('button', { name: 'Cover focal point' });
		await fireEvent.keyDown(pad, { key: 'ArrowLeft', shiftKey: true });
		expect((image as HTMLImageElement).style.objectPosition).toBe('40% 35%');

		// Unrelated gallery updates with the old focal point must not undo the local edit.
		await view.rerender({
			gallery: { ...base, hero_id: 'p1', title: 'Renamed again', focal_point: { x: 50, y: 35 } },
			shareToken: null,
			origin: 'https://pp.test',
			heading,
			onsave: view.onsave,
			schedule: view.schedule,
			cancel: view.cancel,
			onbeforepreview: view.onbeforepreview,
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

	it('wires every tab aria-controls to a mounted tabpanel and hides inactive panels', async () => {
		const { container, user } = setup({ share_token: null, states: ['draft'] });
		const assertTabPanels = (activeTabId: string) => {
			const tabs = screen.getAllByRole('tab');
			expect(tabs).toHaveLength(9);

			for (const tab of tabs) {
				const controlsId = tab.getAttribute('aria-controls');
				expect(controlsId).toBeTruthy();
				const panel = document.getElementById(controlsId!);
				expect(panel).not.toBeNull();
				expect(panel?.getAttribute('role')).toBe('tabpanel');
				expect(panel?.getAttribute('aria-labelledby')).toBe(tab.id);
				expect(panel?.hidden).toBe(tab.id !== activeTabId);
			}
		};

		assertTabPanels('tab-photos');

		await user.click(screen.getByRole('tab', { name: 'Design' }));
		assertTabPanels('tab-design');
		expect((await axe(container)).violations).toEqual([]);
	});

	it('formats shoot_date at local noon and hides Last opened until the first visit', () => {
		const unshared = setup({ shoot_date: '2026-06-14' });

		expect(unshared.container.textContent).toContain('Shot');
		expect(unshared.container.textContent).toContain('14 Jun 2026');
		expect(unshared.container.textContent).not.toContain('12 Jun 2026');
		expect(screen.queryByText(/Last opened/)).toBeNull();
		unshared.unmount();

		const shared = setup({ share_token: 'T'.repeat(22), states: ['shared'] });
		expect(shared.container.textContent).not.toContain('Last opened');
		expect(shared.container.textContent).not.toContain('Created');
		expect(shared.container.textContent).not.toContain('12 Jun 2026');
		expect(screen.getByText('Shared')).toBeTruthy();
	});

	it('skips disabled collection tabs during keyboard navigation before sharing and enables them after', async () => {
		const view = setup({ share_token: null, states: ['draft'] });
		const photos = screen.getByRole('tab', { name: /^Photos/ });
		const favorites = screen.getByRole('tab', { name: /^Favorites/ }) as HTMLButtonElement;
		const comments = screen.getByRole('tab', { name: /^Comments/ }) as HTMLButtonElement;
		const finals = screen.getByRole('tab', { name: /^Retouched/ }) as HTMLButtonElement;
		const design = screen.getByRole('tab', { name: 'Design' });
		const share = screen.getByRole('tab', { name: 'Share' }) as HTMLButtonElement;
		const contacts = screen.getByRole('tab', { name: /^Contacts/ }) as HTMLButtonElement;
		const settings = screen.getByRole('tab', { name: 'Settings' });

		expect(favorites.disabled).toBe(true);
		expect(comments.disabled).toBe(true);
		expect(finals.disabled).toBe(true);
		expect(share.disabled).toBe(false);
		expect(contacts.disabled).toBe(false);
		expect(favorites.title).toBe('Available once the gallery is shared');

		await fireEvent.keyDown(photos, { key: 'ArrowDown' });
		expect(design.getAttribute('aria-selected')).toBe('true');

		await fireEvent.keyDown(design, { key: 'ArrowRight' });
		expect(share.getAttribute('aria-selected')).toBe('true');

		await fireEvent.keyDown(share, { key: 'ArrowRight' });
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
			onbeforepreview: view.onbeforepreview,
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

	it('toggles gallery favorite pin via onsave and reverts on failure', async () => {
		const ok = setup({ favorite: false });
		const pin = screen.getByRole('button', { name: 'Pin gallery to top' });
		expect(pin.getAttribute('aria-pressed')).toBe('false');

		await ok.user.click(pin);
		expect(ok.onsave).toHaveBeenCalledWith({ favorite: true });
		expect(screen.getByRole('button', { name: 'Unpin gallery' }).getAttribute('aria-pressed')).toBe(
			'true'
		);
		expect((await axe(ok.container)).violations).toEqual([]);
		ok.unmount();

		const failing = setup({ favorite: true });
		failing.onsave.mockResolvedValueOnce(false);
		await failing.user.click(screen.getByRole('button', { name: 'Unpin gallery' }));
		await vi.waitFor(() =>
			expect(
				screen.getByRole('button', { name: 'Unpin gallery' }).getAttribute('aria-pressed')
			).toBe('true')
		);
	});

	it('opens Studio → Share from the top bar, returns on Cancel, and disables Share when archived', async () => {
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('[]')));
		const view = setup({ share_token: null, states: ['draft'] });

		await view.user.click(screen.getByRole('tab', { name: 'Design' }));
		await view.user.click(screen.getByRole('button', { name: 'Go to Share' }));

		expect(screen.getByRole('tab', { name: 'Share' }).getAttribute('aria-selected')).toBe('true');
		expect(view.onshare).not.toHaveBeenCalled();
		expect(screen.getByRole('button', { name: 'Cancel' })).toBeTruthy();

		await view.user.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(screen.getByRole('tab', { name: 'Design' }).getAttribute('aria-selected')).toBe('true');

		// Archive disables the Share tab and resets section to Photos if Share was active
		await view.user.click(screen.getByRole('tab', { name: 'Share' }));
		await view.rerender({
			gallery: { ...base, states: ['draft', 'archived'] },
			shareToken: null,
			origin: 'https://pp.test',
			heading,
			onsave: view.onsave,
			schedule: view.schedule,
			cancel: view.cancel,
			onbeforepreview: view.onbeforepreview,
			onshare: view.onshare,
			onpublish: view.onpublish,
			onarchive: view.onarchive,
			onrestore: view.onrestore,
			ondelete: view.ondelete
		});

		const shareTab = screen.getByRole('tab', { name: 'Share' }) as HTMLButtonElement;
		expect(shareTab.disabled).toBe(true);
		expect(screen.getByRole('tab', { name: /^Photos/ }).getAttribute('aria-selected')).toBe('true');
		vi.unstubAllGlobals();
	});

	it('keeps the gallery identity stable and free of a preview action across every collection section', async () => {
		const view = setup({ share_token: 'T'.repeat(22), states: ['shared'], hero_id: 'p1' });
		const identity = document.querySelector('[data-slot="gallery-identity"]');

		expect(identity).not.toBeNull();

		const sections = [
			'Photos',
			'Favorites',
			'Comments',
			'Retouched',
			'Activity',
			'Design',
			'Contacts'
		];

		for (const name of sections) {
			const tab = screen.getByRole('tab', { name: new RegExp(`^${name}`) });

			await view.user.click(tab);

			// Identity keeps the same cover, title and favorite control; no preview action lives here.
			expect(document.querySelector('[data-slot="gallery-identity"]')).toBe(identity);
			expect(identity?.querySelector('button[aria-label="Edit cover and design"]')).not.toBeNull();
			expect(
				within(identity as HTMLElement).getByRole('button', { name: 'Pin gallery to top' })
			).toBeTruthy();
			expect(identity?.textContent).toContain('Camille & Lucas');
			expect(within(identity as HTMLElement).queryByRole('button', { name: 'Preview' })).toBeNull();
		}
	});

	it('keeps the journey in one header, even on a section without actions', async () => {
		const view = setup({ share_token: 'tok', states: ['shared'] });
		const header = view.container.querySelector<HTMLElement>(
			'[data-slot="gallery-content-header"]'
		)!;
		const journey = within(header).getByRole('navigation', { name: 'Gallery progress' });

		await view.user.click(screen.getByRole('tab', { name: 'Design' }));
		expect(within(header).getByRole('button', { name: 'Open full preview' })).toBeTruthy();
		expect(screen.getAllByRole('button', { name: 'Open full preview' })).toHaveLength(1);
		await view.user.click(screen.getByRole('tab', { name: 'Activity' }));
		expect(within(header).getByRole('navigation', { name: 'Gallery progress' })).toBe(journey);
		expect(within(header).queryByRole('button', { name: 'Open full preview' })).toBeNull();
	});

	it('keeps the live cover preview in Design instead of Settings', async () => {
		const fetcher = vi.fn<typeof fetch>(async () => Response.json([]));
		vi.stubGlobal('fetch', fetcher);
		const view = setup({ hero_id: 'saved' });
		expect(fetcher).not.toHaveBeenCalled();
		await view.rerender({
			heroId: 'live',
			heroRevision: 7,
			settings: { ...DEFAULT_SETTINGS, studioName: 'Studio Lune' }
		});
		await view.user.click(screen.getByRole('tab', { name: 'Settings' }));
		expect(screen.queryByRole('region', { name: 'Client preview' })).toBeNull();
		await view.user.click(screen.getByRole('tab', { name: 'Design' }));
		await view.user.click(screen.getByRole('button', { name: 'Open full preview' }));
		const preview = screen.getByRole('dialog', { name: 'Client preview' });
		const cover = await within(preview).findByRole('img', { name: 'Gallery cover' });
		expect(cover.getAttribute('src')).toBe('/api/v1/galleries/g1/photos/live/preview?revision=7');
		expect(within(preview).getAllByText('Studio Lune').length).toBeGreaterThan(0);
		expect(
			fetcher.mock.calls.every(([url]) => String(url).startsWith('/api/v1/galleries/g1/'))
		).toBe(true);
	});

	it('groups Gallery identity on the stable cover and Activity under Studio', async () => {
		const view = setup({ share_token: 'T'.repeat(22), states: ['shared'], hero_id: 'p1' });
		const cover = screen.getByRole('button', { name: 'Edit cover and design' });
		const pin = screen.getByRole('button', { name: 'Pin gallery to top' });
		expect(pin.parentElement).toBe(cover.parentElement);
		expect(cover.parentElement?.textContent).toContain('Shared');
		await view.user.click(pin);
		expect(view.onsave).toHaveBeenCalledExactlyOnceWith({ favorite: true });
		expect(screen.getByRole('tabpanel', { name: /^Photos/ })).toBeTruthy();
		const design = screen.getByRole('tab', { name: 'Design' });
		const activity = screen.getByRole('tab', { name: 'Activity' });
		expect(activity.parentElement).toBe(design.parentElement);
		expect(screen.getByRole('tab', { name: /^Comments/ }).parentElement).toBe(
			activity.parentElement
		);
		await fireEvent.keyDown(screen.getByRole('tab', { name: /^Retouched/ }), { key: 'ArrowRight' });
		expect(document.activeElement).toBe(design);
		await fireEvent.keyDown(design, { key: 'ArrowRight' });
		expect(document.activeElement).toBe(activity);
		expect(screen.getByRole('button', { name: 'Edit cover and design' })).toBe(cover);
	});
});
