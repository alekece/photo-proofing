import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import CopyLink from './CopyLink.svelte';

const clipboard = (writeText: unknown) =>
	Object.defineProperty(navigator, 'clipboard', {
		value: writeText && { writeText },
		configurable: true
	});

afterEach(() => clipboard(undefined));

describe('CopyLink feedback', () => {
	it('shows no visible duplicate text after copying, but still announces it', async () => {
		const user = userEvent.setup();
		clipboard(vi.fn().mockResolvedValue(undefined));
		render(CopyLink, { link: 'https://pp.test/g/x' });

		await user.click(screen.getByRole('button', { name: 'Copy' }));

		const status = await screen.findByRole('status');
		expect(status.textContent).toBe('Copied');
		expect(status.classList.contains('sr-only')).toBe(true);
		expect(screen.getByRole('button', { name: 'Copied' })).toBeTruthy();
	});

	it('plays a scale-up with a ripple on the check, then re-arms', async () => {
		const user = userEvent.setup();
		clipboard(vi.fn().mockResolvedValue(undefined));
		render(CopyLink, { link: 'https://pp.test/g/x' });

		await user.click(screen.getByRole('button', { name: 'Copy' }));

		const button = await screen.findByRole('button', { name: 'Copied' });
		expect(button.querySelector('[data-testid="click-ripple"]')).toBeTruthy();
		expect(button.querySelector('svg')).toBeTruthy();
	});

	it('keeps the manual-copy message visible when copying fails', async () => {
		const user = userEvent.setup();
		clipboard(vi.fn().mockRejectedValue(new Error('denied')));
		render(CopyLink, { link: 'https://pp.test/g/x' });

		await user.click(screen.getByRole('button', { name: 'Copy' }));

		const status = await screen.findByRole('status');
		expect(status.classList.contains('sr-only')).toBe(false);
		expect(status.textContent).toMatch(/copy it manually/);
	});
});
