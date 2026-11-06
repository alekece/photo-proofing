import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';

import GalleryStateFilter from './GalleryStateFilter.svelte';
import { ALL_STATES, DEFAULT_STATES } from '#lib/gallery/states.js';

const counts = { draft: 2, shared: 3, delivered: 1, expired: 0, archived: 4 };
const setup = (selected = DEFAULT_STATES, onchange = vi.fn(), disabled = false) => ({
	onchange,
	...render(GalleryStateFilter, { selected, counts, onchange, disabled })
});

describe('GalleryStateFilter', () => {
	it('reads Status by default and the chosen names otherwise', () => {
		const { unmount } = setup();

		expect(screen.getByRole('button', { name: 'Status' })).toBeTruthy();
		unmount();
		setup(['shared', 'expired']);
		expect(screen.getByRole('button', { name: 'Shared, Expired' })).toBeTruthy();
	});

	it('lists every status with its count and toggles one', async () => {
		const user = userEvent.setup();
		const { onchange } = setup();

		await user.click(screen.getByRole('button', { name: 'Status' }));
		expect(screen.getByRole('checkbox', { name: /Archived/ })).toHaveProperty('checked', false);
		expect(screen.getByRole('checkbox', { name: /Shared/ })).toHaveProperty('checked', true);
		expect(screen.getByText('4')).toBeTruthy();
		await user.click(screen.getByRole('checkbox', { name: /Archived/ }));
		expect(onchange).toHaveBeenLastCalledWith(ALL_STATES);
		await user.click(screen.getByRole('checkbox', { name: /Draft/ }));
		expect(onchange).toHaveBeenLastCalledWith(['shared', 'delivered', 'expired']);
	});

	it('selects all and resets to the default', async () => {
		const user = userEvent.setup();
		const { onchange } = setup(['shared']);

		await user.click(screen.getByRole('button', { name: 'Shared' }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));
		expect(onchange).toHaveBeenLastCalledWith(ALL_STATES);
		await user.click(screen.getByRole('button', { name: 'Reset' }));
		expect(onchange).toHaveBeenLastCalledWith(DEFAULT_STATES);
	});

	it('is disabled for an empty workspace and has no axe violations', async () => {
		const { container } = setup(DEFAULT_STATES, vi.fn(), true);

		expect((screen.getByRole('button', { name: 'Status' }) as HTMLButtonElement).disabled).toBe(
			true
		);
		expect((await axe(container)).violations).toEqual([]);
	});
});
