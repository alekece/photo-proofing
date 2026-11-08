import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn(), refreshAll: vi.fn() }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://localhost/login') } }));
vi.mock('#lib/auth.js', async (original) => ({
	...(await original<typeof import('#lib/auth.js')>()),
	authRequest: vi.fn()
}));
import { AuthError, authRequest } from '#lib/auth.js';
import { useLocale } from '#lib/i18n/testing.js';
import Login from './+page.svelte';

const open = (setupAvailable: boolean) =>
	render(Login, { props: { data: { setupAvailable } as never } });

beforeEach(() => {
	vi.mocked(authRequest).mockReset();
});

describe('login page', () => {
	it('asks to confirm the password only during setup', async () => {
		const { unmount } = open(true);

		expect(await screen.findByLabelText('Confirm password')).toBeTruthy();
		unmount();
		open(false);
		await screen.findByLabelText('Email');

		expect(screen.queryByLabelText('Confirm password')).toBeNull();
	});

	it('rejects a mismatch without calling the API', async () => {
		const user = userEvent.setup();
		open(true);
		await waitFor(() =>
			expect((screen.getByLabelText('Email') as HTMLInputElement).disabled).toBe(false)
		);

		await user.type(screen.getByLabelText('Email'), 'a@example.test');
		await user.type(screen.getByLabelText('Password', { exact: true }), 'first password 123');
		await user.type(screen.getByLabelText('Confirm password'), 'other password 123');
		await user.click(screen.getByRole('button', { name: 'Create account' }));

		expect(screen.getByRole('alert').textContent).toBe("Passwords don't match.");
		expect(authRequest).not.toHaveBeenCalled();
	});

	it('clears both passwords after a rejected request', async () => {
		vi.mocked(authRequest).mockRejectedValue(new AuthError(401, 'refused'));
		const user = userEvent.setup();
		open(true);
		await waitFor(() =>
			expect((screen.getByLabelText('Email') as HTMLInputElement).disabled).toBe(false)
		);

		await user.type(screen.getByLabelText('Email'), 'a@example.test');
		await user.type(screen.getByLabelText('Password', { exact: true }), 'same password 1234');
		await user.type(screen.getByLabelText('Confirm password'), 'same password 1234');
		await user.click(screen.getByRole('button', { name: 'Create account' }));

		expect((await screen.findByRole('alert')).textContent).toBe('Invalid email or password.');
		expect((screen.getByLabelText('Password', { exact: true }) as HTMLInputElement).value).toBe('');
		expect((screen.getByLabelText('Confirm password') as HTMLInputElement).value).toBe('');
	});
});

describe('login page in French', () => {
	useLocale('fr');

	it('speaks the workspace language', async () => {
		open(false);

		expect(await screen.findByRole('heading', { level: 1, name: 'Connexion' })).toBeTruthy();
		expect(screen.getByLabelText('Mot de passe')).toBeTruthy();
		expect(screen.queryByRole('heading', { name: 'Sign in' })).toBeNull();
	});

	it('explains too many attempts', async () => {
		vi.mocked(authRequest).mockRejectedValue(new AuthError(429, 'busy'));
		const user = userEvent.setup();
		open(false);
		await waitFor(() =>
			expect((screen.getByLabelText('E-mail') as HTMLInputElement).disabled).toBe(false)
		);

		await user.type(screen.getByLabelText('E-mail'), 'a@example.test');
		await user.type(screen.getByLabelText('Mot de passe'), 'some password 1234');
		await user.click(screen.getByRole('button', { name: 'Se connecter' }));

		expect((await screen.findByRole('alert')).textContent).toBe(
			'Trop de tentatives de connexion. Patientez un instant, puis réessayez.'
		);
	});
});
