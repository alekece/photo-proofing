import { render, screen, waitFor } from '@testing-library/svelte';
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

	it("searches visitors in the gallery's own timeline with the type filter", async () => {
		const fetcher = vi.fn(async () => Response.json({ events: [comment], next: null }));

		vi.stubGlobal('fetch', fetcher);
		render(Harness, { props: { events, galleryId: 'g1' } });

		const paragraph = (await screen.findByRole('listitem')).querySelector('p');
		expect(paragraph?.textContent?.trim()).toBe('Alice left a comment');
		expect(paragraph?.querySelector('strong')?.textContent).toBe('Alice');
		const user = userEvent.setup();
		expect(screen.queryByRole('button', { name: 'Visitor' })).toBeNull();
		expect(screen.queryByRole('link', { name: 'Wedding' })).toBeNull();

		await user.click(screen.getByRole('button', { name: 'Type' }));
		await user.click(screen.getByRole('menuitemradio', { name: 'Comments' }));

		await user.type(screen.getByRole('searchbox', { name: 'Search by visitor' }), 'Alice');
		await waitFor(() =>
			expect(fetcher).toHaveBeenLastCalledWith(
				'/api/v1/galleries/g1/timeline?kind=comment&q=Alice',
				expect.anything()
			)
		);
	});
});
