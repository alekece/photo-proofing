import { fireEvent, render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import RestoreExpiredDialog from './RestoreExpiredDialog.svelte';

const open = (onsave = vi.fn().mockResolvedValue(true), onclose = vi.fn()) => {
	render(RestoreExpiredDialog, { expiresAt: '2026-09-07T21:59:59Z', onsave, onclose });

	return { onsave, onclose };
};
const tomorrow = () => {
	const date = new Date();
	date.setDate(date.getDate() + 1);

	return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
};

describe('Restore expired dialog', () => {
	it('explains that clients cannot see the gallery and names the expiry date', () => {
		open();

		expect(screen.getByRole('dialog', { name: 'Clients can’t see this gallery yet' })).toBeTruthy();
		expect(screen.getByText(/link expired on 7 Sep/)).toBeTruthy();
		expect(screen.getByLabelText('Open until')).toBe(document.activeElement);
	});

	it('keeps Set date disabled until a date is entered, then saves its end of day', async () => {
		const { onsave } = open();
		const set = screen.getByRole('button', { name: 'Set date' }) as HTMLButtonElement;

		expect(set.disabled).toBe(true);

		await fireEvent.input(screen.getByLabelText('Open until'), { target: { value: tomorrow() } });
		await userEvent.setup().click(set);

		expect(onsave).toHaveBeenCalledOnce();
		expect(new Date(onsave.mock.calls[0][0]).getTime()).toBeGreaterThan(Date.now());
	});

	it('refuses a past date with an inline error tied to the input', async () => {
		const { onsave } = open();
		const input = screen.getByLabelText('Open until');

		await fireEvent.input(input, { target: { value: '2020-01-01' } });
		await userEvent.setup().click(screen.getByRole('button', { name: 'Set date' }));

		expect(onsave).not.toHaveBeenCalled();
		expect((await screen.findByRole('alert')).textContent).toBe('Pick a date from today on.');
		expect(input.getAttribute('aria-describedby')).toBe(screen.getByRole('alert').id);
	});

	it('clears the expiry with No expiry', async () => {
		const { onsave } = open();

		await userEvent.setup().click(screen.getByRole('button', { name: 'No expiry' }));

		expect(onsave).toHaveBeenCalledWith(null);
	});

	it('closes without saving on Keep expired', async () => {
		const { onsave, onclose } = open();

		await userEvent.setup().click(screen.getByRole('button', { name: 'Keep expired' }));

		expect(onsave).not.toHaveBeenCalled();
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('closes without saving on Escape', async () => {
		const { onsave, onclose } = open();

		await userEvent.setup().keyboard('{Escape}');

		expect(onsave).not.toHaveBeenCalled();
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('closes after a successful save and stays open when saving fails', async () => {
		const failing = open(vi.fn().mockResolvedValue(false));
		await userEvent.setup().click(screen.getByRole('button', { name: 'No expiry' }));

		expect(failing.onsave).toHaveBeenCalledOnce();
		expect(failing.onclose).not.toHaveBeenCalled();
		expect(screen.getByRole('dialog')).toBeTruthy();
	});

	it('closes once the save succeeds', async () => {
		const { onclose } = open();

		await userEvent.setup().click(screen.getByRole('button', { name: 'No expiry' }));

		expect(onclose).toHaveBeenCalledOnce();
	});
});
