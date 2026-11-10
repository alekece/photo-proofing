import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { useLocale } from '#lib/i18n/testing.js';
import DatePicker from './DatePicker.svelte';

function day(label: string) {
	return within(screen.getByRole('grid')).getByText(label, {
		selector: '[data-bits-day]:not([data-outside-month])'
	});
}

describe('DatePicker', () => {
	useLocale('en');

	it('shows the placeholder when empty and the day when set', async () => {
		const { rerender } = render(DatePicker, {
			id: 'when',
			value: null,
			placeholder: 'Optional',
			onchange: vi.fn()
		});

		expect(screen.getByRole('button').textContent).toContain('Optional');

		await rerender({ value: '2026-06-14' });

		expect(screen.getByRole('button').textContent).toContain('June 14, 2026');
	});

	it('emits the picked day as YYYY-MM-DD and closes', async () => {
		const user = userEvent.setup();
		const onchange = vi.fn();
		render(DatePicker, { id: 'when', value: '2026-06-01', onchange });

		await user.click(screen.getByRole('button'));
		await user.click(day('14'));

		expect(onchange).toHaveBeenCalledWith('2026-06-14');
		expect(screen.queryByRole('grid')).toBeNull();
	});

	it('disables the days before min', async () => {
		const user = userEvent.setup();
		render(DatePicker, {
			id: 'when',
			value: '2026-06-14',
			min: '2026-06-10',
			onchange: vi.fn()
		});

		await user.click(screen.getByRole('button'));

		expect(day('9').hasAttribute('data-disabled')).toBe(true);
		expect(day('10').hasAttribute('data-disabled')).toBe(false);
	});

	it('clears to null when clearable', async () => {
		const user = userEvent.setup();
		const onchange = vi.fn();
		render(DatePicker, { id: 'when', value: '2026-06-14', clearable: true, onchange });

		await user.click(screen.getByRole('button', { name: /june 14/i }));
		await user.click(screen.getByRole('button', { name: 'Clear' }));

		expect(onchange).toHaveBeenCalledWith(null);
	});

	it('has no axe violations when open', async () => {
		const user = userEvent.setup();
		const { container } = render(DatePicker, {
			id: 'when',
			value: '2026-06-14',
			onchange: vi.fn()
		});

		await user.click(screen.getByRole('button'));

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('DatePicker in French', () => {
	useLocale('fr');

	it('starts weeks on Monday and names months in French', async () => {
		const user = userEvent.setup();
		render(DatePicker, { id: 'when', value: '2026-06-14', onchange: vi.fn() });

		await user.click(screen.getByRole('button'));

		const headers = within(screen.getByRole('grid')).getAllByRole('columnheader');

		expect(headers[0].textContent?.trim().toLowerCase()).toMatch(/^lu/);
		expect(screen.getByText('juin 2026')).toBeTruthy();
	});
});
