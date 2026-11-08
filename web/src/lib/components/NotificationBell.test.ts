import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { NotificationCenter } from '#lib/gallery/notifications.svelte.js';
import NotificationBell from './NotificationBell.svelte';
import { useLocale } from '#lib/i18n/testing.js';

const row = (over = {}) => ({
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

const setup = (rows: object[]) => {
	let current = rows;
	const fetcher = vi.fn(async (_url: string, init?: RequestInit) => {
		if (init?.method === 'POST') current = [];

		return new Response(JSON.stringify({ notifications: current, unread: current.length }), {
			status: 200
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
		expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'POST')).toEqual([]);
	});

	it('marks a row read when it is followed', async () => {
		const { center, fetcher } = setup([row({ kind: 'download', count: 2 })]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));
		const link = screen.getByRole('link', { name: /Wedding/ });
		link.addEventListener('click', (event) => event.preventDefault());
		await userEvent.click(link);

		expect(link.getAttribute('href')).toBe('/dashboard/g1');
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/notifications/n1/read',
			expect.objectContaining({ method: 'POST' })
		);
		expect(center.unread).toBe(0);
	});

	it('marks everything read, and shows the empty state', async () => {
		const { center } = setup([row()]);
		await center.refresh();
		render(NotificationBell, { center });

		await userEvent.click(screen.getByRole('button', { name: /Notifications/ }));
		await userEvent.click(screen.getByRole('button', { name: 'Mark all as read' }));

		expect(screen.getByText('Nothing new.')).toBeTruthy();
		expect(
			(screen.getByRole('button', { name: 'Mark all as read' }) as HTMLButtonElement).disabled
		).toBe(true);
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
});
