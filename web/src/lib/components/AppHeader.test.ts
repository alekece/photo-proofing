import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';

import AppHeader from './AppHeader.svelte';

describe('AppHeader', () => {
	it('shows the brand and no back link by default', () => {
		render(AppHeader);

		expect(screen.getByText('PhotoProofing')).toBeTruthy();
		expect(screen.queryByRole('link')).toBeNull();
	});

	it('shows the back link when given', () => {
		render(AppHeader, { back: { href: '/dashboard', label: 'All galleries' } });

		expect(screen.getByRole('link', { name: /All galleries/ }).getAttribute('href')).toBe(
			'/dashboard'
		);
	});

	it('calls onsignout when Sign out is pressed', async () => {
		const onsignout = vi.fn();
		render(AppHeader, { onsignout });

		await userEvent.setup().click(screen.getByRole('button', { name: 'Sign out' }));

		expect(onsignout).toHaveBeenCalledOnce();
	});

	it('has no axe violations', async () => {
		const { container } = render(AppHeader, {
			back: { href: '/dashboard', label: 'All galleries' }
		});

		expect((await axe(container)).violations).toEqual([]);
	});
});
