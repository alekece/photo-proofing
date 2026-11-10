import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import Toaster from './Toaster.svelte';
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

	it('closes a toast with the close button and has no axe violations', async () => {
		const { container } = render(Toaster);

		toasts.error('Upload failed');
		await screen.findByRole('alert');

		expect((await axe(container)).violations).toEqual([]);

		await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));

		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('does not stack an identical toast while one is visible', () => {
		render(Toaster);

		const first = toasts.error('Copied');

		expect(toasts.error('Copied')).toBe(first);
		expect(toasts.items).toHaveLength(1);

		toasts.dismiss(first);

		expect(toasts.error('Copied')).not.toBe(first);
	});
});
