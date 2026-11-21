import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';

const mocks = vi.hoisted(() => ({ saveLanguage: vi.fn(), setLocale: vi.fn() }));
vi.mock('#lib/i18n/language.js', () => ({ saveLanguage: mocks.saveLanguage }));
vi.mock('#lib/paraglide/runtime.js', async (original) => {
	const runtime = await original<typeof import('#lib/paraglide/runtime.js')>();

	// `getLocale` is read through the module so `overwriteGetLocale` still takes effect.
	return { ...runtime, getLocale: () => runtime.getLocale(), setLocale: mocks.setLocale };
});
import LanguagePicker from './LanguagePicker.svelte';
import { toasts } from './ui/Toast/toasts.svelte.js';

describe('LanguagePicker', () => {
	useLocale('en');
	afterEach(() => {
		vi.clearAllMocks();
		toasts.items = [];
	});

	it('shows both language choices without opening a menu', () => {
		render(LanguagePicker);

		expect(screen.getByRole('radio', { name: 'English', checked: true })).toBeTruthy();
		expect(
			screen.getByRole('radio', { name: 'Français', checked: false }).closest('[lang]')
		).toHaveProperty('lang', 'fr');
	});

	it('saves the choice, then reloads in it', async () => {
		mocks.saveLanguage.mockResolvedValue(undefined);
		render(LanguagePicker);

		const user = userEvent.setup();

		await user.click(screen.getByRole('radio', { name: 'Français' }));

		await waitFor(() => expect(mocks.setLocale).toHaveBeenCalledWith('fr'));
		expect(mocks.saveLanguage).toHaveBeenCalledWith('fr');
		expect(mocks.saveLanguage.mock.invocationCallOrder[0]).toBeLessThan(
			mocks.setLocale.mock.invocationCallOrder[0]
		);
	});

	it('stays on the current language when the save fails', async () => {
		mocks.saveLanguage.mockRejectedValue(new Error('down'));
		render(LanguagePicker);

		const user = userEvent.setup();

		await user.click(screen.getByRole('radio', { name: 'Français' }));

		await waitFor(() =>
			expect(toasts.items.map(({ message }) => message)).toEqual([
				'The language was not saved. Please try again.'
			])
		);
		expect(mocks.setLocale).not.toHaveBeenCalled();
		expect(screen.getByRole('radio', { name: 'English', checked: true })).toBeTruthy();
	});

	it('has no axe violations', async () => {
		const { container } = render(LanguagePicker);

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('LanguagePicker in French', () => {
	useLocale('fr');

	it('shows French as the current language', () => {
		render(LanguagePicker);

		expect(screen.getByRole('radio', { name: 'Français', checked: true })).toBeTruthy();
	});
});
