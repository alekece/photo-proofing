import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import Toaster from './Toaster.svelte';
import { SAVED_QUIET_MS, toasts } from './toasts.svelte.js';

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

	it('shows one Saved toast after a quiet period, however many saves came before', async () => {
		render(Toaster);

		toasts.saved();
		await vi.advanceTimersByTimeAsync(SAVED_QUIET_MS - 1);
		toasts.saved();
		await vi.advanceTimersByTimeAsync(SAVED_QUIET_MS - 1);

		expect(toasts.items).toEqual([]);

		await vi.advanceTimersByTimeAsync(1);

		expect(toasts.items.map((toast) => toast.message)).toEqual(['Saved']);
	});

	it('shows a countdown bar only on auto-dismissing toasts', async () => {
		const { container } = render(Toaster);

		toasts.error('Upload failed');
		await screen.findByRole('alert');

		expect(container.querySelector('[data-slot="toast-countdown"]')).toBeNull();

		toasts.success('Saved');
		await screen.findByRole('status');

		const bar = container.querySelector<HTMLElement>('[data-slot="toast-countdown"]');

		expect(bar?.style.getPropertyValue('--toast-ms')).toBe('4000ms');
	});

	it('still closes an auto-dismissing toast with the close button', async () => {
		render(Toaster);
		toasts.success('Saved');
		await userEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));

		expect(screen.queryByRole('status')).toBeNull();
	});

	it('does not stack an identical toast while one is visible', async () => {
		render(Toaster);

		const first = toasts.success('Copied');
		await vi.advanceTimersByTimeAsync(3000);

		expect(toasts.success('Copied')).toBe(first);
		expect(toasts.error('Copied')).not.toBe(first);
		expect(toasts.items).toHaveLength(2);

		// The duplicate left the original timer alone.
		await vi.advanceTimersByTimeAsync(1100);

		expect(toasts.items.map((toast) => toast.kind)).toEqual(['error']);
		expect(toasts.success('Copied')).not.toBe(first);
	});
});
