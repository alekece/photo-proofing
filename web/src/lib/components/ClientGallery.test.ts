import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Proof, SharedGallery } from '#lib/gallery/types.js';
import ClientGallery from './ClientGallery.svelte';

const gallery: SharedGallery = {
	title: 'Wedding',
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	delivered: false,
	hero_id: null
};
const proof = (id: string): Proof => ({
	id,
	alt: `Proof ${id}`,
	thumbnailUrl: `/thumb/${id}`,
	previewUrl: `/preview/${id}`,
	width: 300,
	height: 200
});
const proofs: Proof[] = [1, 2, 3].map((n) => ({
	id: `p${n}`,
	alt: `Proof ${n}`,
	thumbnailUrl: `data:,t${n}`,
	previewUrl: `data:,p${n}`,
	width: 300,
	height: 200
}));

const shared = (hero_id: string | null): SharedGallery => ({ ...gallery, hero_id });

const show = (props: Partial<Parameters<typeof render<typeof ClientGallery>>[1]> = {}) =>
	render(ClientGallery, { token: 'tok', gallery, proofs, favorites: [], ...props });
const hearts = () => screen.getAllByRole('button', { name: /^favorite proof/i });
const favoritesToggle = () => screen.getByRole('button', { name: 'Favorites' });
const count = () => favoritesToggle().textContent?.trim();

describe('ClientGallery', () => {
	const fetcher = vi.fn();

	beforeEach(() => {
		fetcher.mockReset();
		fetcher.mockResolvedValue(new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetcher);
	});
	afterEach(() => vi.unstubAllGlobals());

	it('wraps a long unbroken title with a utility class, not an inline style', () => {
		const title = 'x'.repeat(200);
		show({ gallery: { ...gallery, title } });
		const heading = screen.getByRole('heading', { level: 1 });
		expect(heading.textContent).toBe(title);
		expect(heading.classList.contains('wrap-anywhere')).toBe(true);
		expect(heading.hasAttribute('style')).toBe(false);
	});

	it('shows what is already selected and counts it', () => {
		show({ favorites: ['p2'] });

		expect(hearts().map((heart) => heart.getAttribute('aria-pressed'))).toEqual([
			'false',
			'true',
			'false'
		]);
		expect(count()).toBe('1');
	});

	it('selects a photo at once and saves it', async () => {
		const user = userEvent.setup();
		show();

		await user.click(hearts()[0]);

		expect(hearts()[0].getAttribute('aria-pressed')).toBe('true');
		expect(count()).toBe('1');
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/shared/tok/favorites/p1',
			expect.objectContaining({ method: 'PUT' })
		);
	});

	it('puts the choice back and says so when saving fails', async () => {
		const user = userEvent.setup();
		fetcher.mockResolvedValue(new Response(null, { status: 500 }));
		show();

		await user.click(hearts()[0]);

		await waitFor(() => expect(screen.getByRole('alert').textContent).toMatch(/couldn.t save/i));
		expect(hearts()[0].getAttribute('aria-pressed')).toBe('false');
	});

	it('selects from the viewer too', async () => {
		const user = userEvent.setup();
		show();

		await user.click(screen.getByRole('button', { name: 'Open proof 2 of 3' }));

		const dialog = screen.getByRole('dialog');
		const heart = dialog.querySelector<HTMLButtonElement>('button[aria-pressed]')!;

		await user.click(heart);

		expect(heart.getAttribute('aria-pressed')).toBe('true');
		expect(fetcher).toHaveBeenCalledWith(
			expect.stringContaining('/favorites/p2'),
			expect.objectContaining({ method: 'PUT' })
		);
	});

	it('shows the package allowance and warns about extras in words', () => {
		show({ gallery: { ...gallery, included_photos: 1 }, favorites: ['p1', 'p2', 'p3'] });

		expect(count()).toBe('3 of 1');
		expect(screen.getByText('2 extra').classList.contains('font-bold')).toBe(true);
	});

	it('shows no extras within the allowance', () => {
		show({ gallery: { ...gallery, included_photos: 5 }, favorites: ['p1'] });

		expect(count()).toBe('1 of 5');
		expect(screen.queryByText(/extra/)).toBeNull();
	});

	it('shows no allowance when selection is unlimited', () => {
		show({ favorites: ['p1'] });

		expect(count()).toBe('1');
	});

	it('names the deadline on the cover', () => {
		show({ gallery: { ...gallery, expires_at: '2999-01-01T00:00:00Z' } });

		expect(screen.getAllByText(/Open until/).length).toBeGreaterThan(0);
	});

	it('offers downloads instead of hearts once delivered', () => {
		show({
			gallery: { ...gallery, delivered: true },
			proofs: [{ ...proof('a'), downloadUrl: '/final/a' }],
			downloadAllUrl: '/all.zip',
			favorites: ['a']
		});

		expect(screen.queryAllByRole('button', { name: /^favorite proof/i })).toEqual([]);
		expect(screen.queryByRole('button', { name: 'Favorites' })).toBeNull();
		expect(screen.getByText('Your photos are ready')).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Download photo 1' }).getAttribute('href')).toBe(
			'/final/a'
		);
		for (const link of screen.getAllByRole('link', { name: /download all/i }))
			expect(link.getAttribute('href')).toBe('/all.zip');
	});

	it('filters to favorites and says when there are none', async () => {
		const user = userEvent.setup();
		show({ favorites: ['p2'] });

		await user.click(screen.getByRole('button', { name: 'Favorites' }));

		expect(hearts()).toHaveLength(1);

		await user.click(screen.getByRole('button', { name: 'All' }));

		expect(hearts()).toHaveLength(3);

		await user.click(screen.getByRole('button', { name: 'Favorites' }));
		await user.click(hearts()[0]);

		expect(screen.getByText('No favorites yet')).toBeTruthy();
	});

	it('keeps the viewer on its photo when unfavoriting under the filter', async () => {
		const user = userEvent.setup();
		show({ favorites: ['p3'] });

		await user.click(screen.getByRole('button', { name: 'Favorites' }));
		await user.click(screen.getByRole('button', { name: 'Open proof 1 of 1' }));
		await user.click(screen.getByRole('button', { name: 'Favorite' }));

		expect(screen.getByText('1 of 1')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Add to favorites' })).toBeTruthy();
	});

	it('signs the cover and footer with the studio', () => {
		show({ studio: 'Studio Lune' });

		expect(screen.getAllByText('Studio Lune')).toHaveLength(1);
		expect(screen.getByRole('contentinfo').textContent).toContain(
			'Studio Lune · PhotoProofing'.toUpperCase()
		);
	});

	it('shows the favorites count in the viewer', async () => {
		const user = userEvent.setup();
		show({ favorites: ['p1', 'p2'] });

		await user.click(screen.getByRole('button', { name: 'Open proof 3 of 3' }));

		expect(screen.getByText('Favorites:').parentElement?.textContent).toContain('2');
	});

	it('shows the selection but cannot change it in preview mode', () => {
		show({ preview: true, favorites: ['p1'] });

		expect(count()).toBe('1');
		const hearts = screen.getAllByRole('button', { name: /^favorite proof/i });
		expect(hearts).toHaveLength(3);
		for (const heart of hearts) expect((heart as HTMLButtonElement).disabled).toBe(true);
		expect(
			screen.getByRole('button', { name: 'Favorite proof 1' }).getAttribute('aria-pressed')
		).toBe('true');
	});

	it('shows the hero photo preview as the cover', () => {
		show({ gallery: shared('b'), proofs: [proof('a'), proof('b')] });

		expect(screen.getByRole('img', { name: 'Gallery cover' }).getAttribute('src')).toBe(
			'/preview/b'
		);
	});

	it('shows no cover without a hero, or when its photo is not among the proofs', () => {
		const { unmount } = show({ gallery: shared(null), proofs: [proof('a')] });

		expect(screen.queryByRole('img', { name: 'Gallery cover' })).toBeNull();
		expect(screen.getByTestId('no-hero')).toBeTruthy();

		unmount();
		show({ gallery: shared('gone'), proofs: [proof('a')] });

		expect(screen.queryByRole('img', { name: 'Gallery cover' })).toBeNull();
	});
});
