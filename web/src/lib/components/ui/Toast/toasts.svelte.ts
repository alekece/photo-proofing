export type Toast = {
	id: number;
	message: string;
	retry?: () => void;
};

class Toasts {
	items = $state<Toast[]>([]);
	#next = 0;

	/** Only errors are toasted; they stay until dismissed, so a Retry stays reachable. */
	error(message: string, retry?: () => void) {
		return this.#add({ message, retry });
	}

	dismiss(id: number) {
		this.items = this.items.filter((toast) => toast.id !== id);
	}

	/** An identical toast that is still visible is reused untouched. */
	#add(toast: Omit<Toast, 'id'>) {
		const existing = this.items.find((item) => item.message === toast.message);

		if (existing) return existing.id;

		const id = this.#next++;

		this.items.push({ ...toast, id });

		return id;
	}
}

export const toasts = new Toasts();
