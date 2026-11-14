export type Toast = {
	id: number;
	kind: 'error' | 'success';
	message: string;
	retry?: () => void;
};

const SUCCESS_DURATION_MS = 4_000;

class Toasts {
	items = $state<Toast[]>([]);
	#next = 0;
	#timers = new Map<number, ReturnType<typeof setTimeout>>();

	/** Errors stay until dismissed, so a Retry stays reachable. */
	error(message: string, retry?: () => void) {
		return this.#add({ kind: 'error', message, retry });
	}

	/** Confirmations auto-dismiss after a short pause while remaining manually dismissible. */
	success(message: string) {
		const id = this.#add({ kind: 'success', message });

		clearTimeout(this.#timers.get(id));
		this.#timers.set(
			id,
			setTimeout(() => this.dismiss(id), SUCCESS_DURATION_MS)
		);

		return id;
	}

	dismiss(id: number) {
		clearTimeout(this.#timers.get(id));
		this.#timers.delete(id);
		this.items = this.items.filter((toast) => toast.id !== id);
	}

	/** An identical toast that is still visible is reused untouched. */
	#add(toast: Omit<Toast, 'id'>) {
		const existing = this.items.find(
			(item) => item.message === toast.message && item.kind === toast.kind
		);

		if (existing) return existing.id;

		const id = this.#next++;

		this.items.push({ ...toast, id });

		return id;
	}
}

export const toasts = new Toasts();
