import { describe, expect, it } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';
import {
	formatDateTime,
	formatFullDate,
	formatLongDate,
	formatShortDate,
	galleryExpiryDescription,
	galleryInitials,
	gallerySummary,
	galleryStatus,
	highlightParts
} from './format.js';

const noon = '2026-09-12T12:00:00Z';
const savedZone = process.env.TZ;

describe('dates in English', () => {
	useLocale('en');

	it('read day first and never shift YYYY-MM-DD calendar dates across time zones', () => {
		expect(formatShortDate(noon)).toBe('12 Sep');
		expect(formatLongDate(noon)).toBe('12 Sep 2026');
		expect(formatFullDate(noon, 'UTC')).toBe('September 12, 2026');
		expect(formatDateTime('2026-11-09T14:38:00Z', 'UTC')).toBe('Nov 9, 2026, 2:38 PM');

		try {
			for (const zone of ['America/New_York', 'Pacific/Auckland']) {
				process.env.TZ = zone;
				expect(formatShortDate('2026-10-31')).toBe('31 Oct');
				expect(formatLongDate('2026-10-31')).toBe('31 Oct 2026');
				expect(formatFullDate('2026-10-31')).toBe('October 31, 2026');
			}
		} finally {
			process.env.TZ = savedZone;
		}
	});
});

describe('dates in French', () => {
	useLocale('fr');

	it('follow the French conventions without shifting YYYY-MM-DD calendar dates', () => {
		expect(formatShortDate(noon)).toBe('12 sept.');
		expect(formatLongDate(noon)).toBe('12 sept. 2026');
		expect(formatFullDate(noon, 'UTC')).toBe('12 septembre 2026');
		expect(formatDateTime('2026-11-09T14:38:00Z', 'UTC')).toBe('9 nov. 2026, 14:38');

		try {
			for (const zone of ['America/New_York', 'Pacific/Auckland']) {
				process.env.TZ = zone;
				expect(formatShortDate('2026-10-31')).toBe('31 oct.');
				expect(formatLongDate('2026-10-31')).toBe('31 oct. 2026');
				expect(formatFullDate('2026-10-31')).toBe('31 octobre 2026');
			}
		} finally {
			process.env.TZ = savedZone;
		}
	});
});

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
	const base = {
		states: ['shared'] as never,
		included_photos: 10,
		photo_count: 32,
		favorite_count: 12,
		final_count: 0,
		downloaded_count: 0
	};
	const line = (gallery: Parameters<typeof gallerySummary>[0]) =>
		gallerySummary(gallery)
			.map(({ text }) => text)
			.join('');

	it('says where each status stands', () => {
		const draft = { ...base, states: ['draft'] as never, favorite_count: 0 };
		expect(line(draft)).toBe('32 photos');
		expect(line({ ...draft, photo_count: 1 })).toBe('1 photo');
		expect(line({ ...draft, photo_count: 0 })).toBe('0 photos');
		expect(line({ ...base, favorite_count: 0 })).toBe('32 photos');
		expect(line(base)).toBe('32 photos, 12\u00a0selected');
		expect(line({ ...base, states: ['delivered'] as never })).toBe('32 photos, 12\u00a0selected');
		expect(line({ ...base, states: ['delivered', 'expired'] as never })).toBe(
			'32 photos, 12\u00a0selected'
		);
		expect(line({ ...base, states: ['delivered', 'archived'] as never })).toBe(
			'32 photos, 12\u00a0selected'
		);
	});

	it('reports how many delivered finals were downloaded', () => {
		const delivered = { ...base, states: ['delivered'] as never, final_count: 20 };

		expect(line({ ...delivered, downloaded_count: 12 })).toBe(
			'32 photos, 12\u00a0selected, 20 retouched, 12\u00a0downloaded'
		);
		expect(line({ ...delivered, downloaded_count: 20 })).toBe(
			'32 photos, 12\u00a0selected, 20 retouched, 20\u00a0downloaded'
		);
		expect(line({ ...delivered, final_count: 1, downloaded_count: 0 })).toBe(
			'32 photos, 12\u00a0selected, 1 retouched, 0\u00a0downloaded'
		);
		expect(line({ ...base, final_count: 3, downloaded_count: 1 })).toBe(
			'32 photos, 12\u00a0selected, 3 retouched, 1\u00a0downloaded'
		);
	});

	it('flags the selected count only when it exceeds the included photos', () => {
		const over = (gallery: Parameters<typeof gallerySummary>[0]) =>
			gallerySummary(gallery)
				.filter((part) => part.over)
				.map(({ text }) => text);

		expect(over(base)).toEqual(['12']);
		expect(over({ ...base, included_photos: 12 })).toEqual([]);
		expect(over({ ...base, included_photos: null })).toEqual([]);
		expect(over({ ...base, favorite_count: 0, included_photos: 10 })).toEqual([]);
	});

	it('describes expiry independently of the card summary', () => {
		const now = new Date('2026-11-10T12:00:00Z');

		expect(galleryExpiryDescription('2026-11-06', now)).toBe('expired 3 days ago');
		expect(galleryExpiryDescription('2026-11-08', now)).toBe('expired yesterday');
		expect(galleryExpiryDescription('2026-11-09', now)).toBe('expired today');
		expect(galleryExpiryDescription(null, now)).toBe('expired');
	});
});

describe('gallerySummary in French', () => {
	useLocale('fr');

	const base = {
		states: ['shared'] as never,
		included_photos: 10,
		photo_count: 32,
		favorite_count: 12,
		final_count: 0,
		downloaded_count: 0
	};
	const line = (gallery: Parameters<typeof gallerySummary>[0]) =>
		gallerySummary(gallery)
			.map(({ text }) => text)
			.join('');

	it('agrees with the counts', () => {
		expect(line(base)).toBe('32 photos, 12\u00a0sélectionnées');
		expect(line({ ...base, favorite_count: 1 })).toBe('32 photos, 1\u00a0sélectionnée');
		expect(
			line({ ...base, states: ['delivered'] as never, final_count: 20, downloaded_count: 1 })
		).toBe('32 photos, 12\u00a0sélectionnées, 20 retouchées, 1\u00a0téléchargée');
	});
});
