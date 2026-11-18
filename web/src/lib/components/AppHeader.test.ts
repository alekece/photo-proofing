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

		expect(names).toEqual(['Galleries', 'All Photos', 'Contacts', 'Activity']);

		const galleries = screen.getByRole('link', { name: 'Galleries' });
		const allPhotos = screen.getByRole('link', { name: 'All Photos' });
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

	it('marks All Photos as active when viewing /photos', () => {
		state.page.url = new URL('http://localhost/photos');
		render(AppHeader);

		expect(screen.getByRole('link', { name: 'All Photos' }).getAttribute('aria-current')).toBe(
			'page'
		);
		expect(screen.getByRole('link', { name: 'Galleries' }).getAttribute('aria-current')).toBeNull();
	});

	it('exposes direct Settings, language and account controls in the rail', () => {
		render(AppHeader);

		expect(screen.getByRole('link', { name: 'Settings' }).getAttribute('href')).toBe('/settings');
		expect(screen.getByRole('button', { name: 'Language: English' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Account menu' })).toBeTruthy();
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

	it('shows what the page puts beside the account menu, such as the bell', () => {
		render(AppHeader, { actions: bell });

		expect(screen.getByRole('button', { name: 'Notifications' })).toBeTruthy();
	});

	it('collapses into a compact row on small screens and a column rail on larger ones', () => {
		render(AppHeader);

		const nav = screen.getByRole('navigation', { name: 'Main sections' });

		expect(nav.className).toContain('flex-wrap');
		expect(nav.className).toContain('sm:flex-col');
	});

	it('keeps the legacy compact bar for gallery pages pending the gallery-specific rail', () => {
		render(AppHeader, { legacy: true });

		const nav = screen.getByRole('navigation', { name: 'Main sections' });

		expect(nav.className).not.toContain('sm:flex-col');
		expect(screen.queryByRole('link', { name: 'All Photos' })).toBeNull();
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
		expect(screen.getByRole('link', { name: 'Toutes les photos' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Contacts' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Activité' }).getAttribute('href')).toBe('/activity');
		expect((await axe(container)).violations).toEqual([]);
	});
});
