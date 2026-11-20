import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { LiveEvents } from '#lib/events.js';
import { useLocale } from '#lib/i18n/testing.js';
import Harness from './GalleryActivityPanelHarness.test.svelte';

const events = {
	onNotification: () => () => {},
	onResync: () => () => {}
} as unknown as LiveEvents;

const comment = {
	id: 'a',
	gallery_id: 'g1',
	gallery_title: 'Wedding',
	contact: { id: 'c1', name: 'Alice' },
	kind: 'comment',
	count: 1,
	photo_ids: [],
	detail: null,
	first_at: new Date().toISOString(),
	last_at: new Date().toISOString()
};

afterEach(() => vi.unstubAllGlobals());

describe('GalleryActivityPanel', () => {
	useLocale('en');

	it("lists the gallery's activity, offers its linked contacts, and filters by kind", async () => {
		const fetcher = vi.fn(async (url: string) =>
			url.endsWith('/contacts')
				? Response.json([
						{
							id: 'c1',
							name: 'Alice',
							email: 'alice@example.test',
							phone: null,
							notes: null,
							galleries: [],
							created_at: '2026-11-01T00:00:00Z',
							updated_at: '2026-11-01T00:00:00Z'
						}
					])
				: Response.json({ events: [comment], next: null })
		);

		vi.stubGlobal('fetch', fetcher);
		render(Harness, { props: { events, galleryId: 'g1' } });

		const paragraph = (await screen.findByRole('listitem')).querySelector('p');
		expect(paragraph?.textContent?.trim()).toBe('Alice left a comment');
		expect(paragraph?.querySelector('strong')?.textContent).toBe('Alice');
		const user = userEvent.setup();
		await user.click(screen.getByRole('button', { name: 'Filters' }));
		expect(await screen.findByRole('option', { name: 'Alice' })).toBeTruthy();
		expect(screen.queryByRole('combobox', { name: 'Gallery' })).toBeNull();
		expect(screen.queryByRole('link', { name: 'Wedding' })).toBeNull();

		await user.selectOptions(screen.getByRole('combobox', { name: 'Type' }), 'comment');

		expect(fetcher).toHaveBeenLastCalledWith(
			'/api/v1/galleries/g1/timeline?kind=comment',
			expect.anything()
		);
	});
});
