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

	it('offers Select all and Unselect all as buttons, not a checkbox', async () => {
		const { onselectall, onunselectall } = setup(1);

		expect(screen.queryByRole('checkbox')).toBeNull();

		await userEvent.click(button('Select all'));
		await userEvent.click(button('Unselect all'));

		expect(onselectall).toHaveBeenCalledOnce();
		expect(onunselectall).toHaveBeenCalledOnce();
	});

	it('puts both buttons in the right-hand action group', () => {
		const { container } = setup(1);
		const group = container.querySelector('.ml-auto')!;

		expect(group.contains(button('Select all'))).toBe(true);
		expect(group.contains(button('Unselect all'))).toBe(true);
	});

	it.each([
		[0, false, true],
		[1, false, false],
		[3, true, false]
	])('with %i of 3 selected, Select all disabled is %s and Unselect all %s', (count, all, none) => {
		setup(count);

		expect(button('Select all').disabled).toBe(all);
		expect(button('Unselect all').disabled).toBe(none);
	});
});
