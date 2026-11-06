import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn(), afterNavigate: vi.fn() }));
import Page from './+page.svelte';
import { toasts } from '#lib/components/ui/toast/toasts.svelte.js';
import { DEFAULT_SETTINGS } from '#lib/settings/settings.js';

const data = { idleTimeoutMs: 60000, settings: DEFAULT_SETTINGS };

function open() {
	return render(Page, { props: { data } as never });
}

describe('Settings page', () => {
	it('keeps Save and Cancel disabled until something changes', async () => {
		open();
		const user = userEvent.setup();

		expect(screen.getByRole('button', { name: 'Save changes' }).hasAttribute('disabled')).toBe(
			true
		);
		expect(screen.getByRole('button', { name: 'Cancel' }).hasAttribute('disabled')).toBe(true);

		await user.type(screen.getByLabelText('Studio name'), 'Studio X');

		expect(screen.getByRole('button', { name: 'Save changes' }).hasAttribute('disabled')).toBe(
			false
		);
	});

	it('saves, shows Saved and announces the regeneration', async () => {
		toasts.items = [];
		open();
		const user = userEvent.setup();

		await user.type(screen.getByLabelText('Studio name'), 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		expect(await screen.findByRole('button', { name: /Saved/ })).toBeTruthy();
		expect(toasts.items.map((toast) => toast.message)).toContain(
			'Watermarked proofs are being regenerated'
		);
	});

	it('does not announce a regeneration for a signature-only change', async () => {
		toasts.items = [];
		open();
		const user = userEvent.setup();

		await user.click(screen.getByRole('radio', { name: 'None' }));
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		expect(toasts.items).toEqual([]);
	});

	it('reverts every unsaved change on Cancel', async () => {
		open();
		const user = userEvent.setup();
		const name = screen.getByLabelText('Studio name') as HTMLInputElement;

		await user.type(name, 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Cancel' }));

		expect(name.value).toBe('');
		expect(await screen.findByRole('button', { name: /Reverted/ })).toBeTruthy();
	});

	it('disables logo options and the position grid until they apply', async () => {
		open();

		expect(screen.getByRole('radio', { name: 'Logo' }).hasAttribute('disabled')).toBe(true);
		expect(screen.getByRole('radio', { name: 'Both' }).hasAttribute('disabled')).toBe(true);
		expect(screen.getByRole('button', { name: 'Logo' }).hasAttribute('disabled')).toBe(true);
		expect(screen.getByRole('button', { name: 'Position 1' }).closest('fieldset')?.disabled).toBe(
			true
		);

		await userEvent.setup().click(screen.getByRole('button', { name: 'Once' }));

		expect(screen.getByRole('button', { name: 'Position 1' }).closest('fieldset')?.disabled).toBe(
			false
		);
	});

	it('has no accessibility violations', async () => {
		const { container } = open();

		expect((await axe(container)).violations).toEqual([]);
	});
});
