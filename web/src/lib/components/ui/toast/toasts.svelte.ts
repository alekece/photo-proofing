export type Toast = {
	id: number;
	kind: 'success' | 'error';
	message: string;
	retry?: () => void;
};

const DISMISS_AFTER_MS = 4000;
/** Quiet period without another save before the single "Saved" toast appears. */
export const SAVED_QUIET_MS = 3000;

class Toasts {
	items = $state<Toast[]>([]);
	#next = 0;
	#savedTimer: ReturnType<typeof setTimeout> | undefined;

	success(message: string) {
		return this.#add({ kind: 'success', message });
	}

	/** Reports a successful save; saves closer together than `SAVED_QUIET_MS` share one toast. */
	saved() {
		clearTimeout(this.#savedTimer);
		this.#savedTimer = setTimeout(() => this.success('Saved'), SAVED_QUIET_MS);
	}

	/** Errors stay until dismissed, so a Retry stays reachable. */
	error(message: string, retry?: () => void) {
		return this.#add({ kind: 'error', message, retry }, false);
	}

	dismiss(id: number) {
		this.items = this.items.filter((toast) => toast.id !== id);
	}

	#add(toast: Omit<Toast, 'id'>, autoDismiss = true) {
		const id = this.#next++;

		this.items.push({ ...toast, id });
		if (autoDismiss) setTimeout(() => this.dismiss(id), DISMISS_AFTER_MS);

		return id;
	}
}

export const toasts = new Toasts();
