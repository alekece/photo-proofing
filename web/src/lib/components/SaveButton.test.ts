import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import SaveButton from './SaveButton.svelte';

describe('SaveButton', () => {
	it('offers Save and calls back when clicked', async () => {
		const onclick = vi.fn();

		render(SaveButton, { status: 'idle', onclick });
		await userEvent.setup().click(screen.getByRole('button', { name: 'Save changes' }));

		expect(onclick).toHaveBeenCalledOnce();
		expect(screen.queryByTestId('click-ripple')).toBeNull();
		expect(screen.getByRole('status').textContent?.trim()).toBe('');
	});

	it('names the button Saving… and announces it', () => {
		render(SaveButton, { status: 'saving', disabled: true, onclick: vi.fn() });

		expect(screen.getByRole('button', { name: 'Saving…' })).toHaveProperty('disabled', true);
		expect(screen.getByRole('status').textContent?.trim()).toBe('Saving…');
	});

	it('confirms with a check and a ripple, and announces Saved', () => {
		const { container } = render(SaveButton, {
			status: 'saved',
			disabled: true,
			onclick: vi.fn()
		});

		expect(screen.getByRole('button', { name: 'Saved' })).toBeTruthy();
		expect(screen.getByTestId('click-ripple')).toBeTruthy();
		expect(container.querySelector('svg.text-ok-on-dark')).not.toBeNull();
		expect(screen.getByRole('status').textContent?.trim()).toBe('Saved');
	});

	it('has no axe violations while saved', async () => {
		const { container } = render(SaveButton, { status: 'saved', onclick: vi.fn() });

		expect((await axe(container)).violations).toEqual([]);
	});
});
