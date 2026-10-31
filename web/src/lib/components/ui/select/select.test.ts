import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import Select from './select.svelte';
import SelectHarness from './select-harness.test.svelte';

const children = createRawSnippet(() => ({
	render: () => '<option value="a">Alpha</option>'
}));

describe('Select', () => {
	it('renders options, passes aria through and is accessible', async () => {
		const { container } = render(Select, {
			children,
			'aria-label': 'Sort',
			'aria-invalid': true,
			class: 'text-lg'
		});
		const select = screen.getByLabelText('Sort');

		expect(screen.getAllByRole('option')).toHaveLength(1);
		expect(select.getAttribute('aria-invalid')).toBe('true');
		expect(select.className).toContain('rounded-full');
		expect(select.className).toContain('text-lg');
		expect(select.className).not.toContain('text-sm');
		expect((await axe(container)).violations).toEqual([]);
	});

	it('updates bind:value on change', async () => {
		render(SelectHarness);

		await userEvent.selectOptions(screen.getByLabelText('Pick'), 'b');

		expect(screen.getByTestId('value').textContent).toBe('b');
	});
});
