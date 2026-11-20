import { SvelteSet } from 'svelte/reactivity';
import type { LiveEvents } from '#lib/events.js';
import {
	listActivity,
	type ActivityEvent,
	type ActivityFilters,
	type ActivityPage,
	type ActivitySort
} from './api';

export type { ActivityEvent, ActivityFilters } from './api';

/** Newest first, as the API lists them. */
function byRecency(a: ActivityEvent, b: ActivityEvent): number {
	return b.last_at.localeCompare(a.last_at) || b.id.localeCompare(a.id);
}

/**
 * The activity log a screen shows. Live changes merge into what is displayed rather than
 * replacing it, so the list never blanks or loses its loaded pages while it updates.
 */
export class ActivityFeed {
	events = $state<ActivityEvent[]>([]);
	next = $state<string | null>(null);
	loading = $state(false);
	loadingMore = $state(false);
	loaded = $state(false);
	failed = $state(false);
	filters = $state<ActivityFilters>({});
	sort = $state<ActivitySort>('newest');
	#galleryId: string | undefined;
	#fetcher: typeof fetch;
	#latest = 0;
	#refreshes = 0;
	#generation = 0;
	#displayedGeneration = 0;
	#pendingLoad: Promise<void> | undefined;

	constructor({ galleryId, fetcher = fetch }: { galleryId?: string; fetcher?: typeof fetch } = {}) {
		this.#galleryId = galleryId;
		this.#fetcher = fetcher;
	}

	#fetch(cursor?: string, sort = this.sort): Promise<ActivityPage> {
		return listActivity(
			{ ...this.filters, sort: sort === 'oldest' ? sort : undefined, cursor },
			this.#galleryId,
			this.#fetcher
		);
	}

	clearFilters(): void {
		this.filters = {};
		void this.load();
	}

	/** Replaces the list with the first page of the current filters; the old list stays until then. */
	load(): Promise<void> {
		const pending = this.#load().finally(() => {
			if (this.#pendingLoad === pending) this.#pendingLoad = undefined;
		});

		this.#pendingLoad = pending;

		return pending;
	}

	async #load(): Promise<void> {
		const ticket = ++this.#latest;

		this.#generation += 1;
		this.loading = true;

		try {
			const page = await this.#fetch();

			if (ticket !== this.#latest) return;

			this.events = page.events;
			this.next = page.next;
			this.#displayedGeneration = this.#generation;
			this.failed = false;
			this.loaded = true;
		} catch {
			if (ticket === this.#latest) this.failed = true;
		} finally {
			if (ticket === this.#latest) this.loading = false;
		}
	}

	/** Appends the page after the last one shown. */
	async loadMore(): Promise<void> {
		const cursor = this.next;
		const generation = this.#generation;

		if (cursor === null || this.loadingMore || generation !== this.#displayedGeneration) return;

		this.loadingMore = true;

		try {
			const page = await this.#fetch(cursor);

			if (generation !== this.#generation) return;

			this.#merge(page.events);
			this.next = page.next;
			this.failed = false;
		} catch {
			if (generation === this.#generation) this.failed = true;
		} finally {
			this.loadingMore = false;
		}
	}

	/**
	 * Folds the latest first page into the list, keeping every older page already loaded. It waits
	 * for a pending load and yields to one that starts meanwhile, since a load replaces the list.
	 */
	async refresh(): Promise<void> {
		const ticket = ++this.#refreshes;

		if (this.#pendingLoad) await this.#pendingLoad;

		const generation = this.#generation;
		const oldestFirst = this.sort === 'oldest';

		if (generation !== this.#displayedGeneration) return;

		try {
			const page = await this.#fetch(undefined, 'newest');

			if (ticket !== this.#refreshes || generation !== this.#generation) return;

			if (oldestFirst) {
				// A live update is at the far end of this order. Don't jump over unvisited pages.
				const loadedIds =
					this.next === null ? null : new SvelteSet(this.events.map(({ id }) => id));
				this.#merge(loadedIds ? page.events.filter(({ id }) => loadedIds.has(id)) : page.events);
			} else {
				const oldest = this.events.at(-1);
				const fresh = page.events.at(-1);
				const deeper =
					oldest !== undefined && (fresh === undefined || byRecency(fresh, oldest) < 0);

				this.#merge(page.events);

				if (!deeper) this.next = page.next;
			}

			this.failed = false;
			this.loaded = true;
		} catch {
			if (ticket === this.#refreshes && generation === this.#generation) this.failed = true;
		}
	}

	#merge(incoming: ActivityEvent[]) {
		const merged: Record<string, ActivityEvent> = {};

		for (const event of [...this.events, ...incoming]) merged[event.id] = event;

		const direction = this.sort === 'oldest' ? -1 : 1;
		this.events = Object.values(merged).sort((a, b) => direction * byRecency(a, b));
	}

	/** Refreshes on each activity notification (of its gallery, when scoped) and after a resync. */
	start(events: LiveEvents): () => void {
		const stops = [
			events.onNotification(({ gallery_id }) => {
				if (this.#galleryId === undefined || gallery_id === this.#galleryId) void this.refresh();
			}),
			events.onResync(() => void this.refresh())
		];

		return () => {
			for (const stop of stops) stop();
		};
	}
}
