export type Toast = {
	id: number;
	kind: 'success' | 'error';
	message: string;
	retry?: () => void;
};

const DISMISS_AFTER_MS = 4000;

class Toasts {
	items = $state<Toast[]>([]);
	#next = 0;

	success(message: string) {
		return this.#add({ kind: 'success', message });
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
