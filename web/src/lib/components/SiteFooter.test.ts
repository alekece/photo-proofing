import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import SiteFooter from './SiteFooter.svelte';

const version = '0123456789abcdef0123456789abcdef01234567';

describe('shared footer', () => {
	it('shows the brand, copyright and short build version', () => {
		render(SiteFooter, { version });

		const footer = screen.getByRole('contentinfo');
		expect(footer.textContent).toContain('PhotoProofing');
		expect(footer.textContent).toContain(`© ${new Date().getFullYear()}`);
		expect(screen.getByText('Build 0123456')).toBeTruthy();
	});

	it('copies the complete version', async () => {
		const user = userEvent.setup();
		const copy = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
		render(SiteFooter, { version });

		await user.click(screen.getByRole('button', { name: 'Copy build version' }));
		expect(copy).toHaveBeenCalledWith(version);
		expect(screen.getByRole('status').textContent).toBe('Copied');
	});

	it('offers manual copying when clipboard access fails', async () => {
		const user = userEvent.setup();
		vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('Denied'));
		render(SiteFooter, { version });

		await user.click(screen.getByRole('button', { name: 'Copy build version' }));
		const input = screen.getByRole('textbox', { name: 'Full build version' });
		expect(input).toHaveProperty('value', version);
		expect(document.activeElement).toBe(input);
		expect(screen.getByRole('status').textContent).toContain('copy it manually');
	});

	it('shows the version without a copy action when not copyable', () => {
		render(SiteFooter, { version, copyable: false });
		expect(screen.getByText('Build 0123456')).toBeTruthy();
		expect(screen.queryByRole('button')).toBeNull();
	});

	it('labels an unstamped local build without a copy action', () => {
		render(SiteFooter, { version: '' });
		expect(screen.getByText('Development build')).toBeTruthy();
		expect(screen.queryByRole('button')).toBeNull();
	});

	it('lists legal notices only when configured', () => {
		const { unmount } = render(SiteFooter, { version });
		expect(screen.queryByRole('navigation', { name: 'Legal' })).toBeNull();
		unmount();

		render(SiteFooter, { version, legalNotices: [{ label: 'Imprint', href: '/imprint' }] });
		expect(screen.getByRole('link', { name: 'Imprint' }).getAttribute('href')).toBe('/imprint');
	});
});
