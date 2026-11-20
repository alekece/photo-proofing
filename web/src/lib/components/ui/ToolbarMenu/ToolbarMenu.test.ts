import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import Harness from './ToolbarMenuHarness.test.svelte';

it('chooses a sort from the keyboard, marks it checked and returns focus', async () => {
	const user = userEvent.setup();
	const onchange = vi.fn();
	render(Harness, { onchange });
	const trigger = screen.getByRole('button', { name: /sort by.*newest/i });

	trigger.focus();
	await user.keyboard('{Enter}{ArrowDown}{Enter}');

	expect(onchange).toHaveBeenCalledExactlyOnceWith('oldest');
	expect(document.activeElement).toBe(trigger);
	expect(screen.queryByRole('menu')).toBeNull();
	await user.keyboard('{Enter}');
	expect(screen.getByRole('menuitemradio', { name: 'Oldest', checked: true })).toBeTruthy();
});

it('dismisses the sort options when tabbing backwards to the trigger', async () => {
	const user = userEvent.setup();
	render(Harness, { onchange: vi.fn() });
	const trigger = screen.getByRole('button', { name: /sort by.*newest/i });

	trigger.focus();
	await user.keyboard('{Enter}');
	await user.tab({ shift: true });

	expect(document.activeElement).toBe(trigger);
	expect(screen.queryByRole('menu')).toBeNull();
});

it('preserves the trigger layout when a count appears, dismisses on Escape and respects disabled', async () => {
	const user = userEvent.setup();
	const onchange = vi.fn();
	const { rerender } = render(Harness, { onchange });
	const trigger = screen.getByRole('button', { name: /sort by.*newest/i });
	const layout = trigger.parentElement?.className;

	await rerender({ activeCount: 2 });
	expect(trigger.textContent).toContain('2');
	expect(trigger.parentElement?.className).toBe(layout);

	await user.click(trigger);
	await user.keyboard('{Escape}');
	expect(screen.queryByRole('menu')).toBeNull();
	expect(document.activeElement).toBe(trigger);
	expect(onchange).not.toHaveBeenCalled();

	await rerender({ disabled: true });
	await user.click(trigger);
	expect(screen.queryByRole('menu')).toBeNull();
});
