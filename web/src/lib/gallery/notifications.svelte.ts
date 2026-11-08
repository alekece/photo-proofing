import { getContext, setContext } from 'svelte';
import type { LiveEvents } from '#lib/events.js';
import { notificationListSchema, parseOrNull, type Notification } from './schemas';

export type { Notification } from './schemas';

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/** What happened, in the words of the bell and the card badge. */
export function notificationText(notification: Notification): string {
	if (notification.kind === 'selection') return `${notification.selected} selected`;
	if (notification.count === 0) return 'Downloaded as a ZIP';

	return notification.count === 1 ? '1 new download' : `${notification.count} new downloads`;
}

export function notificationBadge(notification: Notification): string {
	return notification.kind === 'selection' ? 'New selection' : 'New downloads';
}

export function timeAgo(iso: string, now: number = Date.now()): string {
	const elapsed = Math.max(0, now - Date.parse(iso));

	if (elapsed < MINUTE_MS) return 'just now';
	if (elapsed < HOUR_MS) return `${Math.floor(elapsed / MINUTE_MS)} min ago`;
	if (elapsed < DAY_MS) return `${Math.floor(elapsed / HOUR_MS)} h ago`;

	return `${Math.floor(elapsed / DAY_MS)} d ago`;
}

/** The workspace's notifications; the server's list is authoritative, so every change refetches. */
export class NotificationCenter {
	items = $state<Notification[]>([]);
	unread = $state(0);
	#fetcher: typeof fetch;
	#latest = 0;

	constructor(fetcher: typeof fetch = fetch) {
		this.#fetcher = fetcher;
	}

	/** The unread notification of a gallery, if any. */
	unreadFor(galleryId: string): Notification | undefined {
		return this.items.find((item) => item.gallery_id === galleryId && item.read_at === null);
	}

	async refresh(): Promise<void> {
		const ticket = ++this.#latest;
		const response = await this.#fetcher('/api/v1/notifications', {
			credentials: 'same-origin'
		}).catch(() => null);
		const list = response?.ok
			? parseOrNull(notificationListSchema, await response.json().catch(() => null))
			: null;

		// A newer refresh already answered, or the answer cannot be trusted: keep what is shown.
		if (!list || ticket !== this.#latest) return;

		this.items = list.notifications;
		this.unread = list.unread;
	}

	async markRead(id: string): Promise<void> {
		await this.#post(`/${encodeURIComponent(id)}/read`);
	}

	async markAllRead(): Promise<void> {
		await this.#post('/read');
	}

	/** Loads the list and keeps it current; returns what stops that. */
	start(events: LiveEvents): () => void {
		const stops = [
			events.onNotification(() => void this.refresh()),
			events.onResync(() => void this.refresh())
		];

		void this.refresh();

		return () => stops.forEach((stop) => stop());
	}

	async #post(path: string) {
		await this.#fetcher(`/api/v1/notifications${path}`, {
			method: 'POST',
			credentials: 'same-origin',
			keepalive: true
		}).catch(() => null);
		await this.refresh();
	}
}

const key = Symbol('notifications');

export function setNotifications(center: NotificationCenter) {
	setContext(key, center);
}

/** The notifications of the enclosing dashboard layout; none outside it. */
export function getNotifications(): NotificationCenter | undefined {
	return getContext(key);
}
