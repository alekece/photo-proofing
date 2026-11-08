import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import Switch from './Switch.svelte';

describe('Switch', () => {
	it('toggles and reports the new value', async () => {
		const onchange = vi.fn();

		render(Switch, { 'aria-label': 'Watermark proofs', onchange });

		const toggle = screen.getByRole('switch', { name: 'Watermark proofs' });

		expect(toggle.getAttribute('aria-checked')).toBe('false');

		await userEvent.setup().click(toggle);

		expect(toggle.getAttribute('aria-checked')).toBe('true');
		expect(onchange).toHaveBeenCalledWith(true);
	});

	it('has no axe violations', async () => {
		const { container } = render(Switch, { 'aria-label': 'Watermark proofs', checked: true });

		expect((await axe(container)).violations).toEqual([]);
	});
});
