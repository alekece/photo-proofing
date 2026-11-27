import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { NotificationCenter, type Notification } from '#lib/gallery/notifications.svelte.js';
import NotificationBell from './NotificationBell.svelte';
import { useLocale } from '#lib/i18n/testing.js';

const state = vi.hoisted(() => ({
	page: { route: { id: '/dashboard/[id]' }, params: { id: 'another' }, error: null }
}));
vi.mock('$app/state', () => state);
beforeEach(() => {
	state.page.params.id = 'another';
});

const row = (over: Partial<Notification> = {}): Notification => ({
	id: 'n1',
	gallery_id: 'g1',
	title: 'Wedding',
	hero_id: null,
	kind: 'selection',
	count: 0,
	selected: 13,
	updated_at: new Date().toISOString(),
	read_at: null,
	...over
});

const setup = (rows: Notification[]) => {
	let current = rows;
	const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
		if (init?.method === 'DELETE') {
			current =
				url === '/api/v1/notifications' ? [] : current.filter((item) => !url.endsWith(item.id));
		}
		if (init?.method === 'POST') {
			current = current.map((item) =>
				url === '/api/v1/notifications/read' ||
				url === `/api/v1/notifications/galleries/${item.gallery_id}/read`
					? { ...item, read_at: item.updated_at }
					: item
			);
		}

		return Response.json({
			notifications: current,
			unread: current.filter((item) => item.read_at === null).length
		});
	});
	const center = new NotificationCenter(fetcher as unknown as typeof fetch);

	return { center, fetcher };
};

describe('NotificationBell', () => {
	it('names the unread count and marks nothing read on open', async () => {
		const { center, fetcher } = setup([row()]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: 'Notifications, 1 unread' }));

		expect(screen.getByText('Wedding')).toBeTruthy();
		expect(screen.getByText(/13 selected/)).toBeTruthy();
		await userEvent.click(screen.getByRole('button', { name: 'Close' }));
		expect(document.activeElement).toBe(
			screen.getByRole('button', { name: 'Notifications, 1 unread' })
		);
		expect(screen.queryByText('Wedding')).toBeNull();
		expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'POST')).toEqual([]);
	});

	it('uses a viewport-safe panel placement', async () => {
		const { center } = setup([row()]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));
		const panel = screen.getByText('Wedding').closest('[data-slot="popover-panel"]');

		expect(panel?.className).toContain('fixed');
		expect(panel?.className).toContain('right-3');
		expect(panel?.className).toContain('left-3');
	});

	it('says the client opened the gallery, with the number of opens', async () => {
		const { center } = setup([
			row({ kind: 'open', count: 1 }),
			row({ id: 'n2', kind: 'open', count: 3 })
		]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));

		expect(screen.getByText(/^The client opened the gallery ·/)).toBeTruthy();
		expect(screen.getByText(/^The client opened the gallery 3 times ·/)).toBeTruthy();
	});

	it('leaves marking the gallery read to the destination', async () => {
		const { center, fetcher } = setup([row({ kind: 'download', count: 2 })]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));
		const link = screen.getByRole('link', { name: /Wedding/ });
		window.addEventListener('click', (event) => event.preventDefault(), { once: true });
		await userEvent.click(link);

		expect(link.getAttribute('href')).toBe('/dashboard/g1?section=final');
		expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'POST')).toEqual([]);
	});

	it('opens the tab that holds what each kind of notification reports', async () => {
		const { center } = setup([
			row({ id: 'a', kind: 'selection' }),
			row({ id: 'b', kind: 'comment', count: 1 }),
			row({ id: 'c', kind: 'open', count: 1 })
		]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));

		expect(screen.getAllByRole('link').map((link) => link.getAttribute('href'))).toEqual([
			'/dashboard/g1?section=favorites',
			'/dashboard/g1?section=comments',
			'/dashboard/g1'
		]);
	});

	it('marks an explicit visit to the already-open gallery read exactly once', async () => {
		state.page.params.id = 'g1';
		const { center, fetcher } = setup([row()]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));
		expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'POST')).toEqual([]);
		window.addEventListener('click', (event) => event.preventDefault(), { once: true });
		await userEvent.click(screen.getByRole('link', { name: /Wedding/ }));

		expect(
			fetcher.mock.calls.filter(([, init]) => init?.method === 'POST').map(([url]) => url)
		).toEqual(['/api/v1/notifications/galleries/g1/read']);
		await userEvent.click(screen.getByRole('button', { name: 'Notifications' }));

		expect(screen.getByRole('link', { name: /Wedding/ })).toBeTruthy();
		expect(screen.queryByText('Unread activity')).toBeNull();
	});

	it('shows read and unread activity, and keeps both visible after marking all read', async () => {
		const { center, fetcher } = setup([
			row(),
			row({ id: 'n2', title: 'Portrait', read_at: '2026-01-01T10:00:00Z' })
		]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: 'Notifications, 1 unread' }));
		expect(screen.getByRole('link', { name: /Wedding.*Unread activity/ })).toBeTruthy();
		expect(screen.getByRole('link', { name: /Portrait/ })).toBeTruthy();
		await userEvent.click(screen.getByRole('button', { name: 'Mark all as read' }));

		expect(screen.getAllByRole('link')).toHaveLength(2);
		expect(screen.queryByText('Unread activity')).toBeNull();
		expect(screen.getByRole('button', { name: 'Notifications' })).toBeTruthy();
		expect(
			(screen.getByRole('button', { name: 'Mark all as read' }) as HTMLButtonElement).disabled
		).toBe(true);
		expect(
			fetcher.mock.calls.filter(([, init]) => init?.method === 'POST').map(([url]) => url)
		).toEqual(['/api/v1/notifications/read']);
	});

	it('deletes every notification at once, then closes the menu', async () => {
		const user = userEvent.setup();
		const { center, fetcher } = setup([row(), row({ id: 'n2', title: 'Portraits' })]);
		await center.refresh();
		render(NotificationBell, { center });

		await user.click(screen.getByRole('button', { name: /Notifications/ }));
		await user.click(screen.getByRole('button', { name: 'Delete all' }));

		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/notifications',
			expect.objectContaining({ method: 'DELETE' })
		);
		await waitFor(() => expect(screen.queryByText('Portraits')).toBeNull());
		expect(screen.queryByRole('button', { name: 'Delete all' })).toBeNull();

		await user.click(screen.getByRole('button', { name: /Notifications/ }));

		expect(screen.getByText('Nothing new')).toBeTruthy();
	});

	it('keeps the menu open and says so when the notifications could not be deleted', async () => {
		const user = userEvent.setup();
		const fetcher = vi.fn(async (_url: string, init?: RequestInit) =>
			init?.method === 'DELETE'
				? new Response(null, { status: 500 })
				: Response.json({ notifications: [row()], unread: 1 })
		);
		const center = new NotificationCenter(fetcher as unknown as typeof fetch);
		await center.refresh();
		render(NotificationBell, { center });

		await user.click(screen.getByRole('button', { name: /Notifications/ }));
		await user.click(screen.getByRole('button', { name: 'Delete all' }));

		await waitFor(() =>
			expect(fetcher.mock.calls.some(([, i]) => i?.method === 'DELETE')).toBe(true)
		);
		expect(screen.getByRole('button', { name: 'Delete all' })).toBeTruthy();
		expect(screen.getByText('Wedding')).toBeTruthy();
	});

	it('shows the simple empty state when there are no notifications', async () => {
		const { center } = setup([]);
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: 'Notifications' }));

		expect(screen.getByText('Nothing new')).toBeTruthy();
		expect(screen.queryByRole('link')).toBeNull();
	});
});

describe('NotificationBell in French', () => {
	useLocale('fr');

	it('names the unread count and the news in French', async () => {
		const { center } = setup([row()]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: 'Notifications, 1 non lue' }));

		expect(screen.getByText(/13 sélectionnées/)).toBeTruthy();
	});

	it('says the client opened the gallery in French', async () => {
		const { center } = setup([row({ kind: 'open', count: 2 })]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));

		expect(screen.getByText(/^Le client a ouvert la galerie 2 fois ·/)).toBeTruthy();
	});
});
