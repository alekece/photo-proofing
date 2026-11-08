import { SvelteSet } from 'svelte/reactivity';

/** Select mode of one tab: which photos are ticked, and whether select mode is on. */
export class SelectMode {
	active = $state(false);
	readonly ids = new SvelteSet<string>();

	get count() {
		return this.ids.size;
	}

	has(id: string) {
		return this.ids.has(id);
	}

	toggle(id: string) {
		if (!this.ids.delete(id)) this.ids.add(id);
	}

	/** Picks every id given, which the tab passes from what it shows now. */
	selectAll(ids: Iterable<string>) {
		for (const id of ids) this.ids.add(id);
	}

	/** Empties the selection and stays in select mode. */
	unselectAll() {
		this.ids.clear();
	}

	enter() {
		this.active = true;
	}

	leave() {
		this.active = false;
		this.ids.clear();
	}
}
