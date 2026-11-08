import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import ShareLink from './ShareLink.svelte';

const token = 'A'.repeat(22);
const link = `https://pp.test/g/${token}`;
const clipboard = (writeText: unknown) =>
	Object.defineProperty(navigator, 'clipboard', {
		value: writeText && { writeText },
		configurable: true
	});

afterEach(() => {
	clipboard(undefined);
	vi.useRealTimers();
});

describe('ShareLink', () => {
	it('shows an inactive, read-only field that is still selectable', () => {
		render(ShareLink, { token, origin: 'https://pp.test' });
		const input = screen.getByLabelText('Client link') as HTMLInputElement;

		expect(input.readOnly).toBe(true);
		expect(input.disabled).toBe(false);
		expect(input.compareDocumentPosition(screen.getByRole('button', { name: 'Copy' }))).toBe(
			Node.DOCUMENT_POSITION_FOLLOWING
		);
	});

	it('shows Copied with a check, then returns to Copy after 3 s; clicks restart the delay', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		clipboard(vi.fn().mockResolvedValue(undefined));
		render(ShareLink, { token, origin: 'https://pp.test' });

		await user.click(screen.getByRole('button', { name: 'Copy' }));
		const button = await screen.findByRole('button', { name: 'Copied' });
		expect(button.querySelector('svg')).toBeTruthy();

		await vi.advanceTimersByTimeAsync(2000);
		await user.click(button);
		await vi.advanceTimersByTimeAsync(2000);
		expect(screen.getByRole('button', { name: 'Copied' })).toBeTruthy();

		await vi.advanceTimersByTimeAsync(1500);
		expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
	});

	it('does not confirm when copying fails', async () => {
		const user = userEvent.setup();
		clipboard(vi.fn().mockRejectedValue(new Error('denied')));
		render(ShareLink, { token, origin: 'https://pp.test' });

		await user.click(screen.getByRole('button', { name: 'Copy' }));

		expect(screen.queryByRole('button', { name: 'Copied' })).toBeNull();
	});

	it('offers copying the client link without a photographer Open shortcut', () => {
		render(ShareLink, { token, origin: 'https://pp.test' });

		expect((screen.getByLabelText('Client link') as HTMLInputElement).value).toBe(link);
		expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
		expect(screen.queryByRole('link', { name: 'Open' })).toBeNull();
	});

	it('copies the exact link and announces it', async () => {
		const writeText = vi.fn().mockResolvedValue(undefined);
		const user = userEvent.setup();
		clipboard(writeText);
		render(ShareLink, { token, origin: 'https://pp.test' });

		await user.click(screen.getByRole('button', { name: 'Copy' }));

		expect(writeText).toHaveBeenCalledWith(link);
		expect((await screen.findByRole('status')).textContent).toMatch(/Copied/);
	});

	it.each([undefined, vi.fn().mockRejectedValue(new Error('denied'))])(
		'selects the link and asks to copy manually when the clipboard is unusable',
		async (writeText) => {
			const user = userEvent.setup();
			clipboard(writeText);
			render(ShareLink, { token, origin: 'https://pp.test' });

			await user.click(screen.getByRole('button', { name: 'Copy' }));

			const input = screen.getByLabelText('Client link') as HTMLInputElement;

			expect(document.activeElement).toBe(input);
			expect(input.selectionEnd! - input.selectionStart!).toBe(link.length);
			expect((await screen.findByRole('status')).textContent).toMatch(/copy it manually/i);
		}
	);

	it('is reachable by keyboard', async () => {
		render(ShareLink, { token, origin: 'https://pp.test' });
		const user = userEvent.setup();

		await user.tab();
		expect(document.activeElement).toBe(screen.getByLabelText('Client link'));

		await user.tab();
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Copy' }));
	});
});
