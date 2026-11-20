import { act, fireEvent, render, screen } from '@testing-library/svelte';
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
	signature_size: 4,
	proof_quality: 'medium',
	watermark_opacity: 50,
	watermark_text_color: '#FFFFFF',
	has_logo: false
};

function respond(body: unknown, status = 200) {
	const fetcher = vi.fn(async () => new Response(JSON.stringify(body), { status }));

	vi.stubGlobal('fetch', fetcher);

	return fetcher;
}

afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

function open(settings = data.settings) {
	return render(Page, { props: { data: { ...data, settings } } as never });
}

function tab(name: string) {
	return screen.getByRole('tab', { name });
}

function proofPreview() {
	return screen.getByRole('img', { name: 'Preview of a proof with the watermark' });
}

describe('Settings page', () => {
	it('keeps a single preview beside the selected settings section', async () => {
		open();
		const user = userEvent.setup();
		expect(screen.getAllByRole('tabpanel')).toHaveLength(1);
		expect(proofPreview()).toBeTruthy();

		tab('Studio').focus();
		await user.keyboard('{End}');
		expect(tab('Proof quality')).toBe(document.activeElement);
		expect(tab('Proof quality').getAttribute('aria-selected')).toBe('true');
		expect(proofPreview()).toBeTruthy();
		await user.keyboard('{ArrowLeft}');
		expect(tab('Cover signature')).toBe(document.activeElement);
		expect(
			screen.getByRole('img', { name: 'Preview of a client cover with the signature' })
		).toBeTruthy();
		await user.keyboard('{Home}{ArrowRight}');
		expect(tab('Watermark')).toBe(document.activeElement);
		expect(proofPreview()).toBeTruthy();
		expect(screen.getAllByRole('tabpanel')).toHaveLength(1);
	});

	it('previews and validates color and opacity', async () => {
		const { container } = open();
		const user = userEvent.setup();
		await user.click(tab('Watermark'));
		const color = container.querySelector<HTMLInputElement>('input[type=color]')!;
		const hex = screen.getByLabelText('Hex color') as HTMLInputElement;
		const opacity = screen.getByRole('slider', { name: 'Watermark opacity' });

		await fireEvent.input(color, { target: { value: '#a12b3c' } });
		expect(hex.value).toBe('#A12B3C');
		await fireEvent.input(hex, { target: { value: '#b02040' } });
		expect(color.value).toBe('#b02040');
		await fireEvent.input(hex, { target: { value: '#a12b3c' } });
		for (const pattern of ['Repeat', 'Once']) {
			await user.click(screen.getByRole('button', { name: pattern }));
			const stamp = () =>
				[...proofPreview().querySelectorAll('span')].find(
					(element) => element.style.color === 'rgb(161, 43, 60)'
				);
			expect(stamp()).toBeTruthy();
			await fireEvent.input(opacity, { target: { value: '0' } });
			expect(proofPreview().querySelector<HTMLElement>('[style*="opacity"]')?.style.opacity).toBe(
				'0'
			);
			await fireEvent.input(opacity, { target: { value: '100' } });
			expect(proofPreview().querySelector<HTMLElement>('[style*="opacity"]')?.style.opacity).toBe(
				'1'
			);
		}

		await fireEvent.input(hex, { target: { value: '#a12' } });
		expect(hex.getAttribute('aria-invalid')).toBe('true');
		expect(color.value).toBe('#a12b3c');
		expect(screen.getByRole('button', { name: 'Save changes' })).toHaveProperty('disabled', true);
		await user.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(hex.value).toBe('#FFFFFF');
		expect(color.value).toBe('#ffffff');
		expect(opacity).toHaveProperty('value', '50');
	});

	it('drops an invalid hex buffer, but keeps the last valid color on a logo-only mark', async () => {
		open({ ...DEFAULT_SETTINGS, logo: '/api/v1/settings/logo' });
		const user = userEvent.setup();
		await user.click(tab('Watermark'));
		const hex = screen.getByLabelText('Hex color') as HTMLInputElement;
		await fireEvent.input(hex, { target: { value: '#A12B3C' } });
		await fireEvent.input(hex, { target: { value: '#A12' } });
		await user.click(screen.getByRole('button', { name: 'Logo' }));
		expect(hex.matches(':disabled')).toBe(true);
		expect(screen.getByRole('button', { name: 'Save changes' })).toHaveProperty('disabled', false);
		await user.click(screen.getByRole('button', { name: 'Name' }));
		expect(hex.value).toBe('#A12B3C');
	});

	it('links back to the galleries', () => {
		open();

		expect(screen.getByRole('link', { name: 'Back to galleries' }).getAttribute('href')).toBe(
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
		expect(pending.hasAttribute('disabled')).toBe(false);
		expect(pending.getAttribute('aria-disabled')).toBe('true');
		await user.click(pending);
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
		const fetcher = vi.fn((url: string) =>
			url.includes('/auth/')
				? Promise.resolve(new Response(null, { status: 204 }))
				: new Promise<Response>((resolve) => (finish = resolve))
		);
		vi.stubGlobal('fetch', fetcher);
		open();
		const user = userEvent.setup();
		const name = screen.getByLabelText('Studio name');

		await user.type(name, 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Save changes' }));
		await user.type(name, ' II');
		await user.click(tab('Watermark'));
		await fireEvent.input(screen.getByRole('slider', { name: 'Watermark opacity' }), {
			target: { value: '71' }
		});
		await fireEvent.input(screen.getByLabelText('Hex color'), { target: { value: '#a12' } });
		await user.click(tab('Cover signature'));
		await user.click(screen.getByRole('button', { name: 'None' }));
		finish(Response.json(saved));

		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Save changes' })).toHaveProperty('disabled', true)
		);
		expect(screen.getByText('Enter a six-digit hex color.')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Saved' })).toBeNull();
		expect(screen.getByRole('button', { name: 'None' }).getAttribute('aria-pressed')).toBe('true');
		await user.click(tab('Studio'));
		expect(screen.getByLabelText('Studio name')).toHaveProperty('value', 'Studio X II');
		await user.click(tab('Watermark'));
		expect(screen.getByRole('slider', { name: 'Watermark opacity' })).toHaveProperty('value', '71');
		const hex = screen.getByLabelText('Hex color');
		expect(hex).toHaveProperty('value', '#a12');
		await fireEvent.input(hex, { target: { value: '#a12b3c' } });
		expect(screen.getByRole('button', { name: 'Save changes' })).toHaveProperty('disabled', false);
		expect(fetcher.mock.calls.filter(([url]) => url === '/api/v1/settings')).toHaveLength(1);
	});

	it('saves and confirms on the button only, without any toast', async () => {
		toasts.items = [];
		const fetcher = respond(saved);
		open();
		const user = userEvent.setup();

		await user.type(screen.getByLabelText('Studio name'), 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		await vi.waitFor(() =>
			expect(
				screen.getAllByRole('status').some((live) => live.textContent?.trim() === 'Saved')
			).toBe(true)
		);
		expect(screen.getByTestId('click-ripple')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Saved' })).toHaveProperty('disabled', false);
		expect(toasts.items.map((toast) => toast.message)).not.toContain('Saved');
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
		expect(toasts.items).toEqual([]);
	});

	it('returns Saved and Reverted to the disabled idle actions after their brief confirmations', async () => {
		vi.useFakeTimers();
		respond(saved);
		open();
		const name = screen.getByLabelText('Studio name');
		await fireEvent.input(name, { target: { value: 'Studio X' } });
		await fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));
		await act(() => vi.advanceTimersByTimeAsync(0));
		await act(() => vi.advanceTimersByTimeAsync(1599));
		expect(screen.getByRole('button', { name: 'Saved' })).toBeTruthy();
		await act(() => vi.advanceTimersByTimeAsync(1));
		expect(screen.getByRole('button', { name: 'Save changes' })).toHaveProperty('disabled', true);

		await fireEvent.input(name, { target: { value: 'New draft' } });
		await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		await act(() => vi.advanceTimersByTimeAsync(1399));
		expect(screen.getByRole('button', { name: 'Reverted' })).toBeTruthy();
		await act(() => vi.advanceTimersByTimeAsync(1));
		expect(screen.getByRole('button', { name: 'Cancel' })).toHaveProperty('disabled', true);
	});

	it('does not announce a regeneration for a signature-only change', async () => {
		toasts.items = [];
		respond({ ...saved, studio_name: '', signature: 'none' });
		open();
		const user = userEvent.setup();

		await user.click(tab('Cover signature'));
		await user.click(screen.getByRole('button', { name: 'None' }));
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		expect(toasts.items).toEqual([]);
	});

	it('saves the proof quality without a toast', async () => {
		toasts.items = [];
		const fetcher = respond({ ...saved, proof_quality: 'high' });

		open();
		const user = userEvent.setup();

		await user.click(tab('Proof quality'));
		expect(screen.getByRole('radio', { name: /Medium/ }).getAttribute('aria-checked')).toBe('true');
		await user.click(screen.getByRole('radio', { name: /High/ }));
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		await screen.findByRole('button', { name: 'Saved' });
		expect(toasts.items).toEqual([]);
		expect(
			JSON.parse(
				(fetcher.mock.calls as unknown as [string, RequestInit][]).find(
					([url]) => url === '/api/v1/settings'
				)![1].body as string
			)
		).toMatchObject({ proof_quality: 'high' });
	});

	it('shows the reason the server refused the settings', async () => {
		toasts.items = [];
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
		expect(screen.queryByTestId('click-ripple')).toBeNull();
		expect(screen.getByRole('button', { name: 'Save changes' })).toBeTruthy();
	});

	it('refuses a logo that is not a PNG or JPEG before uploading it and leaves its size to the server', async () => {
		toasts.items = [];
		const { container } = open();
		const user = userEvent.setup({ applyAccept: false });
		const input = container.querySelector('input[type=file]') as HTMLInputElement;

		await user.upload(input, new File(['x'], 'logo.gif', { type: 'image/gif' }));

		expect(toasts.items.map((toast) => toast.message)).toEqual([
			'The logo must be a PNG or JPEG image.'
		]);
		expect(screen.queryByAltText('Studio logo')).toBeNull();

		await user.upload(
			input,
			new File([new Uint8Array(2 * 1024 * 1024)], 'big.png', { type: 'image/png' })
		);

		expect(await screen.findByAltText('Studio logo')).toBeTruthy();
	});

	it('reverts every unsaved change on Cancel', async () => {
		open();
		const user = userEvent.setup();
		const name = screen.getByLabelText('Studio name') as HTMLInputElement;

		await user.type(name, 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Cancel' }));

		expect(name.value).toBe('');

		const reverted = await screen.findByRole('button', { name: /Reverted/ });

		expect(reverted).toHaveProperty('disabled', false);
		expect(reverted.getAttribute('aria-disabled')).toBe('true');
		expect(screen.getByTestId('click-ripple')).toBeTruthy();
		expect(document.body.innerHTML).not.toMatch(/translate-y/);
		expect(screen.getAllByRole('status').some((live) => live.textContent === 'Reverted')).toBe(
			true
		);
	});

	it('disables logo options and the position grid until they apply', async () => {
		open();
		const user = userEvent.setup();
		await user.click(tab('Cover signature'));

		for (const name of ['Logo', 'Both'])
			for (const button of screen.getAllByRole('button', { name }))
				expect(button.hasAttribute('disabled')).toBe(true);
		await user.click(tab('Watermark'));
		expect(screen.getByRole('button', { name: 'Position 1' }).closest('fieldset')?.disabled).toBe(
			true
		);

		await user.click(screen.getByRole('button', { name: 'Once' }));

		expect(screen.getByRole('button', { name: 'Position 1' }).closest('fieldset')?.disabled).toBe(
			false
		);
	});

	it('has no accessibility violations', async () => {
		const { container } = open();
		await userEvent.setup().click(tab('Watermark'));

		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('Settings page in French', () => {
	useLocale('fr');

	it('shows headings and controls in French', async () => {
		open();

		expect(screen.getByRole('heading', { level: 1, name: 'Réglages' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Enregistrer' })).toBeTruthy();
		expect(screen.queryByText('Save changes')).toBeNull();
		await userEvent.setup().click(tab('Filigrane'));
		expect(screen.getByLabelText('Couleur du texte')).toBeTruthy();
		expect(screen.getByLabelText('Code hexadécimal')).toBeTruthy();
		expect(screen.getByRole('slider', { name: 'Opacité du filigrane' })).toBeTruthy();
	});
});
