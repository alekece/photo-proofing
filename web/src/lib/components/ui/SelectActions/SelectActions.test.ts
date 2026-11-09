import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import SelectActions from './SelectActions.svelte';

function setup(count = 1, total = 3) {
	const handlers = { onselectall: vi.fn(), onunselectall: vi.fn(), onexit: vi.fn() };
	const view = render(SelectActions, { count, total, ...handlers });

	return { ...view, ...handlers };
}

const selectAll = () => screen.getByRole('checkbox', { name: 'Select all' }) as HTMLInputElement;

describe('SelectActions', () => {
	it('shows the count and exits from the icon button', async () => {
		const { container, onexit } = setup(3);

		expect(screen.getByText('3 selected')).toBeTruthy();

		await userEvent.click(screen.getByRole('button', { name: 'Exit selection' }));

		expect(onexit).toHaveBeenCalledOnce();
		expect((await axe(container)).violations).toEqual([]);
	});

	it.each([
		[0, false, false],
		[1, false, true],
		[3, true, false]
	])('with %i of 3 selected, checked is %s and indeterminate is %s', (count, checked, mixed) => {
		setup(count);

		expect(selectAll().checked).toBe(checked);
		expect(selectAll().indeterminate).toBe(mixed);
	});

	it('selects all unless everything is selected, then unselects all', async () => {
		const first = setup(1);

		await userEvent.click(selectAll());

		expect(first.onselectall).toHaveBeenCalledOnce();
		expect(first.onunselectall).not.toHaveBeenCalled();

		first.unmount();

		const second = setup(3);

		await userEvent.click(selectAll());

		expect(second.onunselectall).toHaveBeenCalledOnce();
		expect(second.onselectall).not.toHaveBeenCalled();
	});
});
