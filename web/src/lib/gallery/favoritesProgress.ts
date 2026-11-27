type FavoritesProgressStorage = Pick<Storage, 'getItem' | 'setItem'>;

interface Snapshot {
	version: 1;
	photo_ids: string[];
}

const STORAGE_PREFIX = 'photo-proofing:favorites-download:';

export class FavoritesProgress {
	readonly #storage: FavoritesProgressStorage | null;

	constructor(storage?: FavoritesProgressStorage | null) {
		if (storage !== undefined) {
			this.#storage = storage;
			return;
		}

		try {
			this.#storage = typeof window === 'undefined' ? null : window.localStorage;
		} catch {
			this.#storage = null;
		}
	}

	matches(galleryId: string, favoriteIds: readonly string[]): boolean {
		if (!this.#storage) return false;

		try {
			const raw = this.#storage.getItem(this.#key(galleryId));
			if (!raw) return false;

			const snapshot: unknown = JSON.parse(raw);
			if (!this.#valid(snapshot)) return false;

			return this.#same(this.#normalize(snapshot.photo_ids), this.#normalize(favoriteIds));
		} catch {
			return false;
		}
	}

	record(galleryId: string, favoriteIds: readonly string[]): void {
		if (!this.#storage) return;

		const snapshot: Snapshot = { version: 1, photo_ids: this.#normalize(favoriteIds) };

		try {
			this.#storage.setItem(this.#key(galleryId), JSON.stringify(snapshot));
		} catch {
			// Browser-local progress is optional and never changes the download result.
		}
	}

	#key(galleryId: string): string {
		return `${STORAGE_PREFIX}${encodeURIComponent(galleryId)}`;
	}

	#normalize(ids: readonly string[]): string[] {
		return [...new Set(ids)].sort();
	}

	#same(left: readonly string[], right: readonly string[]): boolean {
		return left.length === right.length && left.every((id, index) => id === right[index]);
	}

	#valid(snapshot: unknown): snapshot is Snapshot {
		if (typeof snapshot !== 'object' || snapshot === null) return false;

		const value = snapshot as Partial<Snapshot>;

		return (
			value.version === 1 &&
			Array.isArray(value.photo_ids) &&
			value.photo_ids.every((id) => typeof id === 'string')
		);
	}
}
