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
	// Without the UTC suffix, a whole second sorts before its fractional offsets.
	const left = a.last_at.replace(/Z$/, '');
	const right = b.last_at.replace(/Z$/, '');

	return left === right ? b.id.localeCompare(a.id) : left > right ? -1 : 1;
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
	#tail: string | undefined;
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

	get needsReload(): boolean {
		return this.failed && !this.loading && this.#generation !== this.#displayedGeneration;
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
		const generation = ++this.#generation;

		this.loading = true;
		this.next = null;

		try {
			const page = await this.#fetch();

			if (generation !== this.#generation) return;

			this.events = page.events;
			this.next = page.next;
			this.#tail = page.tail ?? undefined;
			this.#displayedGeneration = generation;
			this.failed = false;
			this.loaded = true;
		} catch {
			if (generation === this.#generation) this.failed = true;
		} finally {
			if (generation === this.#generation) this.loading = false;
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
			this.#tail = page.tail ?? cursor;
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

		if (ticket !== this.#refreshes || this.loading) return;

		const generation = this.#generation;
		const oldestFirst = this.sort === 'oldest';
		const advanceTail = oldestFirst && this.next === null;

		if (generation !== this.#displayedGeneration) return this.load();

		try {
			const page = await this.#fetch(
				advanceTail ? this.#tail : undefined,
				advanceTail ? 'oldest' : 'newest'
			);

			if (ticket !== this.#refreshes || generation !== this.#generation) return;

			if (advanceTail) {
				this.#merge(page.events);
				this.next = page.next;
				this.#tail = page.tail ?? this.#tail;
			} else if (oldestFirst) {
				// A live update is at the far end of this order. Don't jump over unvisited pages.
				const loadedIds = new SvelteSet(this.events.map(({ id }) => id));
				this.#merge(page.events.filter(({ id }) => loadedIds.has(id)));
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
