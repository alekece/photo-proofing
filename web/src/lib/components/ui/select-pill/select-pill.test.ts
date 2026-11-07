import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import SelectPill from './select-pill.svelte';

function setup(props: Partial<Parameters<typeof render<typeof SelectPill>>[1]> = {}) {
	const handlers = { onaction: vi.fn(), onselectall: vi.fn(), onclose: vi.fn() };
	const view = render(SelectPill, { count: 3, actionLabel: 'Delete', ...handlers, ...props });

	return { ...view, ...handlers };
}

describe('SelectPill', () => {
	it('keeps selection controls in the warm toolbar instead of a dark replacement', () => {
		const { container } = setup();
		expect(container.querySelector('[data-slot="select-pill"]')?.className).not.toContain(
			'bg-foreground'
		);
	});

	it('shows the count and runs its three controls', async () => {
		const { container, onaction, onselectall, onclose } = setup();

		expect(screen.getByText('3 selected')).toBeTruthy();

		await userEvent.click(screen.getByRole('button', { name: 'Select all' }));
		await userEvent.click(screen.getByRole('button', { name: 'Delete' }));
		await userEvent.click(screen.getByRole('button', { name: 'Cancel selection' }));

		expect(onselectall).toHaveBeenCalledOnce();
		expect(onaction).toHaveBeenCalledOnce();
		expect(onclose).toHaveBeenCalledOnce();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('disables the action when asked or while busy', () => {
		setup({ actionDisabled: true });

		expect(screen.getByRole('button', { name: 'Delete' })).toHaveProperty('disabled', true);
	});

	it('marks a busy action', () => {
		setup({ actionBusy: true });

		const action = screen.getByRole('button', { name: 'Delete' });

		expect(action).toHaveProperty('disabled', true);
		expect(action.getAttribute('aria-busy')).toBe('true');
	});
});
