import * as v from 'valibot';
import { describe, expect, it } from 'vitest';
import {
	deletedPhotosSchema,
	gallerySchema,
	parseGalleryStates,
	parseGallerySort,
	parseOrNull,
	photoEventSchema,
	photoSchema,
	sharedGallerySchema,
	sharedPhotoSchema,
	uploadLimitsSchema
} from './schemas';

const gallery = {
	id: 'g',
	title: null,
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	share_token: null,
	states: ['draft'],
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
	favorite: false,
	created_at: '2026-11-05T10:00:00Z'
};

describe('schemas', () => {
	it('accept the API shapes and drop fields the app does not read', () => {
		expect(parseOrNull(gallerySchema, { ...gallery, title: 'Wedding', extra: 1 })).toEqual({
			...gallery,
			title: 'Wedding'
		});
		expect(parseOrNull(photoSchema, { ...photo, status: { failed: 'unreadable' } })).not.toBeNull();
		expect(
			parseOrNull(sharedGallerySchema, {
				title: 'T',
				layout: 'masonry',
				expires_at: null,
				included_photos: null,
				delivered: false
			})
		).not.toBeNull();
		expect(parseOrNull(sharedPhotoSchema, { id: 'p', width: 4, height: 3 })).not.toBeNull();
		expect(
			parseOrNull(uploadLimitsSchema, { concurrency: 4, max_size: 1, max_pixels: 1 })
		).not.toBeNull();
		expect(parseOrNull(deletedPhotosSchema, { deleted: 2 })).toEqual({ deleted: 2 });
		expect(
			parseOrNull(photoEventSchema, { gallery_id: 'g', photo_id: 'p', status: 'deleted' })
		).not.toBeNull();
	});

	it('accept lists of shapes', () => {
		expect(parseOrNull(v.array(photoSchema), [photo, photo])).toHaveLength(2);
	});

	it.each([
		['null', null],
		['a string', 'x'],
		['a wrong layout', { ...gallery, layout: 'list' }],
		['a missing id', { ...gallery, id: undefined }],
		['a numeric title', { ...gallery, title: 3 }],
		['a missing flag', { ...gallery, watermark_enabled: undefined }],
		['missing states', { ...gallery, states: undefined }],
		['an unknown state', { ...gallery, states: ['draft', 'sleeping'] }]
	])('reject a gallery that is %s', (_name, value) => {
		expect(parseOrNull(gallerySchema, value)).toBeNull();
	});

	it.each([
		['an unknown status', { ...photo, status: 'sleeping' }],
		['a failed status without a reason', { ...photo, status: { failed: 1 } }],
		['a text width', { ...photo, width: '4' }]
	])('reject a photo with %s', (_name, value) => {
		expect(parseOrNull(photoSchema, value)).toBeNull();
	});

	it('reject a list holding one bad item', () => {
		expect(parseOrNull(v.array(photoSchema), [photo, { ...photo, width: '4' }])).toBeNull();
	});

	it('reject malformed shared, limit, delete and event payloads', () => {
		expect(
			parseOrNull(sharedGallerySchema, { title: 'T', layout: 'x', expires_at: null })
		).toBeNull();
		expect(parseOrNull(sharedPhotoSchema, { id: 'p', width: 4 })).toBeNull();
		expect(parseOrNull(uploadLimitsSchema, { concurrency: 4, max_size: 1 })).toBeNull();
		expect(parseOrNull(deletedPhotosSchema, { deleted: 1.5 })).toBeNull();
		expect(parseOrNull(deletedPhotosSchema, null)).toBeNull();
		expect(
			parseOrNull(photoEventSchema, { gallery_id: 'g', photo_id: 'p', status: 'sleeping' })
		).toBeNull();
	});

	it('fall back to the default sort for unknown values', () => {
		expect(parseGallerySort('title')).toBe('title');
		expect(parseGallerySort('created')).toBe('created');
		expect(parseGallerySort('random')).toBe('updated');
		expect(parseGallerySort(null)).toBe('updated');
	});
});

describe('parseGalleryStates', () => {
	it('keeps valid states once, in order, and drops the rest', () => {
		expect(parseGalleryStates(['shared', 'bogus', 'shared', 'expired', null])).toEqual([
			'shared',
			'expired'
		]);
		expect(parseGalleryStates([])).toEqual([]);
	});
});
