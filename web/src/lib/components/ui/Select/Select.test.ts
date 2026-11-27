import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import Select from './Select.svelte';
import SelectHarness from './SelectHarness.test.svelte';

describe('Select', () => {
	it('exposes the selected option and its accessible description', async () => {
		const { container } = render(Select, {
			label: 'Sort',
			value: 'a',
			options: [{ value: 'a', label: 'Alpha' }],
			'aria-describedby': 'sort-hint'
		});
		const hint = document.createElement('p');
		hint.id = 'sort-hint';
		hint.textContent = 'Choose a sort order.';
		container.append(hint);
		const trigger = screen.getByRole('button', { name: 'Sort: Alpha' });

		expect(trigger.getAttribute('aria-describedby')).toBe(hint.id);
		await userEvent.click(trigger);
		const selected = screen.getByRole('menuitemradio', { name: 'Alpha', checked: true });

		expect(document.activeElement).toBe(selected);
		expect((await axe(container)).violations).toEqual([]);
	});

	it('updates bind:value on change', async () => {
		render(SelectHarness);

		await userEvent.click(screen.getByRole('button', { name: 'Pick: Alpha' }));
		await userEvent.keyboard('{ArrowDown}{Enter}');

		expect(screen.getByTestId('value').textContent).toBe('b');
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Pick: Beta' }));
	});
});
