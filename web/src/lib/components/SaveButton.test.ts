import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import SaveButton from './SaveButton.svelte';

describe('SaveButton', () => {
	it('offers Save and calls back when clicked', async () => {
		const onclick = vi.fn();

		render(SaveButton, { status: 'idle', onclick });
		await userEvent.setup().click(screen.getByRole('button', { name: 'Save' }));

		expect(onclick).toHaveBeenCalledOnce();
		expect(screen.queryByTestId('click-ripple')).toBeNull();
		expect(screen.getByRole('status').textContent?.trim()).toBe('');
	});

	it('names the button Saving… and announces it', () => {
		render(SaveButton, { status: 'saving', onclick: vi.fn() });

		expect(screen.getByRole('button', { name: 'Saving…' })).toBeTruthy();
		expect(screen.getByRole('status').textContent?.trim()).toBe('Saving…');
	});

	it('keeps its look but ignores clicks while saving', async () => {
		const onclick = vi.fn();

		render(SaveButton, { status: 'saving', onclick });
		const button = screen.getByRole('button');

		await userEvent.setup().click(button);

		expect(button).toHaveProperty('disabled', false);
		expect(button.getAttribute('aria-disabled')).toBe('true');
		expect(onclick).not.toHaveBeenCalled();
	});

	it('fades its labels in place instead of sliding them', () => {
		const { container } = render(SaveButton, { status: 'saving', onclick: vi.fn() });

		expect(container.innerHTML).not.toMatch(/translate-y/);
	});

	it('has no axe violations while saving', async () => {
		const { container } = render(SaveButton, { status: 'saving', onclick: vi.fn() });

		expect((await axe(container)).violations).toEqual([]);
	});
});
