import { untrack } from 'svelte';
import { SvelteMap } from 'svelte/reactivity';
import type { LiveEvents } from '#lib/events.js';
import { m } from '#lib/paraglide/messages.js';
import type { CommentsClient } from './comments';

/** Counts and thread refreshes shared by the client, Photos and Favorites surfaces. */
export class PhotoComments {
	#summaries = $state.raw<ReadonlyMap<string, { count: number; latest_body?: string }>>(
		new SvelteMap()
	);
	revision = $state(0);
	error = $state('');
	#latest = 0;

	constructor(readonly client: CommentsClient) {}

	count(id: string): number {
		return this.#summaries.get(id)?.count ?? 0;
	}

	latestBody(id: string): string | undefined {
		return this.#summaries.get(id)?.latest_body;
	}

	async refresh(): Promise<void> {
		const current = ++this.#latest;
		this.revision++;
		try {
			const summaries = await this.client.summaries();
			if (current !== this.#latest) return;
			this.#summaries = summaries;
			this.error = '';
		} catch (cause) {
			if (current === this.#latest)
				this.error = cause instanceof Error ? cause.message : m.api_error_generic();
		}
	}

	start(events?: LiveEvents, galleryId?: string): () => void {
		const refresh = () => {
			void this.refresh();
		};
		const stops = events
			? [
					events.onNotification((event) => {
						if (event.gallery_id === galleryId) refresh();
					}),
					events.onResync(refresh)
				]
			: [];
		untrack(refresh);
		return () => {
			this.#latest++;
			for (const stop of stops) stop();
		};
	}
}
