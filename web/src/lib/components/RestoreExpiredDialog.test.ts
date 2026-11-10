import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import RestoreExpiredDialog from './RestoreExpiredDialog.svelte';

const open = (onsave = vi.fn().mockResolvedValue(true), onclose = vi.fn()) => {
	render(RestoreExpiredDialog, { expiresAt: '2026-09-07T21:59:59Z', onsave, onclose });

	return { onsave, onclose };
};
const thisMonth = (day: string) =>
	within(screen.getByRole('grid')).getByText(day, {
		selector: '[data-bits-day]:not([data-outside-month])'
	});
const lastDayOfMonth = () => {
	const now = new Date();

	return String(new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate());
};

describe('Restore expired dialog', () => {
	it('explains that clients cannot see the gallery and names the expiry date', () => {
		open();

		expect(screen.getByRole('dialog', { name: 'Clients can’t see this gallery yet' })).toBeTruthy();
		expect(screen.getByText(/link expired on 7 Sep/)).toBeTruthy();
		expect(screen.getByLabelText('Open until')).toBe(document.activeElement);
	});

	it('keeps Set date disabled until a date is picked, then saves its end of day', async () => {
		const { onsave } = open();
		const user = userEvent.setup();
		const set = screen.getByRole('button', { name: 'Set date' }) as HTMLButtonElement;

		expect(set.disabled).toBe(true);

		await user.click(screen.getByLabelText('Open until'));
		await user.click(thisMonth(lastDayOfMonth()));
		await user.click(set);

		expect(onsave).toHaveBeenCalledOnce();
		expect(new Date(onsave.mock.calls[0][0]).getTime()).toBeGreaterThan(Date.now());
	});

	it('does not offer days before today', async () => {
		open();

		await userEvent.setup().click(screen.getByLabelText('Open until'));

		const today = new Date().getDate();

		if (today > 1) expect(thisMonth(String(today - 1)).hasAttribute('data-disabled')).toBe(true);
		expect(thisMonth(String(today)).hasAttribute('data-disabled')).toBe(false);
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
