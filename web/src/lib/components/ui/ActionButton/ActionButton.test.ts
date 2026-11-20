import { fireEvent, render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import ActionButton from './ActionButton.svelte';
import ActionButtonHarness from './ActionButtonHarness.test.svelte';

const deleteLabels = { idle: 'Delete', pending: 'Deleting' };
const confirmation = {
	duration: 3000,
	cancelLabel: 'Cancel deletion of Camille',
	description: 'Personal gallery links for this contact will stop working.'
};
const saveLabels = { idle: 'Save', pending: 'Saving', succeeded: 'Saved' };

afterEach(() => vi.useRealTimers());

describe('ActionButton', () => {
	it('keeps focus and runs only after the confirmation window', async () => {
		vi.useFakeTimers();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const onaction = vi.fn(async () => true);

		render(ActionButton, {
			labels: deleteLabels,
			confirmation,
			onaction,
			idleLabel: 'Delete Camille'
		});
		const button = screen.getByRole('button', { name: 'Delete Camille' });
		await user.click(button);
		expect(screen.getByRole('button', { name: confirmation.cancelLabel })).toBe(button);
		expect(document.activeElement).toBe(button);

		await vi.advanceTimersByTimeAsync(2999);
		expect(onaction).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1);
		expect(onaction).toHaveBeenCalledOnce();
	});

	it.each(['click', 'Enter', 'Escape', 'unmount'])('cancels the timer on %s', async (cancel) => {
		vi.useFakeTimers();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const onaction = vi.fn(async () => true);
		const { unmount } = render(ActionButton, { labels: deleteLabels, confirmation, onaction });
		const button = screen.getByRole('button', { name: 'Delete' });

		await user.click(button);
		if (cancel === 'click') await user.click(button);
		else if (cancel === 'unmount') unmount();
		else await user.keyboard(`{${cancel}}`);

		await vi.advanceTimersByTimeAsync(3000);
		expect(onaction).not.toHaveBeenCalled();
	});

	it('submits once, then confirms a successful save before returning to idle', async () => {
		vi.useFakeTimers();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		let resolve!: (value: boolean) => void;
		const onaction = vi.fn(() => new Promise<boolean>((answer) => (resolve = answer)));
		const onsettled = vi.fn();

		render(ActionButtonHarness, { labels: saveLabels, onaction, onsettled });
		await user.click(screen.getByRole('button', { name: 'Save' }));
		await fireEvent.submit(screen.getByLabelText('Name').closest('form')!);
		expect(onaction).toHaveBeenCalledOnce();
		expect(screen.getByRole('button', { name: 'Saving' }).getAttribute('aria-disabled')).toBe(
			'true'
		);

		resolve(true);
		await vi.advanceTimersByTimeAsync(1599);
		expect(screen.getByRole('button', { name: 'Saved' })).toBeTruthy();
		expect(onsettled).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1);
		expect(screen.getByRole('button', { name: 'Save' })).toBeTruthy();
		expect(onsettled).toHaveBeenCalledOnce();
	});

	it('returns to idle after a failed action so it can be retried', async () => {
		const user = userEvent.setup();
		const onaction = vi.fn(async () => false);

		render(ActionButton, { labels: saveLabels, onaction });
		await user.click(screen.getByRole('button', { name: 'Save' }));
		await user.click(screen.getByRole('button', { name: 'Save' }));

		expect(onaction).toHaveBeenCalledTimes(2);
		expect(screen.queryByRole('button', { name: 'Saved' })).toBeNull();
	});

	it('describes the consequence of an armed icon-only action', async () => {
		const { container } = render(ActionButton, {
			labels: deleteLabels,
			confirmation,
			onaction: vi.fn(),
			compactOnMobile: true
		});
		await userEvent.setup().click(screen.getByRole('button', { name: 'Delete' }));
		const button = screen.getByRole('button', { name: confirmation.cancelLabel });
		const description = document.getElementById(button.getAttribute('aria-describedby') ?? '');

		expect(description?.textContent?.trim()).toBe(confirmation.description);
		expect((await axe(container)).violations).toEqual([]);
	});
});
