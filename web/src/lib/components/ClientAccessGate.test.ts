import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useLocale } from '#lib/i18n/testing.js';
import ClientAccessGate from './ClientAccessGate.svelte';

const lockedGallery = {
	locked: true as const,
	title: 'Camille & Hugo',
	shoot_date: '2026-06-14',
	has_cover: true,
	studio_name: 'Studio Lumière',
	signature: 'name' as const,
	signature_size: 4,
	has_logo: false
};

describe('ClientAccessGate (English)', () => {
	useLocale('en');
	const fetcher = vi.fn();

	beforeEach(() => {
		fetcher.mockReset();
		vi.stubGlobal('fetch', fetcher);
	});
	afterEach(() => vi.unstubAllGlobals());

	it('renders the darkened cover, studio signature, title, formatted shoot date, and passes axe', async () => {
		const { container, rerender } = render(ClientAccessGate, {
			token: 'tok',
			gallery: lockedGallery,
			coverUrl: '/api/v1/galleries/shared/tok/cover',
			onunlock: vi.fn()
		});

		expect(screen.getByTestId('studio-signature').textContent).toContain('Studio Lumière');
		expect(screen.getByRole('heading', { level: 1, name: 'Camille & Hugo' })).toBeTruthy();
		expect(screen.getByText(/June 14, 2026/)).toBeTruthy();

		const img = screen.getByRole('img', { name: 'Gallery cover' });
		expect(img.getAttribute('src')).toBe('/api/v1/galleries/shared/tok/cover');
		expect(img.className).toContain('brightness-[0.38]');
		expect((await axe(container)).violations).toEqual([]);

		await rerender({
			token: 'tok',
			gallery: { ...lockedGallery, has_cover: false },
			coverUrl: undefined,
			onunlock: vi.fn()
		});
		expect(screen.getByTestId('no-hero')).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('submits a 6-digit code to /unlock, invokes onunlock on 204, and shows inline errors on 401 and 429', async () => {
		const user = userEvent.setup();
		const onunlock = vi.fn();
		render(ClientAccessGate, {
			token: 'tok',
			gallery: lockedGallery,
			coverUrl: '/api/v1/galleries/shared/tok/cover',
			onunlock
		});

		const input = screen.getByLabelText('Access code') as HTMLInputElement;
		expect(input.getAttribute('inputmode')).toBe('numeric');
		expect(input.getAttribute('maxlength')).toBe('6');
		expect(input.getAttribute('autocomplete')).toBe('one-time-code');

		await user.type(input, '123');
		await user.click(screen.getByRole('button', { name: 'Enter' }));
		expect(screen.getByRole('alert').textContent).toContain('Enter all 6 digits.');
		expect(fetcher).not.toHaveBeenCalled();

		fetcher.mockResolvedValueOnce(Response.json({ code: 'invalid_access_code' }, { status: 401 }));
		await user.clear(input);
		await user.type(input, '000000');
		await user.click(screen.getByRole('button', { name: 'Enter' }));
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/shared/tok/unlock',
			expect.objectContaining({
				method: 'POST',
				credentials: 'same-origin',
				body: JSON.stringify({ code: '000000' })
			})
		);
		expect(screen.getByRole('alert').textContent).toContain(
			'Incorrect access code. Please try again.'
		);
		expect(onunlock).not.toHaveBeenCalled();

		fetcher.mockResolvedValueOnce(Response.json({ code: 'too_many_attempts' }, { status: 429 }));
		await user.click(screen.getByRole('button', { name: 'Enter' }));
		expect(screen.getByRole('alert').textContent).toContain(
			'Too many attempts. Please wait a moment and try again.'
		);

		fetcher.mockResolvedValueOnce(new Response(null, { status: 204 }));
		await user.clear(input);
		await user.type(input, '042819');
		await user.click(screen.getByRole('button', { name: 'Enter' }));
		expect(onunlock).toHaveBeenCalledOnce();
	});
});

describe('ClientAccessGate (French)', () => {
	useLocale('fr');

	it('renders localized French labels and passes axe', async () => {
		const { container } = render(ClientAccessGate, {
			token: 'tok',
			gallery: lockedGallery,
			coverUrl: undefined,
			onunlock: vi.fn()
		});

		expect(screen.getByLabelText('Code d’accès')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Entrer' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});
});
