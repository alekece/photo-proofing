import { describe, expect, it, vi } from 'vitest';
import { NotificationCenter } from './notifications.svelte';

const row = (over = {}) => ({
	id: 'n1',
	gallery_id: 'g1',
	title: 'Wedding',
	hero_id: null,
	kind: 'selection',
	count: 0,
	selected: 2,
	updated_at: '2026-11-09T14:38:00Z',
	read_at: null,
	...over
});

describe('NotificationCenter.markGalleryRead', () => {
	it('clears the gallery badge before the server answers, then posts and refreshes', async () => {
		let answer!: () => void;
		const rows = [row(), row({ id: 'n2', gallery_id: 'g2' })];
		const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
			if (init?.method === 'POST') await new Promise<void>((resolve) => (answer = resolve));

			const current = init?.method === 'POST' ? [] : rows;

			return Response.json({ notifications: current, unread: current.length });
		});
		const center = new NotificationCenter(fetcher as unknown as typeof fetch);
		await center.refresh();

		const marking = center.markGalleryRead('g1');

		expect(center.unreadFor('g1')).toBeUndefined();
		expect(center.unreadFor('g2')).toBeDefined();
		expect(center.unread).toBe(1);

		answer();
		await marking;

		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/notifications/galleries/g1/read',
			expect.objectContaining({ method: 'POST' })
		);
		expect(fetcher.mock.calls.at(-1)?.[0]).toBe('/api/v1/notifications');
	});
});
