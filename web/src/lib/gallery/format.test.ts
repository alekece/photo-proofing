import { describe, expect, it } from 'vitest';
import { galleryInitials, gallerySummary, galleryStatus, highlightParts } from './format.js';

describe('galleryStatus', () => {
	it('lets archived and expired win over the progress state', () => {
		expect(galleryStatus(['shared', 'expired'])).toBe('expired');
		expect(galleryStatus(['delivered', 'expired', 'archived'])).toBe('archived');
		expect(galleryStatus(['shared'])).toBe('shared');
		expect(galleryStatus([])).toBe('draft');
	});
});

describe('galleryInitials', () => {
	it('takes up to two capitals', () => {
		expect(galleryInitials('Camille & Hugo')).toBe('CH');
		expect(galleryInitials('wedding')).toBe('W');
		expect(galleryInitials('A B C')).toBe('AB');
		expect(galleryInitials(null)).toBe('');
	});
});

describe('highlightParts', () => {
	it('marks every case-insensitive match and keeps the original casing', () => {
		expect(highlightParts('Martin Wedding, martin', 'MARTIN')).toEqual([
			{ text: 'Martin', match: true },
			{ text: ' Wedding, ', match: false },
			{ text: 'martin', match: true }
		]);
	});

	it('treats regex characters literally and an empty needle as no match', () => {
		expect(highlightParts('axb a.b', 'a.b')).toEqual([
			{ text: 'axb ', match: false },
			{ text: 'a.b', match: true }
		]);
		expect(highlightParts('Wedding', ' ')).toEqual([{ text: 'Wedding', match: false }]);
	});
});

describe('gallerySummary', () => {
	const now = new Date('2026-11-10T12:00:00Z');
	const base = {
		states: ['shared'] as never,
		included_photos: 10,
		expires_at: null,
		photo_count: 32,
		favorite_count: 12
	};
	const line = (gallery: Parameters<typeof gallerySummary>[0]) =>
		gallerySummary(gallery, now)
			.map(({ text }) => text)
			.join('');

	it('says where each status stands', () => {
		expect(line({ ...base, states: ['draft'] as never })).toBe('32 photos');
		expect(line({ ...base, states: ['draft'] as never, photo_count: 1 })).toBe('1 photo');
		expect(line({ ...base, states: ['draft'] as never, photo_count: 0 })).toBe('No photos yet');
		expect(line(base)).toBe('32 photos · 12 selected');
		expect(line({ ...base, states: ['delivered'] as never })).toBe('Final photos delivered');
		expect(line({ ...base, states: ['delivered', 'archived'] as never })).toBe(
			'32 photos · archived'
		);
	});

	it('flags the selected count only when it exceeds the included photos', () => {
		const over = (gallery: Parameters<typeof gallerySummary>[0]) =>
			gallerySummary(gallery, now)
				.filter((part) => part.over)
				.map(({ text }) => text);

		expect(over(base)).toEqual(['12']);
		expect(over({ ...base, included_photos: 12 })).toEqual([]);
		expect(over({ ...base, included_photos: null })).toEqual([]);
		expect(over({ ...base, favorite_count: 0, included_photos: 10 })).toEqual([]);
	});

	it('says how long ago an expired gallery ended', () => {
		const expired = { ...base, states: ['shared', 'expired'] as never };

		expect(line({ ...expired, expires_at: '2026-11-07T12:00:00Z' })).toBe(
			'32 photos · 12 selected · expired 3 days ago'
		);
		expect(line({ ...expired, expires_at: '2026-11-09T12:00:00Z' })).toContain('expired yesterday');
		expect(line({ ...expired, expires_at: '2026-11-10T08:00:00Z' })).toContain('expired today');
		expect(line(expired)).toMatch(/ · expired$/);
	});
});
