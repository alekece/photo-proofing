import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';

vi.mock('$app/navigation', () => ({ goto: vi.fn(), afterNavigate: vi.fn() }));
import Page from './+page.svelte';
import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
import { DEFAULT_SETTINGS } from '#lib/settings/settings.js';

const data = { idleTimeoutMs: 60000, settings: DEFAULT_SETTINGS };

const saved = {
	studio_name: 'Studio X',
	mark: 'name',
	pattern: 'repeat',
	position: 8,
	size: 2,
	signature: 'name',
	has_logo: false
};

function respond(body: unknown, status = 200) {
	const fetcher = vi.fn(async () => new Response(JSON.stringify(body), { status }));

	vi.stubGlobal('fetch', fetcher);

	return fetcher;
}

afterEach(() => vi.unstubAllGlobals());

function open() {
	return render(Page, { props: { data } as never });
}

describe('Settings page', () => {
	it('links back to the galleries', () => {
		open();

		expect(screen.getByRole('link', { name: '← Back to galleries' }).getAttribute('href')).toBe(
			'/dashboard'
		);
	});

	it('shows Saving and prevents another submission while saving', async () => {
		let finish!: (response: Response) => void;
		const fetcher = vi.fn((url: string) =>
			url.includes('/auth/')
				? Promise.resolve(new Response(null, { status: 204 }))
				: new Promise<Response>((resolve) => (finish = resolve))
		);
		vi.stubGlobal('fetch', fetcher);
		open();
		const user = userEvent.setup();

		await user.type(screen.getByLabelText('Studio name'), 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Save changes' }));
		const pending = screen.getByRole('button', { name: /Saving/ });
		expect(pending.hasAttribute('disabled')).toBe(true);
		await user.click(pending);
		expect(fetcher.mock.calls.filter(([url]) => url === '/api/v1/settings')).toHaveLength(1);
		finish(Response.json(saved));
		await vi.waitFor(() => expect(screen.queryByRole('button', { name: /Saving/ })).toBeNull());
	});

	it('keeps Save and Cancel disabled until something changes', async () => {
		open();
		const user = userEvent.setup();

		expect(screen.getByRole('button', { name: 'Save changes' }).hasAttribute('disabled')).toBe(
			true
		);
		expect(screen.getByRole('button', { name: 'Cancel' }).hasAttribute('disabled')).toBe(true);

		await user.type(screen.getByLabelText('Studio name'), 'Studio X');

		expect(screen.getByRole('button', { name: 'Save changes' }).hasAttribute('disabled')).toBe(
			false
		);
	});

	it('preserves settings edited after a save started', async () => {
		let finish!: (response: Response) => void;
		vi.stubGlobal('fetch', (url: string) =>
			url.includes('/auth/')
				? Promise.resolve(new Response(null, { status: 204 }))
				: new Promise<Response>((resolve) => (finish = resolve))
		);
		open();
		const user = userEvent.setup();
		const name = screen.getByLabelText('Studio name');

		await user.type(name, 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Save changes' }));
		await user.type(name, ' II');
		await user.click(screen.getByRole('radio', { name: 'None' }));
		finish(Response.json(saved));

		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Save changes' })).toHaveProperty('disabled', false)
		);
		expect(name).toHaveProperty('value', 'Studio X II');
		expect(screen.getByRole('radio', { name: 'None' }).getAttribute('aria-checked')).toBe('true');
	});

	it('saves, queues the Saved toast and announces the regeneration', async () => {
		toasts.items = [];
		const savedToast = vi.spyOn(toasts, 'saved');
		const fetcher = respond(saved);
		open();
		const user = userEvent.setup();

		await user.type(screen.getByLabelText('Studio name'), 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		await vi.waitFor(() => expect(savedToast).toHaveBeenCalledOnce());
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/settings',
			expect.objectContaining({ method: 'PUT' })
		);
		expect(
			JSON.parse(
				(fetcher.mock.calls as unknown as [string, RequestInit][]).find(
					([url]) => url === '/api/v1/settings'
				)![1].body as string
			)
		).toMatchObject({
			studio_name: 'Studio X',
			logo: { action: 'keep' }
		});
		expect(toasts.items.map((toast) => toast.message)).toContain(
			'Watermarked proofs are being regenerated'
		);
	});

	it('does not announce a regeneration for a signature-only change', async () => {
		toasts.items = [];
		respond({ ...saved, studio_name: '', signature: 'none' });
		open();
		const user = userEvent.setup();

		await user.click(screen.getByRole('radio', { name: 'None' }));
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		expect(toasts.items).toEqual([]);
	});

	it('shows the reason the server refused the settings', async () => {
		toasts.items = [];
		const savedToast = vi.spyOn(toasts, 'saved');
		respond({ code: 'invalid_studio_name' }, 422);
		open();
		const user = userEvent.setup();

		await user.type(screen.getByLabelText('Studio name'), 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		await vi.waitFor(() =>
			expect(toasts.items.map((toast) => toast.message)).toContain(
				'Enter a studio name of up to 100 characters.'
			)
		);
		expect(savedToast).not.toHaveBeenCalled();
	});

	it('refuses a logo that is not a PNG or JPEG or is too large before uploading it', async () => {
		toasts.items = [];
		const { container } = open();
		const user = userEvent.setup({ applyAccept: false });
		const input = container.querySelector('input[type=file]') as HTMLInputElement;

		await user.upload(input, new File(['x'], 'logo.gif', { type: 'image/gif' }));
		await user.upload(
			input,
			new File([new Uint8Array(1024 * 1024 + 1)], 'big.png', { type: 'image/png' })
		);

		expect(toasts.items.map((toast) => toast.message)).toEqual([
			'The logo must be a PNG or JPEG image.',
			'The logo must be under 1 MB.'
		]);
		expect(screen.queryByAltText('Studio logo')).toBeNull();
	});

	it('reverts every unsaved change on Cancel', async () => {
		open();
		const user = userEvent.setup();
		const name = screen.getByLabelText('Studio name') as HTMLInputElement;

		await user.type(name, 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Cancel' }));

		expect(name.value).toBe('');
		expect(await screen.findByRole('button', { name: /Reverted/ })).toBeTruthy();
	});

	it('disables logo options and the position grid until they apply', async () => {
		open();

		expect(screen.getByRole('radio', { name: 'Logo' }).hasAttribute('disabled')).toBe(true);
		expect(screen.getByRole('radio', { name: 'Both' }).hasAttribute('disabled')).toBe(true);
		expect(screen.getByRole('button', { name: 'Logo' }).hasAttribute('disabled')).toBe(true);
		expect(screen.getByRole('button', { name: 'Position 1' }).closest('fieldset')?.disabled).toBe(
			true
		);

		await userEvent.setup().click(screen.getByRole('button', { name: 'Once' }));

		expect(screen.getByRole('button', { name: 'Position 1' }).closest('fieldset')?.disabled).toBe(
			false
		);
	});

	it('has no accessibility violations', async () => {
		const { container } = open();

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('Settings page in French', () => {
	useLocale('fr');

	it('shows the heading and the save button in French', () => {
		open();

		expect(screen.getByRole('heading', { level: 1, name: 'Réglages' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Enregistrer' })).toBeTruthy();
		expect(screen.queryByText('Save changes')).toBeNull();
	});
});
