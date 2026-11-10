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

const button = (name: string) => screen.getByRole('button', { name }) as HTMLButtonElement;
const checkbox = () => screen.getByRole('checkbox', { name: 'Select all' }) as HTMLInputElement;

describe('SelectActions', () => {
	it('shows the count and exits from the icon button', async () => {
		const { container, onexit } = setup(3);

		expect(screen.getByText('3 selected')).toBeTruthy();

		await userEvent.click(button('Exit selection'));

		expect(onexit).toHaveBeenCalledOnce();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('toggles everything from one tri-state checkbox, not buttons', async () => {
		const { onselectall, onunselectall, rerender } = setup(1);

		expect(screen.queryByRole('button', { name: 'Select all' })).toBeNull();

		await userEvent.click(checkbox());
		expect(onselectall).toHaveBeenCalledOnce();

		await rerender({ count: 3, total: 3 });
		await userEvent.click(checkbox());
		expect(onunselectall).toHaveBeenCalledOnce();
	});

	it.each([
		[0, false, false],
		[1, false, true],
		[3, true, false]
	])('with %i of 3 selected, checked is %s and indeterminate %s', (count, checked, mixed) => {
		setup(count);

		expect(checkbox().checked).toBe(checked);
		expect(checkbox().indeterminate).toBe(mixed);
	});
});
