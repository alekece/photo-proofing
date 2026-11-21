import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Gallery } from '#lib/gallery/api.js';
import { toasts } from './ui/Toast/toasts.svelte.js';
import GallerySettingsPanel from './GallerySettingsPanel.svelte';

const gallery = {
	id: 'g1',
	title: 'Wedding',
	layout: 'grid',
	expires_at: null,
	shoot_date: null,
	included_photos: null,
	watermark_enabled: false,
	states: ['draft']
} as Gallery;

function setup(over: Partial<Gallery> = {}, saves = true, shareToken: string | null = null) {
	const onsave = vi.fn().mockResolvedValue(saves);
	const schedule = vi.fn();
	const cancel = vi.fn();
	const onsharelink = vi.fn();
	const onarchive = vi.fn();
	const onrestore = vi.fn();
	const ondelete = vi.fn();

	const view = render(GallerySettingsPanel, {
		gallery: { ...gallery, ...over },
		shareToken,
		origin: 'https://pp.test',
		onsave,
		schedule,
		cancel,
		onsharelink,
		onarchive,
		onrestore,
		ondelete
	});

	return {
		...view,
		onsave,
		schedule,
		cancel,
		onsharelink,
		onarchive,
		onrestore,
		ondelete,
		user: userEvent.setup()
	};
}

beforeEach(() => {
	vi.stubGlobal(
		'fetch',
		vi.fn(async () => Response.json([]))
	);
});

afterEach(() => {
	toasts.items = [];
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('GallerySettingsPanel', () => {
	it('shows an empty numeric allowance and does not request preview media', () => {
		setup({ hero_id: 'p1' });
		const allowance = screen.getByRole('spinbutton', { name: 'Included photos' });

		expect(allowance).toHaveProperty('value', '');
		expect(allowance.hasAttribute('placeholder')).toBe(false);
		expect(screen.queryByRole('region', { name: 'Client preview' })).toBeNull();
		expect(fetch).not.toHaveBeenCalled();
	});

	it('splits settings into General, Privacy & Protection, and Actions cards', async () => {
		const { container } = setup();

		expect(screen.getByRole('heading', { name: 'Settings' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'General' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Privacy & Protection' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Actions' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('orders General rows as Shoot date, Open until, Included photos', () => {
		setup();

		const labels = screen
			.getAllByText(/^(Shoot date|Open until|Included photos)$/)
			.map((node) => node.textContent);

		expect(labels).toEqual(['Shoot date', 'Open until', 'Included photos']);
	});

	it('keeps Archive and Delete as distinct lifecycle actions in Actions', () => {
		setup();

		const actionsCard = screen.getByRole('heading', { name: 'Actions' }).closest('section')!;
		const archiveButton = within(actionsCard).getByRole('button', { name: 'Archive' });
		const deleteButton = within(actionsCard).getByRole('button', { name: 'Delete' });

		expect(archiveButton).not.toBe(deleteButton);
		expect(
			archiveButton.compareDocumentPosition(deleteButton) & Node.DOCUMENT_POSITION_FOLLOWING
		).toBeTruthy();
	});

	it('says a draft is not shared, offers Share link, and reveals Copy once shared', async () => {
		const first = setup();

		expect(screen.getByText('Not shared yet')).toBeTruthy();
		await first.user.click(screen.getByRole('button', { name: 'Share link' }));
		expect(first.onsharelink).toHaveBeenCalledOnce();
		first.unmount();

		const archivedDraft = setup({ states: ['draft', 'archived'] });
		expect((screen.getByRole('button', { name: 'Share link' }) as HTMLButtonElement).disabled).toBe(
			true
		);
		archivedDraft.unmount();

		setup({}, true, 'A'.repeat(22));

		expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Share link' })).toBeNull();
		expect(screen.queryByRole('link', { name: 'Open' })).toBeNull();
	});

	it('toggles watermarks without flashing a success ring and reverts on failure', async () => {
		const ok = setup();

		await ok.user.click(screen.getByRole('switch', { name: 'Enable watermarks' }));
		expect(ok.onsave).toHaveBeenCalledWith({ watermark_enabled: true });
		await vi.waitFor(() =>
			expect(screen.getByRole('switch', { name: 'Enable watermarks' }).className).not.toContain(
				'ring-ok'
			)
		);
		ok.unmount();

		const fail = setup({}, false);
		const toggle = screen.getByRole('switch', { name: 'Enable watermarks' });

		await fail.user.click(toggle);
		await vi.waitFor(() => expect(toggle.getAttribute('aria-checked')).toBe('false'));
	});

	it('saves the picked open-until calendar date directly and clears it with null', async () => {
		const { onsave, user } = setup({ expires_at: '2030-06-15' });
		const trigger = screen.getByLabelText('Open until');

		expect(trigger.textContent).toContain('Jun 15, 2030');

		await user.click(trigger);
		await user.click(
			within(screen.getByRole('grid')).getByText('20', {
				selector: '[data-bits-day]:not([data-outside-month])'
			})
		);

		expect(onsave).toHaveBeenCalledWith({
			expires_at: '2030-06-20'
		});

		await user.click(trigger);
		await user.click(screen.getByRole('button', { name: 'Clear' }));

		expect(onsave).toHaveBeenLastCalledWith({ expires_at: null });
	});

	it('offers relative presets on Open until and omits them on Shoot date', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		vi.setSystemTime(new Date(2026, 5, 1, 12, 0, 0));

		const { onsave } = setup();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

		await user.click(screen.getByLabelText('Shoot date'));
		expect(screen.queryByRole('button', { name: 'In 2 weeks' })).toBeNull();

		await user.click(screen.getByLabelText('Open until'));
		await user.click(screen.getByRole('button', { name: 'In 2 weeks' }));

		expect(onsave).toHaveBeenCalledWith({
			expires_at: '2026-06-15'
		});
		expect(screen.queryByRole('grid')).toBeNull();
	});

	it('shows the shoot date and clears it with null', async () => {
		const { onsave, user } = setup({ shoot_date: '2026-06-14' });
		const trigger = screen.getByLabelText('Shoot date');

		expect(trigger.textContent).toContain('Jun 14, 2026');

		await user.click(trigger);
		await user.click(screen.getByRole('button', { name: 'Clear' }));

		expect(onsave).toHaveBeenCalledWith({ shoot_date: null });
	});

	it('debounces included photos through the page, disables when published, and reverts invalid input', async () => {
		const first = setup();

		await first.user.type(screen.getByLabelText('Included photos'), '5');
		expect(first.schedule).toHaveBeenCalledWith('included_photos', expect.any(Function));
		expect(first.onsave).not.toHaveBeenCalled();

		await first.user.tab();
		expect(first.cancel).toHaveBeenCalledWith('included_photos');
		expect(first.onsave).toHaveBeenCalledWith({ included_photos: 5 });
		first.unmount();

		const locked = setup({ included_photos: 3, states: ['delivered'] });
		expect((screen.getByLabelText('Included photos') as HTMLInputElement).disabled).toBe(true);
		locked.unmount();

		const invalid = setup({ included_photos: 3 });
		const input = screen.getByLabelText('Included photos') as HTMLInputElement;

		await invalid.user.clear(input);
		await invalid.user.type(input, '0');
		invalid.schedule.mock.calls.at(-1)![1]();
		expect(input.value).toBe('0');
		expect(invalid.onsave).not.toHaveBeenCalled();

		await invalid.user.tab();
		expect(invalid.onsave).not.toHaveBeenCalled();
		expect(input.value).toBe('3');
		expect(toasts.items).toHaveLength(1);
	});

	it('archives and deletes through the page, and restores an archived gallery', async () => {
		const first = setup();

		await first.user.click(screen.getByRole('button', { name: 'Archive' }));
		await first.user.click(screen.getByRole('button', { name: 'Delete' }));

		expect(first.onarchive).toHaveBeenCalledOnce();
		expect(first.ondelete).toHaveBeenCalledOnce();
		first.unmount();

		const second = setup({ states: ['draft', 'archived'] });

		expect(screen.queryByRole('button', { name: 'Archive' })).toBeNull();
		await second.user.click(screen.getByRole('button', { name: 'Restore' }));
		expect(second.onrestore).toHaveBeenCalledOnce();
	});

	it('generates, edits, copies, and clears a 6-digit access code in Draft and reverts on failure', async () => {
		const draft = setup({ access_code: null });
		const writeText = vi.fn().mockResolvedValue(undefined);
		Object.defineProperty(navigator, 'clipboard', {
			value: { writeText },
			configurable: true
		});
		const toggle = screen.getByRole('switch', { name: 'Require access code' });

		await draft.user.click(toggle);
		expect(draft.onsave).toHaveBeenCalledWith({
			access_code: expect.stringMatching(/^[0-9]{6}$/)
		});
		const input = screen.getByLabelText('6-digit access code') as HTMLInputElement;
		expect(input.value).toMatch(/^[0-9]{6}$/);

		await draft.user.click(screen.getByRole('button', { name: 'Generate' }));
		expect(draft.onsave).toHaveBeenCalledTimes(2);
		expect(draft.onsave.mock.calls[1]![0]).toEqual({
			access_code: expect.stringMatching(/^[0-9]{6}$/)
		});

		await draft.user.clear(input);
		await draft.user.type(input, '042819');
		await draft.user.tab();
		expect(draft.onsave).toHaveBeenLastCalledWith({ access_code: '042819' });

		await draft.user.click(screen.getByRole('button', { name: 'Copy' }));
		expect(writeText).toHaveBeenCalledWith('042819');

		expect((await axe(draft.container)).violations).toEqual([]);

		await draft.user.click(toggle);
		expect(draft.onsave).toHaveBeenLastCalledWith({ access_code: null });
		draft.unmount();

		const failing = setup({ access_code: null }, false);
		const failToggle = screen.getByRole('switch', { name: 'Require access code' });
		await failing.user.click(failToggle);
		await vi.waitFor(() => expect(failToggle.getAttribute('aria-checked')).toBe('false'));
	});

	it('locks access-code controls once shared, keeping Copy when set and disabling the switch when unset', async () => {
		const sharedWithCode = setup(
			{ access_code: '042819', states: ['shared'] },
			true,
			'A'.repeat(22)
		);
		const writeText = vi.fn().mockResolvedValue(undefined);
		Object.defineProperty(navigator, 'clipboard', {
			value: { writeText },
			configurable: true
		});
		const codeInput = screen.getByLabelText('6-digit access code') as HTMLInputElement;
		expect(codeInput.readOnly).toBe(true);
		expect(codeInput.value).toBe('042819');
		expect(screen.queryByRole('button', { name: 'Generate' })).toBeNull();
		expect(screen.getByText('Locked after sharing')).toBeTruthy();

		const copyButtons = screen.getAllByRole('button', { name: 'Copy' });
		await sharedWithCode.user.click(copyButtons[1]!);
		expect(writeText).toHaveBeenCalledWith('042819');

		expect((await axe(sharedWithCode.container)).violations).toEqual([]);
		sharedWithCode.unmount();

		const sharedWithoutCode = setup(
			{ access_code: null, states: ['shared'] },
			true,
			'A'.repeat(22)
		);
		const disabledSwitch = screen.getByRole('switch', { name: 'Require access code' });
		expect((disabledSwitch as HTMLButtonElement).disabled).toBe(true);
		expect(
			screen.getByText('Shared without an access code — cannot be added after sharing')
		).toBeTruthy();
		expect((await axe(sharedWithoutCode.container)).violations).toEqual([]);
	});

	it('serializes rapid Generate clicks and reverts partial drafts when Generate fails', async () => {
		let resolveFirst!: (ok: boolean) => void;
		let resolveSecond!: (ok: boolean) => void;
		let callCount = 0;

		const onsave = vi.fn().mockImplementation(() => {
			callCount += 1;
			if (callCount === 1) return Promise.resolve(true);
			if (callCount === 2) return new Promise<boolean>((r) => (resolveFirst = r));
			if (callCount === 3) return new Promise<boolean>((r) => (resolveSecond = r));
			return Promise.resolve(false);
		});

		const user = userEvent.setup();
		render(GallerySettingsPanel, {
			gallery: { ...gallery, access_code: null },
			shareToken: null,
			origin: 'https://pp.test',
			onsave,
			schedule: vi.fn(),
			cancel: vi.fn(),
			onarchive: vi.fn(),
			onrestore: vi.fn(),
			ondelete: vi.fn()
		});

		await user.click(screen.getByRole('switch', { name: 'Require access code' }));
		const input = screen.getByLabelText('6-digit access code') as HTMLInputElement;
		const generate = screen.getByRole('button', { name: 'Generate' });

		await user.click(generate);
		await user.click(generate);
		const secondCode = input.value;
		expect(onsave).toHaveBeenCalledTimes(2);

		resolveFirst(true);
		await vi.waitFor(() => expect(onsave).toHaveBeenCalledTimes(3));
		expect(onsave.mock.calls[2]![0]).toEqual({ access_code: secondCode });

		resolveSecond(true);
		await vi.waitFor(() => expect(input.value).toBe(secondCode));

		// Type partial unsaved draft '12', then fail Generate -> must revert to committed secondCode, not '12'.
		await user.clear(input);
		await user.type(input, '12');
		await user.click(generate);
		await vi.waitFor(() => expect(input.value).toBe(secondCode));
	});

	it('chains an access_code: null clear when toggled OFF while the enabling PATCH is still in flight', async () => {
		let resolveEnable!: (ok: boolean) => void;
		const calls: Array<string | null | undefined> = [];
		const onsave = vi.fn().mockImplementation((changes: { access_code?: string | null }) => {
			calls.push(changes.access_code);
			if (calls.length === 1) return new Promise<boolean>((r) => (resolveEnable = r));
			return Promise.resolve(true);
		});

		const user = userEvent.setup();
		render(GallerySettingsPanel, {
			gallery: { ...gallery, access_code: null },
			shareToken: null,
			origin: 'https://pp.test',
			onsave,
			schedule: vi.fn(),
			cancel: vi.fn(),
			onarchive: vi.fn(),
			onrestore: vi.fn(),
			ondelete: vi.fn()
		});

		const toggle = screen.getByRole('switch', { name: 'Require access code' });
		await user.click(toggle);
		expect(calls).toHaveLength(1);
		expect(calls[0]).toMatch(/^[0-9]{6}$/);

		// Turn OFF before the first PATCH resolves: PATCH #2 (null) must wait and then fire once PATCH #1 finishes.
		await user.click(toggle);
		expect(calls).toHaveLength(1);

		resolveEnable(true);
		await vi.waitFor(() => expect(calls).toEqual([expect.stringMatching(/^[0-9]{6}$/), null]));
		expect(toggle.getAttribute('aria-checked')).toBe('false');
	});
});
