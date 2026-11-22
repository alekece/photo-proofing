import { render, screen, waitFor } from '@testing-library/svelte';
import { axe } from 'vitest-axe';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';

const state = vi.hoisted(() => ({
	page: { status: 500, error: { message: 'boom' }, url: new URL('http://localhost/') }
}));
vi.mock('$app/state', () => state);
import ErrorPage from './+error.svelte';

function show(status: number, path: string) {
	state.page.status = status;
	state.page.error = { message: 'internal detail' };
	state.page.url = new URL(path, 'http://localhost');

	return render(ErrorPage);
}

describe('error page', () => {
	beforeEach(() => document.body.replaceChildren());

	it('says a missing page was not found', () => {
		show(404, '/nowhere');

		expect(
			screen.getByRole('heading', { level: 1, name: /couldn.t find this page/i })
		).toBeTruthy();
	});

	it.each([500, 502, 503])('reads differently for a %i', (status) => {
		show(status, '/nowhere');

		expect(screen.getByRole('heading', { level: 1, name: /something went wrong/i })).toBeTruthy();
		expect(screen.queryByText(/couldn.t find/i)).toBeNull();
	});

	it('does not blame the server for a client error', () => {
		show(403, '/nowhere');

		expect(screen.getByRole('heading', { level: 1, name: /can.t show this page/i })).toBeTruthy();
		expect(screen.queryByText(/our side/i)).toBeNull();
	});

	it('offers no retry for a missing client link', () => {
		show(404, '/g/token123');

		expect(screen.queryByRole('link')).toBeNull();
		expect(screen.getByText(/ask your photographer/i)).toBeTruthy();
	});

	it.each([
		'/settings',
		'/dashboard',
		'/dashboard/abc?photo=p2&kind=final',
		'/photos',
		'/activity',
		'/contacts'
	])('offers a fresh load of %s after a studio outage', (path) => {
		show(503, path);

		const retry = screen.getByRole('link', { name: 'Try again' });
		expect(retry.getAttribute('href')).toBe(path);
		expect(retry.hasAttribute('data-sveltekit-reload')).toBe(true);
		expect(screen.queryByText(/your photographer/i)).toBeNull();
		expect(screen.queryByRole('link', { name: 'Go to sign in' })).toBeNull();
	});

	it('never shows the raw error text', () => {
		show(500, '/dashboard');

		expect(screen.queryByText(/internal detail/)).toBeNull();
	});

	it('moves focus to the heading', async () => {
		show(500, '/dashboard');

		await waitFor(() =>
			expect(document.activeElement).toBe(screen.getByRole('heading', { level: 1 }))
		);
	});

	it.each([
		['/g/token123', 'Try again', '/g/token123'],
		['/nowhere', 'Go to sign in', '/login']
	])('from %s links to the safe place', (path, label, href) => {
		show(500, path);

		expect(screen.getByRole('link', { name: label }).getAttribute('href')).toBe(href);
	});

	it('has no accessibility violations', async () => {
		const { container } = show(404, '/nowhere');

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('error page in French', () => {
	useLocale('fr');
	beforeEach(() => document.body.replaceChildren());

	it('speaks the workspace language', () => {
		show(404, '/dashboard/abc');

		expect(
			screen.getByRole('heading', { level: 1, name: 'Nous n’avons pas trouvé cette page.' })
		).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Retour à vos galeries' })).toBeTruthy();
		expect(screen.queryByText(/couldn.t find/i)).toBeNull();
	});

	it('offers recovery to the photographer in French', () => {
		show(503, '/settings');
		expect(screen.getByRole('link', { name: 'Réessayer' }).getAttribute('href')).toBe('/settings');
		expect(screen.queryByText(/votre photographe/i)).toBeNull();
	});
});
