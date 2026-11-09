import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
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

	it('shows the version with a full-value tooltip and nothing to interact with', () => {
		render(SiteFooter, { version });

		expect(screen.getByText('Build 0123456').getAttribute('title')).toBe(version);
		expect(screen.queryByRole('button')).toBeNull();
		expect(screen.queryByRole('status')).toBeNull();
	});

	it('labels an unstamped local build', () => {
		render(SiteFooter, { version: '' });
		expect(screen.getByText('Development build')).toBeTruthy();
	});

	it('lists legal notices only when configured', () => {
		const { unmount } = render(SiteFooter, { version });
		expect(screen.queryByRole('navigation', { name: 'Legal' })).toBeNull();
		unmount();

		render(SiteFooter, { version, legalNotices: [{ label: 'Imprint', href: '/imprint' }] });
		expect(screen.getByRole('link', { name: 'Imprint' }).getAttribute('href')).toBe('/imprint');
	});
});
