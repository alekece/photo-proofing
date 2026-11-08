import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import Checkbox from './Checkbox.svelte';
import CheckboxHarness from './CheckboxHarness.test.svelte';

describe('Checkbox', () => {
	it('is a native checkbox, passes aria through and is accessible', async () => {
		const { container } = render(Checkbox, { 'aria-label': 'Agree', 'aria-describedby': 'hint' });
		const box = screen.getByRole('checkbox', { name: 'Agree' });

		expect(box.tagName).toBe('INPUT');
		expect(box.getAttribute('aria-describedby')).toBe('hint');
		expect((await axe(container)).violations).toEqual([]);
	});

	it('toggles on click and updates bind:checked', async () => {
		render(CheckboxHarness);
		const box = screen.getByRole('checkbox', { name: 'Mark' });

		await userEvent.click(box);

		expect(screen.getByTestId('checked').textContent).toBe('true');

		await userEvent.click(box);

		expect(screen.getByTestId('checked').textContent).toBe('false');
	});

	it('does not toggle when disabled', async () => {
		render(Checkbox, { 'aria-label': 'Off', disabled: true });
		const box = screen.getByRole('checkbox', { name: 'Off' }) as HTMLInputElement;

		await userEvent.click(box);

		expect(box.checked).toBe(false);
	});
});
