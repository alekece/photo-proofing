import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';
import type { GalleryOverview } from '#lib/gallery/api.js';
import QuickEditGalleriesDialog, { type QuickGalleryEdit } from './QuickEditGalleriesDialog.svelte';

const gallery = (over: Partial<GalleryOverview> = {}): GalleryOverview => ({
	id: 'g1',
	title: 'Martin Wedding',
	favorite: false,
	layout: 'grid',
	cover_layout: 'full_bleed',
	focal_point: { x: 50, y: 35 },
	heading_font: 'editorial',
	grid_density: 'compact',
	grid_spacing: 'compact',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	last_opened_at: null,
	shoot_date: '2026-06-14',
	states: ['draft'],
	created_at: '2026-01-01T10:00:00Z',
	updated_at: '2026-01-02T10:00:00Z',
	photo_count: 0,
	favorite_count: 0,
	final_count: 0,
	downloaded_count: 0,
	...over
});

describe('QuickEditGalleriesDialog', () => {
	it('renders one row per selected gallery and saves only modified rows with zero axe violations', async () => {
		const user = userEvent.setup();
		const onsave = vi.fn().mockResolvedValue(null);
		const onclose = vi.fn();
		const items = [
			gallery({ id: 'g1', title: 'Martin Wedding', shoot_date: '2026-06-14' }),
			gallery({ id: 'g2', title: 'Portrait Session', shoot_date: null })
		];

		const { container } = render(QuickEditGalleriesDialog, {
			galleries: items,
			onsave,
			onclose
		});

		const dialog = screen.getByRole('dialog', { name: 'Quick edit galleries' });
		expect(dialog.className).toContain('overflow-visible');
		expect((await axe(container)).violations).toEqual([]);

		const firstTitle = screen.getByDisplayValue('Martin Wedding');
		await user.clear(firstTitle);
		await user.type(firstTitle, 'Martin & Claire');

		const secondDateBtn = screen.getAllByRole('button', { name: /shoot date/i })[1];
		await user.click(secondDateBtn);
		await user.click(
			within(screen.getByRole('grid')).getByText('15', {
				selector: '[data-bits-day]:not([data-outside-month])'
			})
		);

		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		expect(onsave).toHaveBeenCalledOnce();
		const edits = onsave.mock.calls[0][0];
		expect(edits).toHaveLength(2);
		expect(edits[0]).toEqual({ id: 'g1', changes: { title: 'Martin & Claire' } });
		expect(edits[1].id).toBe('g2');
		expect(edits[1].changes.shoot_date).toMatch(/^\d{4}-\d{2}-15$/);
		await waitFor(() => expect(onclose).toHaveBeenCalledOnce());
	});

	it('skips unchanged rows and closes cleanly when nothing changed', async () => {
		const user = userEvent.setup();
		const onsave = vi.fn().mockResolvedValue(null);
		const onclose = vi.fn();

		render(QuickEditGalleriesDialog, {
			galleries: [gallery({ id: 'g1', title: 'Unchanged', shoot_date: '2026-06-14' })],
			onsave,
			onclose
		});

		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		expect(onsave).not.toHaveBeenCalled();
		await waitFor(() => expect(onclose).toHaveBeenCalledOnce());
	});

	it('prunes succeeded rows on partial failure so retrying only re-submits failed rows', async () => {
		const user = userEvent.setup();
		const items = [
			gallery({ id: 'g1', title: 'One', shoot_date: null }),
			gallery({ id: 'g2', title: 'Two', shoot_date: null })
		];
		const onclose = vi.fn();
		let attempt = 0;
		const { rerender } = render(QuickEditGalleriesDialog, {
			galleries: items,
			onsave: vi.fn(async (edits: QuickGalleryEdit[]) => {
				attempt++;
				if (attempt === 1) {
					expect(edits.map((e) => e.id)).toEqual(['g1', 'g2']);
					await rerender({
						galleries: [items[1]],
						onsave,
						onclose
					});
					return 'Could not save Two';
				}
				expect(edits.map((e) => e.id)).toEqual(['g2']);
				return null;
			}),
			onclose
		});
		const onsave = vi.fn().mockResolvedValue(null);

		const inputs = screen.getAllByLabelText('Title');
		await user.type(inputs[0], ' Updated');
		await user.type(inputs[1], ' Updated');
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		expect((await screen.findByRole('alert')).textContent).toBe('Could not save Two');
		expect(screen.getAllByLabelText('Title')).toHaveLength(1);
		expect(onclose).not.toHaveBeenCalled();

		await user.click(screen.getByRole('button', { name: 'Save changes' }));
		expect(onsave).toHaveBeenCalledWith([{ id: 'g2', changes: { title: 'Two Updated' } }]);
		await waitFor(() => expect(onclose).toHaveBeenCalledOnce());
	});
});
