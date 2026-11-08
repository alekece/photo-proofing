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
import LanguageMenu from './LanguageMenu.svelte';
import { toasts } from './ui/Toast/toasts.svelte.js';

describe('LanguageMenu', () => {
	useLocale('en');
	afterEach(() => {
		vi.clearAllMocks();
		toasts.items = [];
	});

	it('shows the current language and offers both', async () => {
		render(LanguageMenu);

		const button = screen.getByRole('button', { name: 'Language: English' });

		expect(button.textContent?.trim()).toBe('EN');

		await userEvent.setup().click(button);

		expect(screen.getByRole('radio', { name: 'English' }).getAttribute('aria-checked')).toBe(
			'true'
		);
		expect(screen.getByRole('radio', { name: 'Français' }).getAttribute('aria-checked')).toBe(
			'false'
		);
		expect(screen.getByRole('radio', { name: 'Français' }).getAttribute('lang')).toBe('fr');
	});

	it('saves the choice, then reloads in it', async () => {
		mocks.saveLanguage.mockResolvedValue(undefined);
		render(LanguageMenu);

		const user = userEvent.setup();

		await user.click(screen.getByRole('button', { name: 'Language: English' }));
		await user.click(screen.getByRole('radio', { name: 'Français' }));

		await waitFor(() => expect(mocks.setLocale).toHaveBeenCalledWith('fr'));
		expect(mocks.saveLanguage).toHaveBeenCalledWith('fr');
		expect(mocks.saveLanguage.mock.invocationCallOrder[0]).toBeLessThan(
			mocks.setLocale.mock.invocationCallOrder[0]
		);
	});

	it('stays on the current language when the save fails', async () => {
		mocks.saveLanguage.mockRejectedValue(new Error('down'));
		render(LanguageMenu);

		const user = userEvent.setup();

		await user.click(screen.getByRole('button', { name: 'Language: English' }));
		await user.click(screen.getByRole('radio', { name: 'Français' }));

		await waitFor(() =>
			expect(toasts.items.map(({ message }) => message)).toEqual([
				'The language was not saved. Please try again.'
			])
		);
		expect(mocks.setLocale).not.toHaveBeenCalled();
		expect(screen.getByRole('button', { name: 'Language: English' }).textContent?.trim()).toBe(
			'EN'
		);
	});

	it('has no axe violations', async () => {
		const { container } = render(LanguageMenu);

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('LanguageMenu in French', () => {
	useLocale('fr');

	it('shows French as the current language', () => {
		render(LanguageMenu);

		expect(screen.getByRole('button', { name: 'Langue : français' }).textContent?.trim()).toBe(
			'FR'
		);
	});
});
