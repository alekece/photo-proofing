import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';
import LanguagePicker from './LanguagePicker.svelte';

describe('LanguagePicker', () => {
	it('shows both language choices without opening a menu', () => {
		render(LanguagePicker, { value: 'en' });

		expect(screen.getByRole('radio', { name: 'English', checked: true })).toBeTruthy();
		expect(
			screen.getByRole('radio', { name: 'Français', checked: false }).closest('[lang]')
		).toHaveProperty('lang', 'fr');
	});

	it('shows the changed draft choice', async () => {
		render(LanguagePicker, { value: 'en' });
		await userEvent.setup().click(screen.getByRole('radio', { name: 'Français' }));
		expect(screen.getByRole('radio', { name: 'Français', checked: true })).toBeTruthy();
	});

	it('has no axe violations', async () => {
		const { container } = render(LanguagePicker, { value: 'en' });

		expect((await axe(container)).violations).toEqual([]);
	});
});
