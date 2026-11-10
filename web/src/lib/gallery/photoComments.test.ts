import { describe, expect, it, vi } from 'vitest';
import { PhotoComments } from './photoComments.svelte';
import { CommentsClient } from './comments';
import type { LiveEvents, NotificationEvent } from '#lib/events.js';

describe('PhotoComments', () => {
	it('refreshes counts and the open thread only for matching gallery activity, and stops on unmount', async () => {
		let notify: (event: NotificationEvent) => void = () => {};
		const off = vi.fn();
		const events: LiveEvents = {
			onNotification: (handler) => {
				notify = handler;
				return off;
			},
			onResync: () => off,
			onPhoto: () => off,
			onFavorites: () => off,
			onExport: () => off,
			start() {},
			close() {}
		};
		const fetcher = vi
			.fn<typeof fetch>()
			.mockImplementation(async () => Response.json([{ photo_id: 'p1', count: 2 }]));
		const comments = new PhotoComments(new CommentsClient('/base', fetcher));
		const stop = comments.start(events, 'g1');
		await vi.waitFor(() => expect(comments.count('p1')).toBe(2));
		const revision = comments.revision;
		notify({ gallery_id: 'other' });
		expect(fetcher).toHaveBeenCalledOnce();
		notify({ gallery_id: 'g1' });
		await vi.waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2));
		expect(comments.revision).toBeGreaterThan(revision);
		stop();
		expect(off).toHaveBeenCalled();
	});

	it('reports counts failures instead of silently showing an empty success', async () => {
		const comments = new PhotoComments(
			new CommentsClient(
				'/base',
				vi.fn<typeof fetch>().mockResolvedValue(new Response(null, { status: 503 }))
			)
		);
		await comments.refresh();
		expect(comments.error).not.toBe('');
	});
});
