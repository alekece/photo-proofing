import { getContext, setContext } from 'svelte';
import type { LiveEvents } from '#lib/events.js';
import { m } from '#lib/paraglide/messages.js';
import { notificationListSchema, parseOrNull, type Notification } from './schemas';

export type { Notification } from './schemas';

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/** What happened, in the words of the bell and the card badge. */
export function notificationText(notification: Notification): string {
	if (notification.kind === 'selection') {
		return m.notification_selected({ count: notification.selected });
	}
	if (notification.kind === 'open') return m.notification_opened({ count: notification.count });
	if (notification.kind === 'comment')
		return m.notification_comments({ count: notification.count });
	if (notification.count === 0) return m.notification_zip();

	return m.notification_downloads({ count: notification.count });
}

export function notificationBadge(notification: Notification): string {
	if (notification.kind === 'selection') return m.notification_badge_selection();
	if (notification.kind === 'open') return m.notification_badge_opened();
	if (notification.kind === 'comment') return m.comments_title();

	return m.notification_badge_downloads();
}

export function timeAgo(iso: string, now: number = Date.now()): string {
	const elapsed = Math.max(0, now - Date.parse(iso));

	if (elapsed < MINUTE_MS) return m.time_just_now();
	if (elapsed < HOUR_MS) return m.time_minutes_ago({ count: Math.floor(elapsed / MINUTE_MS) });
	if (elapsed < DAY_MS) return m.time_hours_ago({ count: Math.floor(elapsed / HOUR_MS) });

	return m.time_days_ago({ count: Math.floor(elapsed / DAY_MS) });
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

	/** Every unread notification of a gallery, newest first as the server lists them. */
	unreadFor(galleryId: string): Notification[] {
		return this.items.filter((item) => item.gallery_id === galleryId && item.read_at === null);
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

	/** Opening a gallery reads its news: the badge goes at once, then the server confirms. */
	async markGalleryRead(galleryId: string): Promise<void> {
		const cleared = this.items.filter(
			(item) => item.gallery_id === galleryId && item.read_at === null
		).length;

		if (cleared > 0) {
			// A one-off timestamp, not reactive state.
			// eslint-disable-next-line svelte/prefer-svelte-reactivity
			const now = new Date().toISOString();

			this.items = this.items.map((item) =>
				item.gallery_id === galleryId && item.read_at === null ? { ...item, read_at: now } : item
			);
			this.unread = Math.max(0, this.unread - cleared);
		}

		await this.#post(`/galleries/${encodeURIComponent(galleryId)}/read`);
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
