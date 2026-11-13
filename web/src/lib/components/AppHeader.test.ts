import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';

const state = vi.hoisted(() => ({
	page: { url: new URL('http://localhost/dashboard') }
}));
vi.mock('$app/state', () => state);

import AppHeader from './AppHeader.svelte';
import { createRawSnippet } from 'svelte';

const bell = createRawSnippet(() => ({
	render: () => '<button type="button" aria-label="Notifications">Bell</button>'
}));

beforeEach(() => {
	state.page.url = new URL('http://localhost/dashboard');
});

describe('AppHeader', () => {
	it('shows the brand and main section pills with active state and disabled Activity', () => {
		render(AppHeader);

		expect(screen.getByRole('link', { name: 'PhotoProofing' }).getAttribute('href')).toBe(
			'/dashboard'
		);

		const galleries = screen.getByRole('link', { name: 'Galleries' });
		const contacts = screen.getByRole('link', { name: 'Contacts' });
		const activity = screen.getByRole('button', { name: 'Activity' }) as HTMLButtonElement;

		expect(galleries.getAttribute('href')).toBe('/dashboard');
		expect(galleries.getAttribute('aria-current')).toBe('page');
		expect(contacts.getAttribute('href')).toBe('/contacts');
		expect(contacts.getAttribute('aria-current')).toBeNull();
		expect(activity.disabled).toBe(true);
		expect(activity.title).toBe('Coming soon');
	});

	it('marks Contacts as active when viewing /contacts', () => {
		state.page.url = new URL('http://localhost/contacts');
		render(AppHeader);

		expect(screen.getByRole('link', { name: 'Contacts' }).getAttribute('aria-current')).toBe(
			'page'
		);
		expect(screen.getByRole('link', { name: 'Galleries' }).getAttribute('aria-current')).toBeNull();
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
		const { container } = render(AppHeader);

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('AppHeader in French', () => {
	useLocale('fr');

	it('translates navigation pills and Coming soon tooltip', async () => {
		const { container } = render(AppHeader);

		expect(screen.getByRole('link', { name: 'Galeries' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Contacts' })).toBeTruthy();
		const activity = screen.getByRole('button', { name: 'Activité' }) as HTMLButtonElement;
		expect(activity.disabled).toBe(true);
		expect(activity.title).toBe('Bientôt disponible');
		expect((await axe(container)).violations).toEqual([]);
	});
});
