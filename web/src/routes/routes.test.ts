import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ afterNavigate: vi.fn(), goto: vi.fn() }));
vi.mock('$app/state', () => ({ navigating: { to: null } }));
import ClientGallery from '#lib/components/ClientGallery.svelte';
import AccessUnavailable from '#lib/components/AccessUnavailable.svelte';
import type { Proof, SharedGallery } from '#lib/gallery/types.js';
import Dashboard from './dashboard/+page.svelte';
import Gallery from './g/[token]/+page.svelte';

const shared = (overrides: Partial<SharedGallery> = {}): SharedGallery => ({
	title: 'Sample session',
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	delivered: false,
	hero_id: null,
	...overrides
});
const proofs: Proof[] = [1, 2, 3].map((n) => ({
	id: `p${n}`,
	alt: `Proof ${n}`,
	previewUrl: `data:,${n}`,
	thumbnailUrl: `data:,t${n}`,
	width: 300,
	height: 200
}));
const open = (gallery: SharedGallery | null, galleryProofs: Proof[] = []) =>
	render(Gallery, {
		props: {
			data: { gallery, proofs: galleryProofs, favorites: [] },
			params: { token: 'tok' }
		}
	});

describe('route shells', () => {
	it('dashboard renders its heading', () => {
		render(Dashboard, {
			props: {
				data: {
					identity: { account_id: 'account', workspace_id: 'workspace' },
					idleTimeoutMs: 2592000000,
					galleries: [],
					sort: 'updated'
				}
			}
		});
		expect(screen.getByText('Nothing in frame yet.')).toBeTruthy();
	});
});

// jsdom has no EventSource; the client page opens one for live selection updates.
beforeEach(() =>
	vi.stubGlobal(
		'EventSource',
		class {
			addEventListener() {}
			close() {}
		}
	)
);
afterEach(() => vi.unstubAllGlobals());

describe('client gallery route', () => {
	it('shows a neutral unavailable page without gallery data', () => {
		const { container } = open(null);
		expect(screen.getByRole('heading', { level: 1, name: 'Focus not locked' })).toBeTruthy();
		expect(container.textContent).toMatch(/contact your photographer/i);
		expect(container.textContent).not.toMatch(/expired|Sample session/i);
		expect(screen.queryAllByRole('img')).toHaveLength(0);
	});

	it('shows the proofs the server loaded', () => {
		open(shared(), proofs);
		expect(screen.getAllByRole('img').map((img) => img.getAttribute('alt'))).toEqual([
			'Proof 1',
			'Proof 2',
			'Proof 3'
		]);
	});

	it('shows the title and the empty state', () => {
		open(shared());
		expect(screen.getByRole('heading', { name: 'Sample session' })).toBeTruthy();
		expect(
			screen.getByText("The photographer hasn't added photos yet. Check back soon.")
		).toBeTruthy();
	});

	it('shows the expiry badge only when an expiry is set', () => {
		const { container, unmount } = open(shared());
		expect(container.textContent).not.toMatch(/Access until/);
		unmount();
		const second = open(shared({ expires_at: '2099-01-01T00:00:00Z' }));
		expect(second.container.textContent).toMatch(/Access until/);
	});

	it('names an untitled gallery', () => {
		open(shared({ title: null }));
		expect(screen.getByRole('heading', { name: 'Untitled gallery' })).toBeTruthy();
	});
});

describe('ClientGallery', () => {
	it('opens the viewer by keyboard and returns focus to the thumbnail', async () => {
		const user = userEvent.setup();
		render(ClientGallery, { token: 'tok', gallery: shared(), proofs });
		const thumb = screen.getAllByRole('button', { name: /^Open proof/ })[0];
		thumb.focus();
		await user.keyboard('{Enter}');
		expect(screen.getByRole('dialog', { name: /proof viewer/i })).toBeTruthy();
		await user.keyboard('{Escape}');
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(document.activeElement).toBe(thumb);
	});

	it('has no axe violations for the grid and the open viewer', async () => {
		const user = userEvent.setup();
		const { container } = render(ClientGallery, { token: 'tok', gallery: shared(), proofs });
		expect((await axe(container)).violations).toEqual([]);
		screen.getAllByRole('button', { name: /^Open proof/ })[0].focus();
		await user.keyboard('{Enter}');
		expect((await axe(document.body)).violations).toEqual([]);
	});
});

describe('AccessUnavailable', () => {
	it('shows the viewfinder page with a heading and the next step', () => {
		render(AccessUnavailable);
		expect(screen.getByRole('heading', { level: 1 })).toBeTruthy();
		expect(screen.getByText(/Please contact your photographer\./)).toBeTruthy();
	});

	it('does not reveal why the link failed', () => {
		const { container } = render(AccessUnavailable);
		const content = `${container.textContent} ${container.innerHTML}`;
		expect(content).not.toMatch(/expired|archived|deleted|shared|token/i);
	});

	it('has no axe violations', async () => {
		const { container } = render(AccessUnavailable);
		expect((await axe(container)).violations).toEqual([]);
	});
});
