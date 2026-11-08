import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import SelectActions from './SelectActions.svelte';

function setup(count = 3) {
	const handlers = { onunselectall: vi.fn(), onleave: vi.fn() };
	const view = render(SelectActions, { count, ...handlers });

	return { ...view, ...handlers };
}

describe('SelectActions', () => {
	it('shows the count and unselects all', async () => {
		const { container, onunselectall, onleave } = setup();

		expect(screen.getByText('3 selected')).toBeTruthy();

		await userEvent.click(screen.getByRole('button', { name: 'Unselect all' }));

		expect(onunselectall).toHaveBeenCalledOnce();
		expect(onleave).not.toHaveBeenCalled();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('leaves select mode from Cancel when nothing is selected', async () => {
		const { onunselectall, onleave } = setup(0);

		await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));

		expect(onleave).toHaveBeenCalledOnce();
		expect(onunselectall).not.toHaveBeenCalled();
	});
});
