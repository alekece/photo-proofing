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

describe('SelectActions', () => {
	it('shows the count and exits from the icon button', async () => {
		const { container, onexit } = setup(3);

		expect(screen.getByText('3 selected')).toBeTruthy();

		await userEvent.click(button('Exit selection'));

		expect(onexit).toHaveBeenCalledOnce();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('selects everything, or clears once everything is selected', async () => {
		const { onselectall, onunselectall, rerender } = setup(1);

		await userEvent.click(button('Select all'));
		expect(onselectall).toHaveBeenCalledOnce();

		await rerender({ count: 3, total: 3 });
		expect(screen.queryByRole('button', { name: 'Select all' })).toBeNull();

		await userEvent.click(button('Unselect all'));
		expect(onunselectall).toHaveBeenCalledOnce();
	});

	it('cannot select anything when there is nothing to select', () => {
		setup(0, 0);

		expect(button('Select all').disabled).toBe(true);
	});
});
