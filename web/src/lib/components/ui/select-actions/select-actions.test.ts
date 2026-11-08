import { createRawSnippet } from 'svelte';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import SelectActions from './select-actions.svelte';

function setup(count = 3) {
	const handlers = { onselectall: vi.fn(), onunselectall: vi.fn() };
	const children = createRawSnippet(() => ({ render: () => '<button>Delete</button>' }));
	const view = render(SelectActions, { count, children, ...handlers });

	return { ...view, ...handlers };
}

describe('SelectActions', () => {
	it('shows the count and runs both bulk actions', async () => {
		const { container, onselectall, onunselectall } = setup();

		expect(screen.getByText('3 selected')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Delete' })).toBeTruthy();

		await userEvent.click(screen.getByRole('button', { name: 'Select all' }));
		await userEvent.click(screen.getByRole('button', { name: 'Unselect all' }));

		expect(onselectall).toHaveBeenCalledOnce();
		expect(onunselectall).toHaveBeenCalledOnce();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('disables Unselect all when nothing is selected', () => {
		setup(0);

		expect(screen.getByRole('button', { name: 'Unselect all' })).toHaveProperty('disabled', true);
		expect(screen.getByRole('button', { name: 'Select all' })).toHaveProperty('disabled', false);
	});
});
