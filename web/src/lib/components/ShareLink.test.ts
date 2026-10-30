import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
import { goto } from '$app/navigation';
import { GalleryApiError, type Gallery } from '#lib/gallery/api.js';
import ShareLink from './ShareLink.svelte';

const token = 'A'.repeat(22);
const link = `https://pp.test/g/${token}`;
const gallery = (share_token: string | null = null) =>
	({ id: 'g1', share_token }) as unknown as Gallery;
const sharing = () => vi.fn().mockResolvedValue(gallery(token));
const clipboard = (writeText: unknown) =>
	Object.defineProperty(navigator, 'clipboard', {
		value: writeText && { writeText },
		configurable: true
	});

afterEach(() => clipboard(undefined));

describe('ShareLink', () => {
	it('offers only Share until the gallery is shared', async () => {
		const share = sharing();
		render(ShareLink, { gallery: gallery(), origin: 'https://pp.test', share });
		expect(screen.queryByRole('textbox')).toBeNull();
		await userEvent.setup().click(screen.getByRole('button', { name: 'Share' }));
		expect(share).toHaveBeenCalledTimes(1);
		expect(share).toHaveBeenCalledWith('g1');
		expect(((await screen.findByLabelText('Client link')) as HTMLInputElement).value).toBe(link);
		expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
		expect(screen.getByText(/no photos yet/i)).toBeTruthy();
	});

	it('shows an existing link without sharing again', () => {
		const share = sharing();
		render(ShareLink, { gallery: gallery(token), origin: 'https://pp.test', share });
		expect((screen.getByLabelText('Client link') as HTMLInputElement).value).toBe(link);
		expect(screen.queryByRole('button', { name: 'Share' })).toBeNull();
		expect(share).not.toHaveBeenCalled();
	});

	it('copies the exact link and announces it', async () => {
		const writeText = vi.fn().mockResolvedValue(undefined);
		const user = userEvent.setup();
		clipboard(writeText);
		render(ShareLink, { gallery: gallery(token), origin: 'https://pp.test', share: sharing() });
		await user.click(screen.getByRole('button', { name: 'Copy' }));
		expect(writeText).toHaveBeenCalledWith(link);
		expect((await screen.findByRole('status')).textContent).toMatch(/Copied/);
	});

	it.each([undefined, vi.fn().mockRejectedValue(new Error('denied'))])(
		'selects the link and asks to copy manually when the clipboard is unusable',
		async (writeText) => {
			const user = userEvent.setup();
			clipboard(writeText);
			render(ShareLink, { gallery: gallery(token), origin: 'https://pp.test', share: sharing() });
			await user.click(screen.getByRole('button', { name: 'Copy' }));
			const input = screen.getByLabelText('Client link') as HTMLInputElement;
			expect(document.activeElement).toBe(input);
			expect(input.selectionEnd! - input.selectionStart!).toBe(link.length);
			expect((await screen.findByRole('status')).textContent).toMatch(/copy it manually/i);
		}
	);

	it('shows an alert and no link when sharing fails, and stays retryable', async () => {
		const share = vi
			.fn()
			.mockRejectedValueOnce(new Error('down'))
			.mockResolvedValue(gallery(token));
		render(ShareLink, { gallery: gallery(), origin: 'https://pp.test', share });
		const user = userEvent.setup();
		await user.click(screen.getByRole('button', { name: 'Share' }));
		expect((await screen.findByRole('alert')).textContent).toMatch(/couldn.t share/i);
		expect(screen.queryByLabelText('Client link')).toBeNull();
		await user.click(screen.getByRole('button', { name: 'Share' }));
		expect(await screen.findByLabelText('Client link')).toBeTruthy();
	});

	it('sends an expired session to sign-in instead of offering a retry', async () => {
		const share = vi.fn().mockRejectedValue(new GalleryApiError(401, 'unauthorized'));
		render(ShareLink, { gallery: gallery(), origin: 'https://pp.test', share });
		await userEvent.setup().click(screen.getByRole('button', { name: 'Share' }));
		await vi.waitFor(() =>
			expect(goto).toHaveBeenCalledWith('/login?expired=1', { refreshAll: true })
		);
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('is reachable by keyboard', async () => {
		render(ShareLink, { gallery: gallery(), origin: 'https://pp.test', share: sharing() });
		const user = userEvent.setup();
		await user.tab();
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Share' }));
		await user.keyboard('{Enter}');
		await screen.findByLabelText('Client link');
		await user.tab();
		expect(document.activeElement).toBe(screen.getByLabelText('Client link'));
		await user.tab();
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Copy' }));
	});
});
