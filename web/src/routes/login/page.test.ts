import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn(), refreshAll: vi.fn() }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://localhost/login') } }));
vi.mock('#lib/auth.js', () => ({ authRequest: vi.fn() }));
import { authRequest } from '#lib/auth.js';
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
		vi.mocked(authRequest).mockRejectedValue(new Error('nope'));
		const user = userEvent.setup();
		open(true);
		await waitFor(() =>
			expect((screen.getByLabelText('Email') as HTMLInputElement).disabled).toBe(false)
		);

		await user.type(screen.getByLabelText('Email'), 'a@example.test');
		await user.type(screen.getByLabelText('Password', { exact: true }), 'same password 1234');
		await user.type(screen.getByLabelText('Confirm password'), 'same password 1234');
		await user.click(screen.getByRole('button', { name: 'Create account' }));

		expect((await screen.findByRole('alert')).textContent).toBe('nope');
		expect((screen.getByLabelText('Password', { exact: true }) as HTMLInputElement).value).toBe('');
		expect((screen.getByLabelText('Confirm password') as HTMLInputElement).value).toBe('');
	});
});
