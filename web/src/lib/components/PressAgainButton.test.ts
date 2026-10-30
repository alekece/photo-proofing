import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import PressAgainButton, { CONFIRM_WINDOW } from './PressAgainButton.svelte';

const setup = (props: { disabled?: boolean } = {}) => {
	const onconfirm = vi.fn();
	const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
	render(PressAgainButton, {
		label: 'Share gallery',
		confirmLabel: 'Press again to share',
		onconfirm,
		...props
	});

	return { onconfirm, user };
};

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('PressAgainButton', () => {
	it('arms on the first press and fires on the second', async () => {
		const { onconfirm, user } = setup();

		await user.click(screen.getByRole('button', { name: 'Share gallery' }));

		expect(onconfirm).not.toHaveBeenCalled();

		await user.click(screen.getByRole('button', { name: 'Press again to share' }));

		expect(onconfirm).toHaveBeenCalledOnce();
		expect(screen.getByRole('button', { name: 'Share gallery' })).toBeTruthy();
	});

	it('does not fire after the window has passed', async () => {
		const { onconfirm, user } = setup();

		await user.click(screen.getByRole('button', { name: 'Share gallery' }));
		vi.advanceTimersByTime(CONFIRM_WINDOW + 1);
		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Share gallery' })).toBeTruthy()
		);
		await user.click(screen.getByRole('button', { name: 'Share gallery' }));

		expect(onconfirm).not.toHaveBeenCalled();
		expect(screen.getByRole('button', { name: 'Press again to share' })).toBeTruthy();
	});

	it('disarms on Escape', async () => {
		const { onconfirm, user } = setup();

		await user.click(screen.getByRole('button', { name: 'Share gallery' }));
		await user.keyboard('{Escape}');

		expect(screen.getByRole('button', { name: 'Share gallery' })).toBeTruthy();
		expect(onconfirm).not.toHaveBeenCalled();
	});

	it('disarms on a click elsewhere', async () => {
		const { onconfirm, user } = setup();

		await user.click(screen.getByRole('button', { name: 'Share gallery' }));
		await user.click(document.body);

		expect(screen.getByRole('button', { name: 'Share gallery' })).toBeTruthy();
		expect(onconfirm).not.toHaveBeenCalled();
	});

	it('cannot be armed while disabled', async () => {
		const { onconfirm, user } = setup({ disabled: true });

		await user.click(screen.getByRole('button', { name: 'Share gallery' }));

		expect(screen.queryByRole('button', { name: 'Press again to share' })).toBeNull();
		expect(onconfirm).not.toHaveBeenCalled();
	});

	it('announces the confirmation politely once armed', async () => {
		const { user } = setup();

		await user.click(screen.getByRole('button', { name: 'Share gallery' }));

		const live = document.querySelector('[aria-live="polite"]');

		expect(live?.textContent).toBe('Press again to share');
	});
});
