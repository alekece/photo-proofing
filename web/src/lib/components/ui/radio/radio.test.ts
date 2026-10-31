import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import Radio from './radio.svelte';

describe('Radio', () => {
	it('is a native radio with name and value, passes aria through and is accessible', async () => {
		const { container } = render(Radio, {
			name: 'layout',
			value: 'grid',
			'aria-label': 'Grid',
			'aria-describedby': 'hint'
		});
		const radio = screen.getByRole('radio', { name: 'Grid' }) as HTMLInputElement;

		expect(radio.tagName).toBe('INPUT');
		expect(radio.name).toBe('layout');
		expect(radio.value).toBe('grid');
		expect(radio.getAttribute('aria-describedby')).toBe('hint');
		expect((await axe(container)).violations).toEqual([]);
	});

	it('reflects the checked prop', () => {
		render(Radio, { 'aria-label': 'On', checked: true });
		render(Radio, { 'aria-label': 'Off', checked: false });

		expect((screen.getByRole('radio', { name: 'On' }) as HTMLInputElement).checked).toBe(true);
		expect((screen.getByRole('radio', { name: 'Off' }) as HTMLInputElement).checked).toBe(false);
	});

	it('fires onchange when clicked', async () => {
		const onchange = vi.fn();
		render(Radio, { 'aria-label': 'Pick', onchange });

		await userEvent.click(screen.getByRole('radio', { name: 'Pick' }));

		expect(onchange).toHaveBeenCalledTimes(1);
	});

	it('does not change when disabled', async () => {
		const onchange = vi.fn();
		render(Radio, { 'aria-label': 'Nope', disabled: true, onchange });
		const radio = screen.getByRole('radio', { name: 'Nope' }) as HTMLInputElement;

		await userEvent.click(radio);

		expect(radio.checked).toBe(false);
		expect(onchange).not.toHaveBeenCalled();
	});
});
