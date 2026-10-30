import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ afterNavigate: vi.fn(), goto: vi.fn() }));
vi.mock('$app/state', () => ({ navigating: { to: null } }));
import { loadClientGallery } from '#lib/gallery/load.js';
import Dashboard from './dashboard/+page.svelte';
import Gallery from './g/[gallery]/+page.svelte';

const open = async (slug: string) =>
	render(Gallery, { props: { data: { view: await loadClientGallery(slug) } } });

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
		expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeTruthy();
	});
});

describe('client gallery route', () => {
	it('shows state messaging, expiry and proof previews only', async () => {
		const { container } = await open('demo-proofing');
		expect(screen.getByRole('heading', { name: 'Sample session' })).toBeTruthy();
		expect(screen.getByRole('status').textContent).toMatch(/favourites/);
		expect(container.textContent).toMatch(/Access until/);
		expect(screen.getAllByRole('img')).toHaveLength(9);
		expect(container.querySelectorAll('a')).toHaveLength(0);
	});

	it.each(['demo-expired', 'demo-draft', 'no-such-gallery'])(
		'renders the same leak-free access state for %s',
		async (slug) => {
			const { container } = await open(slug);
			expect(screen.getByRole('heading', { name: /isn't available/ })).toBeTruthy();
			expect(container.textContent).not.toMatch(/Sample session|Proof/);
			expect(screen.queryAllByRole('img')).toHaveLength(0);
		}
	);

	it('shows no previews in delivery', async () => {
		await open('demo-delivery');
		expect(screen.queryAllByRole('img')).toHaveLength(0);
	});

	it('opens the viewer by keyboard and returns focus to the thumbnail', async () => {
		const user = userEvent.setup();
		await open('demo-proofing');
		await user.tab();
		const thumb = document.activeElement as HTMLElement;
		await user.keyboard('{Enter}');
		expect(screen.getByRole('dialog', { name: /proof viewer/i })).toBeTruthy();
		await user.keyboard('{Escape}');
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(document.activeElement).toBe(thumb);
	});

	it('has no axe violations for the grid and the open viewer', async () => {
		const user = userEvent.setup();
		const { container } = await open('demo-proofing');
		expect((await axe(container)).violations).toEqual([]);
		await user.tab();
		await user.keyboard('{Enter}');
		expect((await axe(document.body)).violations).toEqual([]);
	});
});
