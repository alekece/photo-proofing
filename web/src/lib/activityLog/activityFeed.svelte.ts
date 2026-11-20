import { SvelteMap, SvelteSet } from 'svelte/reactivity';
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
	paging = $state<'idle' | 'loading' | 'failed'>('idle');
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
	#pendingRefresh: Promise<void> | undefined;
	#queuedPage: { promise: Promise<void>; resolve: (page?: Promise<void>) => void } | undefined;
	#advancedTail = false;

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

	get canPage(): boolean {
		return !this.loading && this.next !== null && this.#generation === this.#displayedGeneration;
	}

	get loadingMore(): boolean {
		return this.paging === 'loading';
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
		this.#pendingRefresh = undefined;
		this.#queuedPage?.resolve();
		this.#queuedPage = undefined;
		this.#advancedTail = false;
		this.paging = 'idle';

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
		if (this.#queuedPage) return this.#queuedPage.promise;

		if (cursor === null || !this.canPage) return Promise.resolve();

		this.paging = 'loading';
		if (this.#pendingRefresh) {
			let resolve!: (page?: Promise<void>) => void;
			const promise = new Promise<void>((done) => (resolve = done));
			this.#queuedPage = { promise, resolve };
			return promise;
		}

		const pending = this.#appendPage(cursor, generation).finally(() => {
			if (this.#pendingPage === pending) {
				this.#pendingPage = undefined;
				if (this.paging === 'loading') this.paging = 'idle';
			}
		});
		this.#pendingPage = pending;

		return pending;
	}

	async #appendPage(cursor: string, generation: number): Promise<void> {
		try {
			const page = await this.#fetch(cursor);

			if (generation !== this.#generation) return;

			if (page.next === cursor) {
				this.paging = 'failed';
				return;
			}

			this.#merge(page.events);
			this.next = page.next;
			this.#rememberOldestBoundary(page);
			this.#advancedTail = true;
		} catch {
			if (generation === this.#generation) this.paging = 'failed';
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
	refresh(): Promise<void> {
		const pending = this.#refresh().finally(() => {
			if (this.#pendingRefresh !== pending) return;

			// A queued cursor waits for the current refresh, not a superseded request.
			const queued = this.#queuedPage;
			this.#pendingRefresh = undefined;
			this.#queuedPage = undefined;
			if (queued) queued.resolve(this.loadMore());
		});
		this.#pendingRefresh = pending;

		return pending;
	}

	async #refresh(): Promise<void> {
		const ticket = ++this.#refreshes;
		const generation = this.#generation;
		const oldestFirst = this.sort === 'oldest';
		const current = () => ticket === this.#refreshes && generation === this.#generation;

		if (this.#pendingLoad) await this.#pendingLoad;

		if (!current() || this.loading) return;

		if (generation !== this.#displayedGeneration) return this.load();

		try {
			// Page and live snapshots must read in sequence: last_at is not a version.
			if (this.#pendingPage) {
				await this.#pendingPage;
				if (!current()) return;
			}

			const page = await this.#fetch(undefined, 'newest');

			if (!current()) return;

			let incoming = page.events;

			if (oldestFirst) {
				const loadedIds = new SvelteSet(this.events.map(({ id }) => id));
				if (this.next !== null) {
					incoming = page.events.filter(({ id }) => loadedIds.has(id));
				} else {
					const boundary = this.#oldestBoundary;
					const behindBoundary = (event: ActivityEvent) =>
						boundary !== undefined && byRecency(event, boundary.event) >= 0;
					incoming = page.events.filter(
						(event) => loadedIds.has(event.id) || behindBoundary(event)
					);

					// Reconcile recent rows behind the cursor, but read forward without skipping gaps.
					if (page.events.some((event) => !behindBoundary(event))) {
						const continuation = await this.#fetch(boundary?.cursor, 'oldest');
						if (!current()) return;

						incoming = [...incoming, ...continuation.events];
						this.next = continuation.next;
						this.#rememberOldestBoundary(continuation);
					}
				}
			} else if (!this.#advancedTail && this.paging === 'idle') {
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
		const merged = new SvelteMap(this.events.map((event) => [event.id, event]));

		for (const event of incoming) merged.set(event.id, event);

		const direction = this.sort === 'oldest' ? -1 : 1;
		this.events = [...merged.values()].sort((a, b) => direction * byRecency(a, b));
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
