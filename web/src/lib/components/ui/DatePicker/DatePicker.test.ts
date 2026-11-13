import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { useLocale } from '#lib/i18n/testing.js';
import DatePicker from './DatePicker.svelte';

afterEach(() => {
	vi.useRealTimers();
});

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

		expect(screen.getByRole('button').textContent).toContain('Jun 14, 2026');
	});

	it('keeps the date and closes when the picked day is clicked again', async () => {
		const user = userEvent.setup();
		const onchange = vi.fn();
		render(DatePicker, { id: 'when', value: '2026-06-14', onchange });

		await user.click(screen.getByRole('button'));
		await user.click(day('14'));

		expect(onchange).not.toHaveBeenCalled();
		expect(screen.queryByRole('grid')).toBeNull();
		expect(screen.getByRole('button').textContent).toContain('Jun 14, 2026');
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

		await user.click(screen.getByRole('button', { name: /jun 14/i }));
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

	it('omits preset buttons when presets is not set', async () => {
		const user = userEvent.setup();
		render(DatePicker, { id: 'when', value: '2026-06-14', onchange: vi.fn() });

		await user.click(screen.getByRole('button'));

		expect(screen.queryByRole('button', { name: 'In 1 week' })).toBeNull();
	});

	it('computes all seven relative presets from today (including month-end rollover) and closes on click', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		vi.setSystemTime(new Date(2026, 0, 31, 12, 0, 0));

		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const onchange = vi.fn();
		const { container } = render(DatePicker, {
			id: 'when',
			value: null,
			placeholder: '–',
			presets: true,
			clearable: true,
			onchange
		});

		const cases: [string, string][] = [
			['In 1 week', '2026-02-07'],
			['In 2 weeks', '2026-02-14'],
			['In 1 month', '2026-02-28'],
			['In 2 months', '2026-03-31'],
			['In 4 months', '2026-05-31'],
			['In 6 months', '2026-07-31'],
			['In 1 year', '2027-01-31']
		];

		for (const [label, expected] of cases) {
			await user.click(document.getElementById('when')!);

			if (label === 'In 1 week') {
				expect((await axe(container)).violations).toEqual([]);
			}

			await user.click(screen.getByRole('button', { name: label }));

			expect(onchange).toHaveBeenLastCalledWith(expected);
			expect(screen.queryByRole('grid')).toBeNull();
		}
	});

	it('closes without firing onchange when clicking a preset matching the current value', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		vi.setSystemTime(new Date(2026, 0, 31, 12, 0, 0));

		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const onchange = vi.fn();
		render(DatePicker, {
			id: 'when',
			value: '2026-02-07',
			presets: true,
			clearable: true,
			onchange
		});

		await user.click(document.getElementById('when')!);
		await user.click(screen.getByRole('button', { name: 'In 1 week' }));

		expect(onchange).not.toHaveBeenCalled();
		expect(screen.queryByRole('grid')).toBeNull();

		await user.click(document.getElementById('when')!);
		await user.click(screen.getByRole('button', { name: 'Clear' }));

		expect(onchange).toHaveBeenCalledWith(null);
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

describe('DatePicker in French', () => {
	useLocale('fr');

	it('names the month buttons and presets in the current language', async () => {
		const { container } = render(DatePicker, {
			id: 'when',
			value: '2026-06-14',
			presets: true,
			onchange: vi.fn()
		});

		await userEvent.setup().click(screen.getByRole('button'));

		expect(screen.getByRole('button', { name: 'Mois suivant' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Mois précédent' })).toBeTruthy();
		for (const label of [
			'Dans 1 semaine',
			'Dans 2 semaines',
			'Dans 1 mois',
			'Dans 2 mois',
			'Dans 4 mois',
			'Dans 6 mois',
			'Dans 1 an'
		]) {
			expect(screen.getByRole('button', { name: label })).toBeTruthy();
		}
		expect((await axe(container)).violations).toEqual([]);
	});
});
