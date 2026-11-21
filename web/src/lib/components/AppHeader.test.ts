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
	it('shows the brand and the rail links in order with their active state', () => {
		render(AppHeader);

		expect(screen.getByRole('link', { name: 'PhotoProofing' }).getAttribute('href')).toBe(
			'/dashboard'
		);

		const nav = screen.getByRole('navigation', { name: 'Main sections' });
		const links = nav.querySelectorAll('a');
		const names = [...links].map((link) => link.textContent?.trim());

		expect(names).toEqual(['Galleries', 'Photos', 'Contacts', 'Activity', 'Settings']);

		const galleries = screen.getByRole('link', { name: 'Galleries' });
		const allPhotos = screen.getByRole('link', { name: 'Photos' });
		const contacts = screen.getByRole('link', { name: 'Contacts' });
		const activity = screen.getByRole('link', { name: 'Activity' });

		expect(galleries.getAttribute('href')).toBe('/dashboard');
		expect(galleries.getAttribute('aria-current')).toBe('page');
		expect(allPhotos.getAttribute('href')).toBe('/photos');
		expect(allPhotos.getAttribute('aria-current')).toBeNull();
		expect(contacts.getAttribute('href')).toBe('/contacts');
		expect(contacts.getAttribute('aria-current')).toBeNull();
		expect(activity.getAttribute('href')).toBe('/activity');
		expect(activity.getAttribute('aria-current')).toBeNull();
	});

	it('marks Activity as active when viewing /activity', () => {
		state.page.url = new URL('http://localhost/activity');
		render(AppHeader);

		expect(screen.getByRole('link', { name: 'Activity' }).getAttribute('aria-current')).toBe(
			'page'
		);
		expect(screen.getByRole('link', { name: 'Galleries' }).getAttribute('aria-current')).toBeNull();
	});

	it('marks Contacts as active when viewing /contacts', () => {
		state.page.url = new URL('http://localhost/contacts');
		render(AppHeader);

		expect(screen.getByRole('link', { name: 'Contacts' }).getAttribute('aria-current')).toBe(
			'page'
		);
		expect(screen.getByRole('link', { name: 'Galleries' }).getAttribute('aria-current')).toBeNull();
	});

	it('marks Photos as active when viewing /photos', () => {
		state.page.url = new URL('http://localhost/photos');
		render(AppHeader);

		expect(screen.getByRole('link', { name: 'Photos' }).getAttribute('aria-current')).toBe('page');
		expect(screen.getByRole('link', { name: 'Galleries' }).getAttribute('aria-current')).toBeNull();
	});

	it('exposes Settings and the account without duplicating the language control', () => {
		render(AppHeader);

		expect(screen.getByRole('link', { name: 'Settings' }).getAttribute('href')).toBe('/settings');
		expect(screen.queryByRole('button', { name: 'Language: English' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Account menu' })).toBeTruthy();
	});

	it('keeps Settings below Activity in the persistent rail', () => {
		render(AppHeader);

		const nav = screen.getByRole('navigation', { name: 'Main sections' });
		const links = [...nav.querySelectorAll('a')].map((link) => link.textContent?.trim());

		expect(links).toEqual(['Galleries', 'Photos', 'Contacts', 'Activity', 'Settings']);
	});

	it('uses the clamped desktop rail and compact phone navigation', () => {
		render(AppHeader);

		const header = screen.getByRole('banner');
		const nav = screen.getByRole('navigation', { name: 'Main sections' });

		expect(header.className).toContain('sm:w-(--gallery-rail-width)');
		expect(nav.className).toContain('flex-wrap');
		expect(nav.className).toContain('sm:flex-col');
	});

	it('shows a loaded studio name and signs out from the bottom identity', async () => {
		const onsignout = vi.fn();
		render(AppHeader, { onsignout, studioName: 'Atelier Lumiere' });

		expect(screen.getByText('Atelier Lumiere')).toBeTruthy();

		const user = userEvent.setup();
		await user.click(screen.getByRole('button', { name: 'Account menu' }));
		await user.click(screen.getByRole('button', { name: 'Sign out' }));

		expect(onsignout).toHaveBeenCalledOnce();
	});

	it('places the account menu safely on phones and above the identity on desktop', async () => {
		render(AppHeader);
		const user = userEvent.setup();

		await user.click(screen.getByRole('button', { name: 'Account menu' }));
		const panel = screen.getByText('Sign out').closest('[data-slot="popover-panel"]');

		expect(panel?.className).toContain('fixed');
		expect(panel?.className).toContain('right-3');
		expect(panel?.className).toContain('left-3');
		expect(panel?.className).toContain('sm:bottom-full');
	});

	it('calls onsignout when Sign out is pressed from the account menu', async () => {
		const onsignout = vi.fn();
		render(AppHeader, { onsignout });

		const user = userEvent.setup();

		expect(screen.queryByRole('button', { name: 'Sign out' })).toBeNull();
		await user.click(screen.getByRole('button', { name: 'Account menu' }));
		await user.click(screen.getByRole('button', { name: 'Sign out' }));

		expect(onsignout).toHaveBeenCalledOnce();
	});

	it('keeps the bell with the brand, above the workspace navigation', () => {
		render(AppHeader, { actions: bell });

		const brand = screen.getByRole('link', { name: 'PhotoProofing' });
		const notifications = screen.getByRole('button', { name: 'Notifications' });

		expect(brand.parentElement?.contains(notifications)).toBe(true);
		expect(brand.parentElement?.contains(screen.getByRole('navigation'))).toBe(false);
		expect(
			brand.parentElement?.contains(screen.getByRole('button', { name: 'Account menu' }))
		).toBe(false);
	});

	it('collapses into a compact row on small screens and a column rail on larger ones', () => {
		render(AppHeader);

		const nav = screen.getByRole('navigation', { name: 'Main sections' });

		expect(nav.className).toContain('flex-wrap');
		expect(nav.className).toContain('sm:flex-col');
	});

	it('shows the gallery route in the same persistent rail', () => {
		state.page.url = new URL('http://localhost/dashboard/gallery-id');
		render(AppHeader);

		const header = screen.getByRole('banner');
		const nav = screen.getByRole('navigation', { name: 'Main sections' });

		expect(header.className).toContain('sm:fixed');
		expect(nav.className).toContain('sm:flex-col');
		expect(screen.getByRole('link', { name: 'Galleries' }).getAttribute('aria-current')).toBe(
			'page'
		);
	});

	it('has no axe violations', async () => {
		const { container } = render(AppHeader);

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('AppHeader in French', () => {
	useLocale('fr');

	it('translates the rail links', async () => {
		const { container } = render(AppHeader);

		expect(screen.getByRole('link', { name: 'Galeries' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Photos' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Contacts' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Activité' }).getAttribute('href')).toBe('/activity');
		expect((await axe(container)).violations).toEqual([]);
	});
});
