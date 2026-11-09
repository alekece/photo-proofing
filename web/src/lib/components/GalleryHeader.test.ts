import { createRawSnippet } from 'svelte';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { useLocale } from '#lib/i18n/testing.js';

import type { Gallery } from '#lib/gallery/api.js';

import GalleryHeader from './GalleryHeader.svelte';

const base: Gallery = {
	id: 'g1',
	title: 'Camille & Hugo',
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	last_opened_at: null,
	states: ['draft'],
	created_at: '2026-06-12T10:00:00Z',
	updated_at: '2026-06-12T10:00:00Z'
};
const heading = createRawSnippet(() => ({ render: () => '<h1>Camille &amp; Hugo</h1>' }));

function setup(over: Partial<Gallery> = {}, counts: { photos?: number; favorites?: number } = {}) {
	const handlers = {
		onpreview: vi.fn(),
		onshare: vi.fn(),
		ondeliver: vi.fn(),
		onrestore: vi.fn()
	};
	const view = render(GalleryHeader, {
		gallery: { ...base, ...over },
		photoCount: counts.photos ?? null,
		favoriteCount: counts.favorites ?? null,
		heading,
		...handlers
	});

	return { ...view, ...handlers, user: userEvent.setup() };
}

describe('GalleryHeader', () => {
	it('uses the hero render revision without replacing the image on a title edit', async () => {
		const props = {
			gallery: { ...base, hero_id: 'p1' },
			heroRevision: 2,
			heading,
			onpreview: vi.fn(),
			onshare: vi.fn(),
			ondeliver: vi.fn(),
			onrestore: vi.fn()
		};
		const view = render(GalleryHeader, props);
		const image = view.container.querySelector('img');

		expect(image?.getAttribute('src')).toBe('/api/v1/galleries/g1/photos/p1/preview?revision=2');
		await view.rerender({ ...props, gallery: { ...props.gallery, title: 'New title' } });
		expect(view.container.querySelector('img')).toBe(image);
	});

	it('offers Share on a draft and lists the journey with Upload done', async () => {
		const { onshare, user } = setup({}, { photos: 48 });

		expect(screen.getByText('Draft')).toBeTruthy();
		expect(screen.getByText(/^48 photos · created /)).toBeTruthy();
		expect(screen.getByText('48 photos', { selector: '[data-slot="step-fact"]' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Share' }));

		expect(onshare).toHaveBeenCalledOnce();
	});

	it('offers Deliver once shared and counts the selection', async () => {
		const { ondeliver, user } = setup(
			{ share_token: 'T'.repeat(22), states: ['shared'] },
			{ photos: 3, favorites: 13 }
		);

		expect(screen.getByText('13 selected', { selector: '[data-slot="step-fact"]' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Deliver' }));

		expect(ondeliver).toHaveBeenCalledOnce();
	});

	it('shows when the client last opened a shared gallery, or that it has not', () => {
		const shared = { share_token: 'T'.repeat(22), states: ['shared' as const] };
		const { unmount } = setup(shared, { photos: 3 });

		expect(screen.getByText(/ · Not opened yet$/)).toBeTruthy();

		unmount();
		setup({ ...shared, last_opened_at: '2026-11-09T14:38:00Z' }, { photos: 3 });

		expect(screen.getByText(/ · Last opened \w{3} \d{1,2}, 2026/)).toBeTruthy();
	});

	it('shows nothing about opens before the gallery is shared', () => {
		setup({}, { photos: 3 });

		expect(screen.queryByText(/opened/i)).toBeNull();
	});

	it('shows the status only in the badge, not in the meta line or the steps', () => {
		setup({ share_token: 'T'.repeat(22), states: ['delivered'] }, { photos: 3 });

		expect(screen.getByText('Delivered')).toBeTruthy();
		expect(screen.getAllByText('done', { selector: '.sr-only' }).length).toBeGreaterThan(0);
		expect(screen.queryByText(/delivered|shared/i, { selector: 'p' })).toBeNull();
		expect(
			screen.queryByText(/^(Shared|Delivered)$/, { selector: '[data-slot="step-fact"]' })
		).toBeNull();
	});

	it('has no main action once delivered', () => {
		setup({ share_token: 'T'.repeat(22), states: ['delivered'] }, { photos: 3 });

		expect(screen.getByRole('button', { name: 'Preview' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Share' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Deliver' })).toBeNull();
	});

	it('offers Restore and hides the journey when archived', async () => {
		const { onrestore, user } = setup({ states: ['draft', 'archived'] });

		expect(screen.queryByRole('navigation', { name: 'Gallery progress' })).toBeNull();

		await user.click(screen.getByRole('button', { name: 'Restore' }));

		expect(onrestore).toHaveBeenCalledOnce();
	});

	it('uses the hero photo when there is one and initials when there is not', () => {
		const { container, unmount } = setup({ hero_id: 'p1' });

		expect(container.querySelector('img')?.getAttribute('src')).toContain('/p1/preview');
		expect(screen.queryByTestId('no-hero')).toBeNull();

		unmount();
		setup();

		expect(screen.getByTestId('no-hero')).toBeTruthy();
		expect(screen.getByText('CH')).toBeTruthy();
	});

	it('has no axe violations', async () => {
		const { container } = setup({}, { photos: 2 });

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('GalleryHeader in French', () => {
	useLocale('fr');

	it('words the last open in French', () => {
		setup(
			{ share_token: 'T'.repeat(22), states: ['shared'], last_opened_at: '2026-11-09T14:38:00Z' },
			{ photos: 3 }
		);

		expect(screen.getByText(/ · Dernière ouverture le \d{1,2} nov\. 2026/)).toBeTruthy();
	});

	it('shows the actions and the next step in French', () => {
		setup();

		expect(screen.getByRole('button', { name: /Aperçu/ })).toBeTruthy();
		expect(screen.getByText('Ajoutez vos épreuves')).toBeTruthy();
		expect(screen.queryByText('Add your proofs')).toBeNull();
	});
});
