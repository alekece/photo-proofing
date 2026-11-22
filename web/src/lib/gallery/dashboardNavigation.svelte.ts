import { getContext, setContext } from 'svelte';
import type { GallerySort, GalleryState } from './schemas.js';

export type GalleryGridSnapshot = { query: string; selected: GalleryState | null; scrollY: number };

const key = Symbol('gallery grid return');

export class GalleryGridReturn {
	view = $state<{ snapshot: GalleryGridSnapshot; sort: GallerySort }>();

	get href() {
		const sort = this.view?.sort;

		return sort && sort !== 'updated' ? `/dashboard?sort=${sort}` : '/dashboard';
	}
}

export function setGalleryGridReturn() {
	return setContext(key, new GalleryGridReturn());
}

export function getGalleryGridReturn(): GalleryGridReturn | undefined {
	return getContext(key);
}
