import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';

import AppHeader from './AppHeader.svelte';
import { createRawSnippet } from 'svelte';

const bell = createRawSnippet(() => ({
	render: () => '<button type="button" aria-label="Notifications">Bell</button>'
}));

describe('AppHeader', () => {
	it('shows the brand and no back link by default', () => {
		render(AppHeader);

		expect(screen.getByText('PhotoProofing')).toBeTruthy();
		expect(screen.getByRole('link', { name: 'PhotoProofing' }).getAttribute('href')).toBe(
			'/dashboard'
		);
		expect(screen.queryByRole('link', { name: 'Back' })).toBeNull();
	});

	it('shows the back link when given', () => {
		render(AppHeader, { back: { href: '/dashboard', label: 'All galleries' } });

		expect(screen.getByRole('link', { name: /All galleries/ }).getAttribute('href')).toBe(
			'/dashboard'
		);
	});

	it('links to Settings from the account menu', async () => {
		render(AppHeader);

		await userEvent.setup().click(screen.getByRole('button', { name: 'Account menu' }));

		expect(screen.getByRole('link', { name: 'Settings' }).getAttribute('href')).toBe('/settings');
	});

	it('calls onsignout when Sign out is pressed', async () => {
		const onsignout = vi.fn();
		render(AppHeader, { onsignout });

		const user = userEvent.setup();

		expect(screen.queryByRole('button', { name: 'Sign out' })).toBeNull();
		await user.click(screen.getByRole('button', { name: 'Account menu' }));
		await user.click(screen.getByRole('button', { name: 'Sign out' }));

		expect(onsignout).toHaveBeenCalledOnce();
	});

	it('shows what the page puts beside the account menu, such as the bell', () => {
		render(AppHeader, { actions: bell });

		expect(screen.getByRole('button', { name: 'Notifications' })).toBeTruthy();
	});

	it('has no axe violations', async () => {
		const { container } = render(AppHeader, {
			back: { href: '/dashboard', label: 'All galleries' }
		});

		expect((await axe(container)).violations).toEqual([]);
	});
});
