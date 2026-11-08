import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import PopoverHarness from './PopoverHarness.test.svelte';

describe('Popover', () => {
	it('opens from its button and closes on Escape, returning focus to the button', async () => {
		const user = userEvent.setup();
		render(PopoverHarness);
		const button = screen.getByRole('button', { name: 'Open' });

		expect(button.getAttribute('aria-expanded')).toBe('false');
		expect(screen.queryByText('Inside')).toBeNull();
		await user.click(button);
		expect(button.getAttribute('aria-expanded')).toBe('true');
		expect(screen.getByText('Inside')).toBeTruthy();
		await user.keyboard('{Escape}');
		expect(screen.queryByText('Inside')).toBeNull();
		expect(document.activeElement).toBe(button);
	});

	it('closes when pressing outside but not inside', async () => {
		const user = userEvent.setup();
		render(PopoverHarness);

		await user.click(screen.getByRole('button', { name: 'Open' }));
		await user.click(screen.getByText('Inside'));
		expect(screen.getByText('Inside')).toBeTruthy();
		await user.click(screen.getByText('Outside'));
		expect(screen.queryByText('Inside')).toBeNull();
	});

	it('closes when keyboard focus leaves it', async () => {
		const user = userEvent.setup();
		render(PopoverHarness);

		await user.click(screen.getByRole('button', { name: 'Open' }));
		await user.tab();
		await user.tab();
		expect(screen.queryByText('Inside')).toBeNull();
	});

	it('does not open while disabled and has no axe violations', async () => {
		const user = userEvent.setup();
		const { container } = render(PopoverHarness, { disabled: true });

		await user.click(screen.getByRole('button', { name: 'Open' }));
		expect(screen.queryByText('Inside')).toBeNull();
		expect((await axe(container)).violations).toEqual([]);
	});
});
