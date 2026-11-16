import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import CopyButton from './CopyButton.svelte';

const clipboard = (writeText: unknown) =>
	Object.defineProperty(navigator, 'clipboard', {
		value: writeText && { writeText },
		configurable: true
	});

afterEach(() => {
	clipboard(undefined);
	vi.useRealTimers();
});

describe('CopyButton', () => {
	it('copies value, announces Copied, and resets to Copy after 3 seconds', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const writeText = vi.fn().mockResolvedValue(undefined);
		clipboard(writeText);

		render(CopyButton, { value: '042819' });

		await user.click(screen.getByRole('button', { name: 'Copy' }));

		expect(writeText).toHaveBeenCalledWith('042819');
		const button = await screen.findByRole('button', { name: 'Copied' });
		expect(button.querySelector('[data-testid="click-ripple"]')).toBeTruthy();

		const status = screen.getByRole('status');
		expect(status.textContent).toBe('Copied');
		expect(status.classList.contains('sr-only')).toBe(true);

		await vi.advanceTimersByTimeAsync(3000);
		expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
		expect(screen.getByRole('status').textContent).toBe('');
	});

	it('re-arms to Copy immediately when value changes', async () => {
		const user = userEvent.setup();
		clipboard(vi.fn().mockResolvedValue(undefined));

		const view = render(CopyButton, { value: '042819' });
		await user.click(screen.getByRole('button', { name: 'Copy' }));
		expect(await screen.findByRole('button', { name: 'Copied' })).toBeTruthy();

		await view.rerender({ value: '918273' });
		expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
	});

	it('focuses and selects fallbackInput and reveals statusClass on clipboard failure', async () => {
		const user = userEvent.setup();
		clipboard(vi.fn().mockRejectedValue(new Error('denied')));

		const fallbackInput = document.createElement('input');
		fallbackInput.value = '042819';
		document.body.appendChild(fallbackInput);
		const focusSpy = vi.spyOn(fallbackInput, 'focus');
		const selectSpy = vi.spyOn(fallbackInput, 'select');

		try {
			render(CopyButton, {
				value: '042819',
				fallbackInput,
				statusClass: 'text-xs text-danger'
			});

			await user.click(screen.getByRole('button', { name: 'Copy' }));

			const status = await screen.findByRole('status');
			expect(status.classList.contains('sr-only')).toBe(false);
			expect(status.className).toContain('text-danger');
			expect(status.textContent).toMatch(/copy it manually/);
			expect(focusSpy).toHaveBeenCalledOnce();
			expect(selectSpy).toHaveBeenCalledOnce();
		} finally {
			fallbackInput.remove();
		}
	});

	it('respects disabled prop', () => {
		render(CopyButton, { value: '12', disabled: true });
		expect((screen.getByRole('button', { name: 'Copy' }) as HTMLButtonElement).disabled).toBe(true);
	});
});
