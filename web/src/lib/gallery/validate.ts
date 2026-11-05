import type { Gallery, GalleryLayout, GallerySort } from './api';
import type { Photo, PhotoStatus, UploadLimits } from './photos';
import type { SharedGallery, SharedPhoto } from './types';

const SORTS: readonly GallerySort[] = ['updated', 'created', 'title'];
const LAYOUTS: readonly GalleryLayout[] = ['grid', 'masonry'];
const STATUSES: readonly string[] = ['waiting', 'processing', 'ready'];

type Fields = Record<string, unknown>;

function isObject(value: unknown): value is Fields {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

const isString = (value: unknown): value is string => typeof value === 'string';
const isNumber = (value: unknown): value is number => Number.isFinite(value);
const isNullable = (check: (value: unknown) => boolean) => (value: unknown) =>
	value === null || check(value);
const isLayout = (value: unknown): value is GalleryLayout =>
	LAYOUTS.includes(value as GalleryLayout);

function hasFields(value: unknown, checks: Record<string, (field: unknown) => boolean>) {
	return isObject(value) && Object.entries(checks).every(([key, check]) => check(value[key]));
}

/** Only the fields the web app reads are checked, so the API can add fields freely. */
export function isGallery(value: unknown): value is Gallery {
	return hasFields(value, {
		id: isString,
		title: isNullable(isString),
		layout: isLayout,
		expires_at: isNullable(isString),
		included_photos: isNullable(isNumber),
		watermark_enabled: (field) => typeof field === 'boolean',
		share_token: isNullable(isString),
		created_at: isString,
		updated_at: isString
	});
}

export function isSharedGallery(value: unknown): value is SharedGallery {
	return hasFields(value, {
		title: isNullable(isString),
		layout: isLayout,
		expires_at: isNullable(isString)
	});
}

export function isSharedPhoto(value: unknown): value is SharedPhoto {
	return hasFields(value, { id: isString, width: isNumber, height: isNumber });
}

function isPhotoStatus(value: unknown): value is PhotoStatus {
	return (
		(isString(value) && STATUSES.includes(value)) || (isObject(value) && isString(value.failed))
	);
}

export function isPhoto(value: unknown): value is Photo {
	return hasFields(value, {
		id: isString,
		original_name: isString,
		size: isNumber,
		width: isNumber,
		height: isNumber,
		status: isPhotoStatus,
		created_at: isString
	});
}

export function isUploadLimits(value: unknown): value is UploadLimits {
	return hasFields(value, { concurrency: isNumber, max_size: isNumber, max_pixels: isNumber });
}

export function isGallerySort(value: unknown): value is GallerySort {
	return SORTS.includes(value as GallerySort);
}

/** An unknown sort, such as one typed into the URL, means the default order. */
export function parseGallerySort(value: unknown): GallerySort {
	return isGallerySort(value) ? value : 'updated';
}

export function everyItem<T>(value: unknown, check: (item: unknown) => item is T): value is T[] {
	return Array.isArray(value) && value.every(check);
}
