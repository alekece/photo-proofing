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

class FakeEventSource extends EventTarget {
	static last: FakeEventSource | undefined;
	closed = false;

	constructor(readonly url: string) {
		super();
		FakeEventSource.last = this;
	}

	close() {
		this.closed = true;
	}
}

const show = (props: Partial<Parameters<typeof render<typeof ClientGallery>>[1]> = {}) =>
	render(ClientGallery, { token: 'tok', gallery, proofs, favorites: [], ...props });
const hearts = () => screen.getAllByRole('button', { name: /^favorite proof/i });

describe('ClientGallery', () => {
	const fetcher = vi.fn();

	beforeEach(() => {
		FakeEventSource.last = undefined;
		fetcher.mockReset();
		fetcher.mockResolvedValue(new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetcher);
		vi.stubGlobal('EventSource', FakeEventSource);
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
		expect(screen.getByText('1 selected')).toBeTruthy();
	});

	it('selects a photo at once and saves it', async () => {
		const user = userEvent.setup();
		show();

		await user.click(hearts()[0]);

		expect(hearts()[0].getAttribute('aria-pressed')).toBe('true');
		expect(screen.getByText('1 selected')).toBeTruthy();
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

		const summary = screen.getByText(/3 selected/);

		expect(summary.textContent).toBe('3 selected · 1 included · 2 extra');
		expect(summary.querySelector('.font-bold')?.textContent).toBe('2 extra');
	});

	it('shows no extras within the allowance', () => {
		show({ gallery: { ...gallery, included_photos: 5 }, favorites: ['p1'] });

		expect(screen.getByText(/1 selected/).textContent).toBe('1 selected · 5 included');
	});

	it('is read-only once delivered', () => {
		show({ gallery: { ...gallery, delivered: true }, favorites: ['p1'] });

		expect(screen.queryAllByRole('button', { name: /^favorite proof/i })).toEqual([]);
		expect(screen.getByText('1 selected')).toBeTruthy();
		expect(FakeEventSource.last).toBeUndefined();
	});

	it('refreshes from the link stream and closes it on leaving', async () => {
		fetcher.mockResolvedValue(new Response(JSON.stringify(['p3']), { status: 200 }));
		const { unmount } = show();
		const source = FakeEventSource.last!;

		expect(source.url).toBe('/api/v1/galleries/shared/tok/events');

		source.dispatchEvent(new Event('favorites'));

		await waitFor(() => expect(hearts()[2].getAttribute('aria-pressed')).toBe('true'));

		unmount();

		expect(source.closed).toBe(true);
	});

	it('refetches after the stream resyncs', async () => {
		fetcher.mockResolvedValue(new Response(JSON.stringify(['p1']), { status: 200 }));
		show();

		FakeEventSource.last!.dispatchEvent(new Event('resync'));

		await waitFor(() => expect(screen.getByText('1 selected')).toBeTruthy());
	});

	it('shows the selection but cannot change it in preview mode', () => {
		show({ preview: true, favorites: ['p1'] });

		expect(screen.getByText('1 selected')).toBeTruthy();
		const hearts = screen.getAllByRole('button', { name: /^favorite proof/i });
		expect(hearts).toHaveLength(3);
		for (const heart of hearts) expect((heart as HTMLButtonElement).disabled).toBe(true);
		expect(
			screen.getByRole('button', { name: 'Favorite proof 1' }).getAttribute('aria-pressed')
		).toBe('true');
		expect(FakeEventSource.last).toBeUndefined();
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

		unmount();
		show({ gallery: shared('gone'), proofs: [proof('a')] });

		expect(screen.queryByRole('img', { name: 'Gallery cover' })).toBeNull();
	});
});
