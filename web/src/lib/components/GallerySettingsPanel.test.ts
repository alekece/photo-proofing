import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Gallery } from '#lib/gallery/api.js';
import { toasts } from './ui/Toast/toasts.svelte.js';
import GallerySettingsPanel from './GallerySettingsPanel.svelte';

const gallery = {
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
		onarchive,
		onrestore,
		ondelete
	});

	return {
		...view,
		onsave,
		schedule,
		cancel,
		onarchive,
		onrestore,
		ondelete,
		user: userEvent.setup()
	};
}

afterEach(() => {
	toasts.items = [];
	vi.useRealTimers();
});

describe('GallerySettingsPanel', () => {
	it('splits settings into General, Privacy & Protection, and Actions cards', async () => {
		const { container } = setup();

		expect(screen.getByRole('heading', { name: 'Settings' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'General' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Privacy & Protection' })).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Actions' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('says a draft is not shared and offers the link once shared', () => {
		const first = setup();

		expect(screen.getByText('Not shared yet')).toBeTruthy();
		first.unmount();

		setup({}, true, 'A'.repeat(22));

		expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
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

	it('saves the picked open-until date as the end of that day and clears it with null', async () => {
		const { onsave, user } = setup({ expires_at: new Date(2030, 5, 15, 23, 59, 59).toISOString() });
		const trigger = screen.getByLabelText('Open until');

		expect(trigger.textContent).toContain('Jun 15, 2030');

		await user.click(trigger);
		await user.click(
			within(screen.getByRole('grid')).getByText('20', {
				selector: '[data-bits-day]:not([data-outside-month])'
			})
		);

		expect(onsave).toHaveBeenCalledWith({
			expires_at: new Date(2030, 5, 20, 23, 59, 59).toISOString()
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
			expires_at: new Date(2026, 5, 15, 23, 59, 59).toISOString()
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
		const draft = setup({ access_code: null, access_code_protected: false });
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

		const failing = setup({ access_code: null, access_code_protected: false }, false);
		const failToggle = screen.getByRole('switch', { name: 'Require access code' });
		await failing.user.click(failToggle);
		await vi.waitFor(() => expect(failToggle.getAttribute('aria-checked')).toBe('false'));
	});

	it('locks access-code controls once shared, keeping Copy when set and disabling the switch when unset', async () => {
		const sharedWithCode = setup(
			{ access_code: '042819', access_code_protected: true, states: ['shared'] },
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
			{ access_code: null, access_code_protected: false, states: ['shared'] },
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
});
