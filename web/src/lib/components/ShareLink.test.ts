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

afterEach(() => clipboard(undefined));

describe('ShareLink', () => {
	it('shows the private link built from the origin and the empty-gallery note', () => {
		render(ShareLink, { token, origin: 'https://pp.test' });

		expect((screen.getByLabelText('Client link') as HTMLInputElement).value).toBe(link);
		expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
		expect(screen.getByText(/no photos yet/i)).toBeTruthy();
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
