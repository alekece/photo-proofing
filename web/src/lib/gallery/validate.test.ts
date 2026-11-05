import { describe, expect, it } from 'vitest';
import {
	isGallery,
	isGallerySort,
	isPhoto,
	isSharedGallery,
	isSharedPhoto,
	isUploadLimits,
	parseGallerySort
} from './validate';

const gallery = {
	id: 'g',
	title: null,
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	share_token: null,
	created_at: '2026-11-05T10:00:00Z',
	updated_at: '2026-11-05T10:00:00Z'
};
const photo = {
	id: 'p',
	original_name: 'a.jpg',
	size: 10,
	width: 4,
	height: 3,
	status: 'ready',
	created_at: '2026-11-05T10:00:00Z'
};

describe('validators', () => {
	it('accept the API shapes, tolerating extra fields', () => {
		expect(isGallery({ ...gallery, title: 'Wedding', extra: 1 })).toBe(true);
		expect(isPhoto({ ...photo, status: { failed: 'unreadable' } })).toBe(true);
		expect(isSharedGallery({ title: 'T', layout: 'masonry', expires_at: null })).toBe(true);
		expect(isSharedPhoto({ id: 'p', width: 4, height: 3 })).toBe(true);
		expect(isUploadLimits({ concurrency: 4, max_size: 1, max_pixels: 1 })).toBe(true);
	});

	it.each([
		['null', null],
		['a string', 'x'],
		['a wrong layout', { ...gallery, layout: 'list' }],
		['a missing id', { ...gallery, id: undefined }],
		['a numeric title', { ...gallery, title: 3 }],
		['a missing flag', { ...gallery, watermark_enabled: undefined }]
	])('reject a gallery that is %s', (_name, value) => {
		expect(isGallery(value)).toBe(false);
	});

	it.each([
		['an unknown status', { ...photo, status: 'sleeping' }],
		['a failed status without a reason', { ...photo, status: { failed: 1 } }],
		['a text width', { ...photo, width: '4' }]
	])('reject a photo with %s', (_name, value) => {
		expect(isPhoto(value)).toBe(false);
	});

	it('reject malformed shared and limit payloads', () => {
		expect(isSharedGallery({ title: 'T', layout: 'x', expires_at: null })).toBe(false);
		expect(isSharedPhoto({ id: 'p', width: 4 })).toBe(false);
		expect(isUploadLimits({ concurrency: 4, max_size: 1 })).toBe(false);
	});

	it('fall back to the default sort for unknown values', () => {
		expect(isGallerySort('title')).toBe(true);
		expect(isGallerySort('random')).toBe(false);
		expect(parseGallerySort('created')).toBe('created');
		expect(parseGallerySort('random')).toBe('updated');
		expect(parseGallerySort(null)).toBe('updated');
	});
});
