import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
import type { Proof, SharedGallery } from '#lib/gallery/types.js';
import { useLocale } from '#lib/i18n/testing.js';
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
const favoritesToggle = () => screen.getByRole('button', { name: /^Favorites/ });
const count = () => favoritesToggle().textContent?.replace('Favorites', '').trim();

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

	it('emphasises the selected count over the allowance without a separate extra count', () => {
		show({ gallery: { ...gallery, included_photos: 1 }, favorites: ['p1', 'p2', 'p3'] });

		expect(count()).toBe('3 of 1');
		expect(screen.getByText('3').classList.contains('text-warning')).toBe(true);
		expect(screen.getByRole('button', { name: 'Favorites 3 of 1' })).toBeTruthy();
		expect(screen.queryByText(/extra/)).toBeNull();
	});

	it('keeps the selected count plain within the allowance', () => {
		show({ gallery: { ...gallery, included_photos: 5 }, favorites: ['p1'] });

		expect(count()).toBe('1 of 5');
		expect(screen.getByText('1').classList.contains('text-warning')).toBe(false);
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
			favorites: ['a']
		});

		expect(screen.queryAllByRole('button', { name: /^favorite proof/i })).toEqual([]);
		expect(screen.queryByRole('button', { name: /^Favorites/ })).toBeNull();
		expect(screen.getByText('Your photos are ready')).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Download photo 1' }).getAttribute('href')).toBe(
			'/final/a'
		);
		expect(screen.getAllByRole('button', { name: /download all/i }).length).toBeGreaterThan(0);
	});

	it('prepares the ZIP, then downloads it, and apologises when it fails', async () => {
		const user = userEvent.setup();
		const answers = [202, 200];
		const fetcher = vi.fn(async () => new Response('{}', { status: answers.shift() ?? 503 }));

		vi.stubGlobal('fetch', fetcher);
		show({ token: 't', gallery: { ...gallery, delivered: true } });

		await user.click(screen.getAllByRole('button', { name: /download all/i })[0]);

		expect(await screen.findAllByText('Downloading…')).not.toHaveLength(0);
		await vi.waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2), { timeout: 4000 });
		expect(fetcher).toHaveBeenCalledWith('/api/v1/galleries/shared/t/download', {
			method: 'POST'
		});

		await user.click(
			await screen.findAllByRole('button', { name: /download all/i }).then((b) => b[0])
		);

		await vi.waitFor(() =>
			expect(toasts.items.map((toast) => toast.message)).toContain(
				'Couldn’t prepare the download. Please try again.'
			)
		);
		vi.unstubAllGlobals();
	});

	describe('sticky bar', () => {
		let report: (isIntersecting: boolean) => void;

		beforeEach(() => {
			vi.stubGlobal(
				'IntersectionObserver',
				class {
					constructor(callback: (entries: { isIntersecting: boolean }[]) => void) {
						report = (isIntersecting) => callback([{ isIntersecting }]);
					}
					observe() {}
					disconnect() {}
				}
			);
		});

		const delivered = () => show({ gallery: { ...gallery, delivered: true } });

		it('is absent while the hero is visible, so the title and download appear once', async () => {
			delivered();
			await tick();

			expect(screen.getAllByRole('heading', { name: 'Wedding' })).toHaveLength(1);
			expect(screen.getAllByRole('button', { name: /download all/i })).toHaveLength(1);
			expect(screen.queryByRole('heading', { level: 2 })).toBeNull();
		});

		it('shows the title and download once the hero scrolls out of view', async () => {
			delivered();
			await tick();
			report(false);
			await tick();

			expect(screen.getByRole('heading', { level: 2 }).textContent?.trim()).toBe('Wedding');
			expect(screen.getAllByRole('button', { name: /download all/i })).toHaveLength(2);
		});

		it('shows the All / Favorites switch only once the hero is out of view', async () => {
			show();
			await tick();

			expect(screen.queryByRole('button', { name: /^Favorites/ })).toBeNull();

			report(false);
			await tick();

			expect(favoritesToggle()).toBeTruthy();
		});
	});

	it('keeps the bar available when there is no IntersectionObserver', async () => {
		vi.stubGlobal('IntersectionObserver', undefined);
		show();
		await tick();

		expect(favoritesToggle()).toBeTruthy();
	});

	it('filters to favorites and says when there are none', async () => {
		const user = userEvent.setup();
		show({ favorites: ['p2'] });

		await user.click(screen.getByRole('button', { name: /^Favorites/ }));

		expect(hearts()).toHaveLength(1);

		await user.click(screen.getByRole('button', { name: 'All' }));

		expect(hearts()).toHaveLength(3);

		await user.click(screen.getByRole('button', { name: /^Favorites/ }));
		await user.click(hearts()[0]);

		expect(screen.getByText('No favorites yet')).toBeTruthy();
	});

	it('keeps the viewer on its photo when unfavoriting under the filter', async () => {
		const user = userEvent.setup();
		show({ favorites: ['p3'] });

		await user.click(screen.getByRole('button', { name: /^Favorites/ }));
		await user.click(screen.getByRole('button', { name: 'Open proof 1 of 1' }));
		await user.click(screen.getByRole('button', { name: 'Favorite' }));

		expect(screen.getByText('1 of 1')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Add to favorites' })).toBeTruthy();
	});

	it('signs the cover with the studio and leaves the footer to the app layout', () => {
		show({ studio: 'Studio Lune' });

		expect(screen.getAllByText('Studio Lune')).toHaveLength(1);
		expect(screen.queryByRole('contentinfo')).toBeNull();
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

	it('uses the clean cover separately from the client proofs', () => {
		show({
			gallery: shared('b'),
			proofs: [proof('a'), proof('b')],
			coverUrl: '/clean-cover/b'
		});

		expect(screen.getByRole('img', { name: 'Gallery cover' }).getAttribute('src')).toBe(
			'/clean-cover/b'
		);
	});

	it('shows no image without an authorized clean cover', () => {
		const { unmount } = show({ gallery: shared(null), proofs: [proof('a')] });

		expect(screen.queryByRole('img', { name: 'Gallery cover' })).toBeNull();
		expect(screen.getByTestId('no-hero')).toBeTruthy();

		unmount();
		show({ gallery: shared('gone'), proofs: [proof('a')] });

		expect(screen.queryByRole('img', { name: 'Gallery cover' })).toBeNull();
	});

	it('keeps the clean hero when watermarked proofs are unavailable', () => {
		show({ gallery: shared('b'), proofs: [], coverUrl: '/clean-cover/b' });

		expect(screen.getByRole('img', { name: 'Gallery cover' }).getAttribute('src')).toBe(
			'/clean-cover/b'
		);
	});
});

describe('ClientGallery in French', () => {
	useLocale('fr');

	it('shows the proofing gallery in French', async () => {
		show({ gallery: { ...gallery, included_photos: 2 } });

		expect(screen.getByRole('button', { name: 'Voir la galerie' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Favoris 0 sur 2' })).toBeTruthy();
		expect(screen.getByText('sur 2')).toBeTruthy();
		expect(screen.getAllByRole('button', { name: /^Mettre l’épreuve 1 en favori/ })).toBeTruthy();
		expect(screen.queryByText('View the gallery')).toBeNull();

		await userEvent
			.setup()
			.click(screen.getByRole('button', { name: /^Ouvrir l’épreuve 1 sur 3/ }));

		expect(screen.getByRole('button', { name: 'Fermer la visionneuse' })).toBeTruthy();
		expect(screen.getByText('1 sur 3')).toBeTruthy();
	});

	it('shows the delivered page in French', () => {
		show({ gallery: { ...gallery, delivered: true } });

		expect(screen.getByText('Vos photos sont prêtes')).toBeTruthy();
		expect(screen.getAllByRole('button', { name: /Tout télécharger/ }).length).toBeGreaterThan(0);
		expect(screen.queryByText('Your photos are ready')).toBeNull();
	});
});
