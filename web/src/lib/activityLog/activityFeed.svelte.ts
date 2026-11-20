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
	#oldestBoundary: { cursor: string; event: ActivityEvent } | undefined;
	#refreshes = 0;
	#generation = 0;
	#displayedGeneration = 0;
	#pendingLoad: Promise<void> | undefined;
	#pendingPage: Promise<void> | undefined;

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
		this.#pendingPage = undefined;
		this.loadingMore = false;

		try {
			const page = await this.#fetch();

			if (generation !== this.#generation) return;

			this.events = page.events;
			this.next = page.next;
			this.#oldestBoundary = undefined;
			this.#rememberOldestBoundary(page);
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
	loadMore(): Promise<void> {
		const cursor = this.next;
		const generation = this.#generation;

		if (this.#pendingPage) return this.#pendingPage;

		if (cursor === null || generation !== this.#displayedGeneration) return Promise.resolve();

		this.loadingMore = true;
		const pending = this.#appendPage(cursor, generation).finally(() => {
			if (this.#pendingPage === pending) {
				this.#pendingPage = undefined;
				this.loadingMore = false;
			}
		});
		this.#pendingPage = pending;

		return pending;
	}

	async #appendPage(cursor: string, generation: number): Promise<void> {
		try {
			const page = await this.#fetch(cursor);

			if (generation !== this.#generation) return;

			this.#merge(page.events);
			this.next = page.next;
			this.#rememberOldestBoundary(page);
			this.failed = false;
		} catch {
			if (generation === this.#generation) this.failed = true;
		}
	}

	#rememberOldestBoundary(page: ActivityPage): void {
		const cursor = page.next ?? page.tail;
		const event = page.events.at(-1);

		if (this.sort === 'oldest' && cursor && event) this.#oldestBoundary = { cursor, event };
	}

	/**
	 * Reconciles recent rows and, when an oldest-first list is exhausted, resumes forward from its
	 * last page. A replacement load invalidates both kinds of refresh.
	 */
	async refresh(): Promise<void> {
		const ticket = ++this.#refreshes;

		if (this.#pendingLoad) await this.#pendingLoad;

		if (ticket !== this.#refreshes || this.loading) return;

		const generation = this.#generation;
		const oldestFirst = this.sort === 'oldest';
		const current = () => ticket === this.#refreshes && generation === this.#generation;

		if (generation !== this.#displayedGeneration) return this.load();

		try {
			const page = await this.#fetch(undefined, 'newest');

			if (!current()) return;

			let incoming = page.events;

			if (oldestFirst) {
				// A pending page can exhaust the list after the newest request started.
				await this.#pendingPage;
				if (!current()) return;

				if (this.next !== null) {
					const loadedIds = new SvelteSet(this.events.map(({ id }) => id));
					incoming = page.events.filter(({ id }) => loadedIds.has(id));
				} else {
					const boundary = this.#oldestBoundary;
					incoming = boundary
						? page.events.filter((event) => byRecency(event, boundary.event) >= 0)
						: [];

					// Reconcile recent rows behind the cursor, but read forward without skipping gaps.
					if (incoming.length < page.events.length) {
						const continuation = await this.#fetch(boundary?.cursor, 'oldest');
						if (!current()) return;

						incoming = [...incoming, ...continuation.events];
						this.next = continuation.next;
						this.#rememberOldestBoundary(continuation);
					}
				}
			} else {
				const oldest = this.events.at(-1);
				const fresh = page.events.at(-1);
				const deeper =
					oldest !== undefined && (fresh === undefined || byRecency(fresh, oldest) < 0);

				if (!deeper) this.next = page.next;
			}

			this.#merge(incoming);
			this.failed = false;
			this.loaded = true;
		} catch {
			if (current()) this.failed = true;
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
