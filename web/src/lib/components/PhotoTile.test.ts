import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import PhotoTile from './PhotoTile.svelte';

const base = { name: 'a.jpg', state: 'ready', thumbnailUrl: '/t', onremove: () => {} } as const;

describe('PhotoTile hero star', () => {
	it('is a toggle named after what a click does', async () => {
		const onhero = vi.fn();
		const user = userEvent.setup();
		const view = render(PhotoTile, { ...base, onhero });
		const star = screen.getByRole('button', { name: 'Set as hero photo' });

		expect(star.getAttribute('aria-pressed')).toBe('false');

		await user.click(star);

		expect(onhero).toHaveBeenCalledTimes(1);

		await view.rerender({ ...base, onhero, hero: true });

		expect(
			screen.getByRole('button', { name: 'Remove hero photo' }).getAttribute('aria-pressed')
		).toBe('true');
	});

	it('is reachable by keyboard', async () => {
		const onhero = vi.fn();
		const user = userEvent.setup();

		render(PhotoTile, { ...base, onhero });
		screen.getByRole('button', { name: 'Open a.jpg' }).focus();
		await user.tab();

		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Set as hero photo' }));

		await user.keyboard('{Enter}');

		expect(onhero).toHaveBeenCalledTimes(1);
	});

	it('is absent without a handler and on photos that are not ready', () => {
		render(PhotoTile, base);
		render(PhotoTile, { ...base, state: 'processing', onhero: () => {} });

		expect(screen.queryByRole('button', { name: /hero photo/ })).toBeNull();
	});
});
