import { fireEvent, render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';

vi.mock('$app/navigation', () => ({
	goto: vi.fn(),
	afterNavigate: vi.fn(),
	invalidateAll: vi.fn()
}));
const setLocale = vi.hoisted(() => vi.fn());
vi.mock('#lib/paraglide/runtime.js', async (original) => {
	const runtime = await original<typeof import('#lib/paraglide/runtime.js')>();

	return { ...runtime, getLocale: () => runtime.getLocale(), setLocale };
});
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
	setLocale.mockClear();
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
	it('keeps presentation and theme choices in the draft and resets both on cancel', async () => {
		open();
		const user = userEvent.setup();
		const save = screen.getByRole('button', { name: 'Save' });
		await user.click(screen.getByRole('radio', { name: 'Print' }));
		await user.click(screen.getByRole('radio', { name: 'Neutral board' }));
		expect(save).toHaveProperty('disabled', false);
		await user.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(screen.getByRole('radio', { name: 'Full image' })).toHaveProperty('checked', true);
		expect(screen.getByRole('radio', { name: 'Warm board' })).toHaveProperty('checked', true);
		expect(save).toHaveProperty('disabled', true);
	});

	it('puts language and proof quality in General and reserves previews for the editors', async () => {
		open();
		const user = userEvent.setup();
		expect(screen.getAllByRole('tabpanel')).toHaveLength(1);
		expect(screen.queryByRole('img', { name: /Preview of/ })).toBeNull();
		expect(screen.getByRole('radio', { name: 'English', checked: true })).toBeTruthy();
		expect(screen.getByRole('radio', { name: 'Français', checked: false })).toBeTruthy();
		expect(screen.getByRole('radio', { name: /Medium/ })).toBeTruthy();

		tab('General').focus();
		await user.keyboard('{End}');
		expect(tab('Cover signature')).toBe(document.activeElement);
		expect(
			screen.getByRole('img', { name: 'Preview of a client cover with the signature' })
		).toBeTruthy();
		await user.keyboard('{Home}{ArrowRight}');
		expect(tab('Watermark')).toBe(document.activeElement);
		expect(proofPreview()).toBeTruthy();
		expect(screen.getAllByRole('tabpanel')).toHaveLength(1);
	});

	it('keeps the language and watermark local until the same save', async () => {
		const fetcher = vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(async () =>
			Response.json({ ...saved, language: 'fr', watermark_opacity: 67 })
		);
		vi.stubGlobal('fetch', fetcher);
		open();
		const user = userEvent.setup();

		await user.click(screen.getByRole('radio', { name: 'Français' }));
		await user.click(tab('Watermark'));
		await fireEvent.input(screen.getByRole('slider', { name: 'Opacity' }), {
			target: { value: '67' }
		});
		const writes = () => fetcher.mock.calls.filter(([, init]) => init?.method === 'PUT');
		expect(writes()).toHaveLength(0);
		expect(setLocale).not.toHaveBeenCalled();
		await user.click(screen.getByRole('button', { name: 'Save' }));
		expect(writes()).toHaveLength(1);
		expect(writes()[0][0]).toBe('/api/v1/settings');
		expect(JSON.parse(writes()[0][1]?.body as string)).toMatchObject({
			language: 'fr',
			watermark_opacity: 67
		});
		await vi.waitFor(() => expect(setLocale).toHaveBeenCalledExactlyOnceWith('fr'));
	});

	it('previews text color and floors new opacity adjustments without changing a saved zero', async () => {
		const { container } = open({ ...DEFAULT_SETTINGS, watermarkOpacity: 0 });
		const user = userEvent.setup();
		await user.click(tab('Watermark'));
		const color = container.querySelector<HTMLInputElement>('input[type=color]')!;
		const hex = screen.getByLabelText('Hex color') as HTMLInputElement;
		const opacity = screen.getByRole('slider', { name: 'Opacity' });

		expect(proofPreview().querySelector<HTMLElement>('[style*="opacity"]')?.style.opacity).toBe(
			'0'
		);
		expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', true);

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
				'0.05'
			);
			await fireEvent.input(opacity, { target: { value: '100' } });
			expect(proofPreview().querySelector<HTMLElement>('[style*="opacity"]')?.style.opacity).toBe(
				'1'
			);
		}

		await fireEvent.input(hex, { target: { value: '#a12' } });
		expect(hex.getAttribute('aria-invalid')).toBe('true');
		expect(color.value).toBe('#a12b3c');
		expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', true);
		await user.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(hex.value).toBe('#FFFFFF');
		expect(color.value).toBe('#ffffff');
		expect(proofPreview().querySelector<HTMLElement>('[style*="opacity"]')?.style.opacity).toBe(
			'0'
		);
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
		expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', false);
		await user.click(screen.getByRole('button', { name: 'Name' }));
		expect(hex.value).toBe('#A12B3C');
	});

	it('keeps the language and other edits after a failed save', async () => {
		toasts.items = [];
		respond({}, 500);
		open();
		const user = userEvent.setup();

		await user.type(screen.getByLabelText('Studio name'), 'Studio X');
		await user.click(screen.getByRole('radio', { name: 'Français' }));
		await user.click(screen.getByRole('button', { name: 'Save' }));

		await vi.waitFor(() => expect(toasts.items).toHaveLength(1));
		expect(setLocale).not.toHaveBeenCalled();
		expect(screen.getByRole('radio', { name: 'Français', checked: true })).toBeTruthy();
		expect(screen.getByLabelText('Studio name')).toHaveProperty('value', 'Studio X');
		expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', false);
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
		await user.click(screen.getByRole('button', { name: 'Save' }));
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

		expect(screen.getByRole('button', { name: 'Save' }).hasAttribute('disabled')).toBe(true);
		expect(screen.getByRole('button', { name: 'Cancel' }).hasAttribute('disabled')).toBe(true);

		await user.type(screen.getByLabelText('Studio name'), 'Studio X');

		expect(screen.getByRole('button', { name: 'Save' }).hasAttribute('disabled')).toBe(false);
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
		await user.click(screen.getByRole('button', { name: 'Save' }));
		await user.type(name, ' II');
		await user.click(tab('Watermark'));
		await fireEvent.input(screen.getByRole('slider', { name: 'Opacity' }), {
			target: { value: '71' }
		});
		await fireEvent.input(screen.getByLabelText('Hex color'), { target: { value: '#a12' } });
		await user.click(tab('Cover signature'));
		await user.click(screen.getByRole('button', { name: 'None' }));
		finish(Response.json(saved));

		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', true)
		);
		expect(screen.getByText('Enter a six-digit hex color in Watermark.')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Saved' })).toBeNull();
		expect(screen.getByRole('button', { name: 'None' }).getAttribute('aria-pressed')).toBe('true');
		await user.click(tab('General'));
		expect(screen.getByLabelText('Studio name')).toHaveProperty('value', 'Studio X II');
		await user.click(tab('Watermark'));
		expect(screen.getByRole('slider', { name: 'Opacity' })).toHaveProperty('value', '71');
		const hex = screen.getByLabelText('Hex color');
		expect(hex).toHaveProperty('value', '#a12');
		await fireEvent.input(hex, { target: { value: '#a12b3c' } });
		expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', false);
		expect(fetcher.mock.calls.filter(([url]) => url === '/api/v1/settings')).toHaveLength(1);
	});

	it('saves and returns to the idle action without a visible confirmation', async () => {
		toasts.items = [];
		const fetcher = respond(saved);
		open();
		const user = userEvent.setup();

		await user.type(screen.getByLabelText('Studio name'), 'Studio X');
		await user.click(screen.getByRole('button', { name: 'Save' }));

		await vi.waitFor(() =>
			expect(
				screen.getAllByRole('status').some((live) => live.textContent?.trim() === 'Saved')
			).toBe(true)
		);
		expect(screen.queryByRole('button', { name: 'Saved' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', true);
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

	it('does not announce a regeneration for a signature-only change', async () => {
		toasts.items = [];
		respond({ ...saved, studio_name: '', signature: 'none' });
		open();
		const user = userEvent.setup();

		await user.click(tab('Cover signature'));
		await user.click(screen.getByRole('button', { name: 'None' }));
		await user.click(screen.getByRole('button', { name: 'Save' }));

		expect(toasts.items).toEqual([]);
	});

	it('saves a signature text color and inherits the cover color when the hex field is cleared', async () => {
		const fetcher = vi.fn(async (_url: string, init?: RequestInit) => {
			const body = JSON.parse(init?.body as string);

			return Response.json({ ...saved, signature_color: body.signature_color });
		});
		vi.stubGlobal('fetch', fetcher);
		open({ ...DEFAULT_SETTINGS, studioName: 'Studio X' });
		const user = userEvent.setup();

		await user.click(tab('Cover signature'));
		const color = screen.getByLabelText('Text color');
		const hex = screen.getByLabelText('Hex color');
		const name = screen.getByText('Studio X');
		expect(name.style.color).toBe('');
		await fireEvent.input(hex, { target: { value: '#a12b3c' } });
		expect(color).toHaveProperty('value', '#a12b3c');
		expect(name.style.color).toBe('rgb(161, 43, 60)');
		await fireEvent.input(hex, { target: { value: '#abc' } });
		expect(name.style.color).toBe('rgb(161, 43, 60)');
		expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', true);
		await user.click(tab('Watermark'));
		expect(screen.getByLabelText('Hex color')).toHaveProperty('value', '#FFFFFF');
		expect(screen.getByText('Enter a six-digit hex color in Cover signature.')).toBeTruthy();
		await user.click(tab('Cover signature'));
		expect(screen.getByLabelText('Hex color')).toHaveProperty('value', '#abc');
		await fireEvent.input(screen.getByLabelText('Hex color'), { target: { value: '#a12b3c' } });
		await user.click(screen.getByRole('button', { name: 'Save' }));
		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', true)
		);

		expect(screen.queryByRole('button', { name: 'Reset text color' })).toBeNull();
		await user.clear(screen.getByLabelText('Hex color'));
		expect(screen.getByText('Studio X').style.color).toBe('');
		expect(screen.getByLabelText('Hex color')).toHaveProperty('value', '');
		await user.click(screen.getByRole('button', { name: 'Save' }));
		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', true)
		);

		expect(
			fetcher.mock.calls
				.filter(([url]) => url === '/api/v1/settings')
				.map(([, init]) => JSON.parse(init?.body as string).signature_color)
		).toEqual(['#A12B3C', null]);
	});

	it('saves the proof quality without a toast', async () => {
		toasts.items = [];
		const fetcher = respond({ ...saved, proof_quality: 'high' });

		open();
		const user = userEvent.setup();

		expect(screen.getByRole('radio', { name: /Medium/ })).toHaveProperty('checked', true);
		await user.click(screen.getByRole('radio', { name: /High/ }));
		await user.click(screen.getByRole('button', { name: 'Save' }));

		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Save' })).toHaveProperty('disabled', true)
		);
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
		await user.click(screen.getByRole('button', { name: 'Save' }));

		await vi.waitFor(() =>
			expect(toasts.items.map((toast) => toast.message)).toContain(
				'Enter a studio name of up to 100 characters.'
			)
		);
		expect(screen.queryByTestId('click-ripple')).toBeNull();
		expect(screen.getByRole('button', { name: 'Save' })).toBeTruthy();
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
		const fetcher = respond(saved);
		open();
		const user = userEvent.setup();
		const name = screen.getByLabelText('Studio name') as HTMLInputElement;

		await user.type(name, 'Studio X');
		await user.click(screen.getByRole('radio', { name: 'Français' }));
		await user.click(tab('Watermark'));
		await user.click(screen.getByRole('button', { name: 'Once' }));
		await user.click(screen.getByRole('button', { name: 'Cancel' }));

		expect(screen.getByRole('button', { name: 'Repeat' }).getAttribute('aria-pressed')).toBe(
			'true'
		);
		await user.click(tab('General'));
		expect(screen.getByLabelText('Studio name')).toHaveProperty('value', '');

		expect(screen.getByRole('radio', { name: 'English', checked: true })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Reverted' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Cancel' })).toHaveProperty('disabled', true);
		expect(fetcher).not.toHaveBeenCalledWith(
			expect.stringContaining('/settings'),
			expect.objectContaining({ method: 'PUT' })
		);
		expect(screen.getAllByRole('status').some((live) => live.textContent === 'Reverted')).toBe(
			true
		);
	});

	it('hides watermark positions in Repeat and restores the selected cell in Once', async () => {
		open();
		const user = userEvent.setup();
		await user.click(tab('Watermark'));
		expect(screen.queryByRole('button', { name: 'Position 1' })).toBeNull();

		await user.click(screen.getByRole('button', { name: 'Once' }));
		await user.click(screen.getByRole('button', { name: 'Position 1' }));
		const selected = screen.getByRole('button', { name: 'Position 1', pressed: true });
		const inactive = screen.getByRole('button', { name: 'Position 2', pressed: false });
		expect(selected.querySelector('svg')?.innerHTML).toBe(inactive.querySelector('svg')?.innerHTML);
		await user.click(screen.getByRole('button', { name: 'Repeat' }));
		expect(screen.queryByRole('button', { name: 'Position 1' })).toBeNull();
		await user.click(screen.getByRole('button', { name: 'Once' }));
		expect(screen.getByRole('button', { name: 'Position 1' }).getAttribute('aria-pressed')).toBe(
			'true'
		);
	});

	it('does not offer a signature color for logo-only or hidden signatures', async () => {
		open({ ...DEFAULT_SETTINGS, logo: '/api/v1/settings/logo' });
		const user = userEvent.setup();

		await user.click(tab('Cover signature'));
		for (const name of ['Logo', 'None']) {
			await user.click(screen.getByRole('button', { name }));
			expect(screen.getByLabelText('Text color').matches(':disabled')).toBe(true);
		}
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
		expect(screen.queryByRole('button', { name: 'Save' })).toBeNull();
		await userEvent.setup().click(tab('Filigrane'));
		expect(screen.getByLabelText('Couleur du texte')).toBeTruthy();
		expect(screen.getByLabelText('Code hexadécimal')).toBeTruthy();
		expect(screen.getByRole('slider', { name: 'Opacité' })).toBeTruthy();
	});
});
