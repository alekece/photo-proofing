import { SvelteSet } from 'svelte/reactivity';
import { favoriteIdsSchema, parseOrNull } from './schemas';

interface SelectionOptions {
	token: string;
	initial: string[];
	fetcher?: typeof fetch;
}

const FAILED = 'We couldn’t save that choice. Please try again.';
const FINAL = 'This gallery has been delivered, so the selection is final.';

/**
 * What a client has selected on a private link. A choice shows at once and is saved in the
 * background; if saving fails it goes back and a short message says so.
 */
export class Selection {
	readonly ids = new SvelteSet<string>();
	error = $state<string | null>(null);

	readonly #base: string;
	readonly #fetcher: typeof fetch;
	/** The last save of each photo being saved, so toggles of one photo reach the server in order. */
	// Bookkeeping only; nothing renders from it.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	readonly #saving = new Map<string, Promise<void>>();

	constructor({ token, initial, fetcher = fetch }: SelectionOptions) {
		this.#base = `/api/v1/galleries/shared/${encodeURIComponent(token)}`;
		this.#fetcher = fetcher;
		for (const id of initial) this.ids.add(id);
	}

	get count() {
		return this.ids.size;
	}

	has(id: string) {
		return this.ids.has(id);
	}

	toggle(id: string): Promise<void> {
		const selected = !this.ids.has(id);

		if (selected) this.ids.add(id);
		else this.ids.delete(id);

		const previous = this.#saving.get(id) ?? Promise.resolve();
		const saving = previous.then(() => this.#save(id, selected));
		const tracked = saving.finally(() => {
			if (this.#saving.get(id) === tracked) this.#saving.delete(id);
		});

		this.#saving.set(id, tracked);

		return tracked;
	}

	/** Replaces the selection with the server's, except photos still being saved. */
	async refresh(): Promise<void> {
		let ids: string[] | null = null;

		try {
			const response = await this.#fetcher(`${this.#base}/favorites`, {
				credentials: 'same-origin',
				cache: 'no-store'
			});

			if (response.ok) ids = parseOrNull(favoriteIdsSchema, await response.json());
		} catch {
			// The next event or reconnect refreshes again.
		}

		if (ids === null) return;

		for (const id of [...this.ids]) {
			if (!ids.includes(id) && !this.#saving.has(id)) this.ids.delete(id);
		}
		for (const id of ids) this.ids.add(id);
	}

	async #save(id: string, selected: boolean) {
		let failure = FAILED;

		try {
			const response = await this.#fetcher(`${this.#base}/favorites/${encodeURIComponent(id)}`, {
				method: selected ? 'PUT' : 'DELETE',
				credentials: 'same-origin'
			});

			if (response.ok) {
				this.error = null;

				return;
			}
			if (response.status === 409) failure = FINAL;
		} catch {
			// Reported below like any failed save.
		}

		if (selected) this.ids.delete(id);
		else this.ids.add(id);
		this.error = failure;
	}
}
