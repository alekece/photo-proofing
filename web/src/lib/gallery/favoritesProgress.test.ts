import { describe, expect, it, vi } from 'vitest';

import { FavoritesProgress } from './favoritesProgress.js';

type TestStorage = Pick<Storage, 'getItem' | 'setItem'>;

function memoryStorage(): TestStorage & { values: Map<string, string> } {
	const values = new Map<string, string>();

	return {
		values,
		getItem: vi.fn((key: string) => values.get(key) ?? null),
		setItem: vi.fn((key: string, value: string) => {
			values.set(key, value);
		})
	};
}

describe('FavoritesProgress', () => {
	it('records a canonical exact favorite set and restores it on reload', () => {
		const storage = memoryStorage();
		const progress = new FavoritesProgress(storage);

		progress.record('g1', ['p2', 'p1', 'p2']);

		expect(vi.mocked(storage.setItem)).toHaveBeenCalledOnce();
		const [, stored] = vi.mocked(storage.setItem).mock.calls[0]!;
		expect(JSON.parse(stored)).toEqual({ version: 1, photo_ids: ['p1', 'p2'] });
		expect(new FavoritesProgress(storage).matches('g1', ['p2', 'p1'])).toBe(true);
	});

	it('requires the downloaded and current favorite sets to match exactly', () => {
		const storage = memoryStorage();
		const progress = new FavoritesProgress(storage);
		progress.record('g1', ['p1', 'p2']);

		expect(progress.matches('g1', ['p1', 'p2', 'p3'])).toBe(false);
		expect(progress.matches('g1', ['p1'])).toBe(false);
		expect(progress.matches('g2', ['p1', 'p2'])).toBe(false);
	});

	it.each([
		'not json',
		JSON.stringify({ version: 2, photo_ids: ['p1'] }),
		JSON.stringify({ version: 1, photo_ids: [1] })
	])('ignores corrupt or unsupported snapshot %s', (value) => {
		const storage: TestStorage = {
			getItem: () => value,
			setItem: vi.fn()
		};

		expect(new FavoritesProgress(storage).matches('g1', ['p1'])).toBe(false);
	});

	it('contains browser storage read and write failures', () => {
		const storage: TestStorage = {
			getItem: () => {
				throw new Error('blocked');
			},
			setItem: () => {
				throw new Error('blocked');
			}
		};
		const progress = new FavoritesProgress(storage);

		expect(progress.matches('g1', ['p1'])).toBe(false);
		expect(() => progress.record('g1', ['p1'])).not.toThrow();
	});
});
