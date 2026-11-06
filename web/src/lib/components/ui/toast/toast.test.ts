import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import Toaster from './toaster.svelte';
import { toasts } from './toasts.svelte.js';

describe('Toaster', () => {
	beforeEach(() => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		// jsdom has no Web Animations; finish every transition immediately.
		Element.prototype.animate = (() => {
			const animation = { cancel() {}, pause() {}, play() {}, currentTime: 0 } as Animation;

			Object.defineProperty(animation, 'onfinish', {
				set: (callback: () => void) => queueMicrotask(callback)
			});

			return animation;
		}) as unknown as typeof Element.prototype.animate;
	});
	afterEach(() => {
		toasts.items = [];
		vi.useRealTimers();
	});

	it('shows a success toast and dismisses it automatically', async () => {
		const { container } = render(Toaster);

		toasts.success('Saved');

		expect(await screen.findByRole('status')).toHaveProperty(
			'textContent',
			expect.stringContaining('Saved')
		);
		expect((await axe(container)).violations).toEqual([]);

		await vi.advanceTimersByTimeAsync(4100);

		expect(screen.queryByRole('status')).toBeNull();
	});

	it('keeps an error toast until it is dismissed and runs Retry', async () => {
		const retry = vi.fn();

		render(Toaster);
		toasts.error('Upload failed', retry);
		await vi.advanceTimersByTimeAsync(10_000);

		expect(await screen.findByRole('alert')).toBeTruthy();

		await userEvent.click(screen.getByRole('button', { name: 'Retry' }));

		expect(retry).toHaveBeenCalledOnce();
		expect(screen.queryByRole('alert')).toBeNull();
	});
});
