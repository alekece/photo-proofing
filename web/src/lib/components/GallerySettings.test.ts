import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';

import type { Gallery } from '#lib/gallery/api.js';

import { toasts } from './ui/Toast/toasts.svelte.js';
import GallerySettings from './GallerySettings.svelte';

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

	const view = render(GallerySettings, {
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
});

describe('GallerySettings', () => {
	it('says a draft is not shared and offers the link once shared', () => {
		setup();

		expect(screen.getByText('Not shared yet')).toBeTruthy();

		setup({}, true, 'A'.repeat(22));

		expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
		expect(screen.queryByRole('link', { name: 'Open' })).toBeNull();
	});

	it('saves the layout and watermark as patches', async () => {
		const { onsave, user } = setup();

		await user.click(screen.getByRole('button', { name: 'Staggered columns' }));
		await user.click(screen.getByRole('switch', { name: 'Enable watermarks' }));

		expect(onsave).toHaveBeenNthCalledWith(1, { layout: 'masonry' });
		expect(onsave).toHaveBeenNthCalledWith(2, { watermark_enabled: true });
	});

	it('puts a refused setting back', async () => {
		const { user } = setup({}, false);
		const toggle = screen.getByRole('switch', { name: 'Enable watermarks' });

		await user.click(toggle);

		await vi.waitFor(() => expect(toggle.getAttribute('aria-checked')).toBe('false'));
	});

	it('does not flash a success ring after autosaving', async () => {
		const { user } = setup();

		await user.click(screen.getByRole('switch', { name: 'Enable watermarks' }));

		await vi.waitFor(() =>
			expect(screen.getByRole('switch', { name: 'Enable watermarks' }).className).not.toContain(
				'ring-ok'
			)
		);
	});

	it('saves the picked open-until date as the end of that day and clears it with null', async () => {
		const { onsave, user } = setup({ expires_at: new Date(2030, 5, 15, 23, 59, 59).toISOString() });
		const trigger = screen.getByLabelText('Open until');

		expect(trigger.textContent).toContain('June 15, 2030');

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

	it('shows the shoot date and clears it with null', async () => {
		const { onsave, user } = setup({ shoot_date: '2026-06-14' });
		const trigger = screen.getByLabelText('Shoot date');

		expect(trigger.textContent).toContain('June 14, 2026');

		await user.click(trigger);
		await user.click(screen.getByRole('button', { name: 'Clear' }));

		expect(onsave).toHaveBeenCalledWith({ shoot_date: null });
	});

	it('debounces included photos through the page and saves on leaving', async () => {
		const { onsave, schedule, cancel, user } = setup();

		await user.type(screen.getByLabelText('Included photos'), '5');
		expect(schedule).toHaveBeenCalledWith('included_photos', expect.any(Function));
		expect(onsave).not.toHaveBeenCalled();

		await user.tab();
		expect(cancel).toHaveBeenCalledWith('included_photos');
		expect(onsave).toHaveBeenCalledWith({ included_photos: 5 });
	});

	it('disables the included photos once the gallery is delivered', () => {
		setup({ included_photos: 3, states: ['delivered'] });

		expect((screen.getByLabelText('Included photos') as HTMLInputElement).disabled).toBe(true);
	});

	it('reverts an invalid included count with an error toast', async () => {
		const { onsave, user } = setup({ included_photos: 3 });
		const input = screen.getByLabelText('Included photos') as HTMLInputElement;

		await user.clear(input);
		await user.type(input, '0');
		await user.tab();

		expect(onsave).not.toHaveBeenCalled();
		expect(input.value).toBe('3');
		expect(toasts.items).toHaveLength(1);
	});

	it('leaves a half-typed included count alone until the field is left', async () => {
		const { onsave, schedule, user } = setup({ included_photos: 3 });
		const input = screen.getByLabelText('Included photos') as HTMLInputElement;

		await user.clear(input);
		await user.type(input, '0');
		schedule.mock.calls.at(-1)![1]();

		expect(input.value).toBe('0');
		expect(onsave).not.toHaveBeenCalled();
		expect(toasts.items).toEqual([]);
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

	it('pops a card out of the phone rail and closes it with Escape', async () => {
		const { user } = setup();
		const button = screen.getByRole('button', { name: 'Show Look' });

		await user.click(button);
		expect(button.getAttribute('aria-expanded')).toBe('true');

		await user.keyboard('{Escape}');
		expect(button.getAttribute('aria-expanded')).toBe('false');
	});

	it('has no axe violations', async () => {
		const { container } = setup();

		expect((await axe(container)).violations).toEqual([]);
	});
});
