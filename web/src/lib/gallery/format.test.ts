import { describe, expect, it } from 'vitest';
import {
	galleryInitials,
	gallerySummary,
	galleryStatus,
	highlightParts,
	journeyProgress
} from './format.js';

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

describe('journeyProgress', () => {
	it('moves along the journey and ends full once delivered or archived', () => {
		expect(journeyProgress('draft')).toBeLessThan(journeyProgress('shared'));
		expect(journeyProgress('shared')).toBeLessThan(journeyProgress('delivered'));
		expect(journeyProgress('delivered')).toBe(1);
		expect(journeyProgress('archived')).toBe(1);
	});
});

describe('gallerySummary', () => {
	const now = new Date('2026-11-10T12:00:00Z');
	const base = { states: ['draft'] as never, included_photos: null, expires_at: null };

	it('says where each status stands', () => {
		expect(gallerySummary(base, now)).toBe('Not shared yet');
		expect(gallerySummary({ ...base, states: ['shared'] }, now)).toBe('Shared with your client');
		expect(gallerySummary({ ...base, states: ['shared'], included_photos: 30 }, now)).toBe(
			'Shared · 30 photos included'
		);
		expect(gallerySummary({ ...base, states: ['delivered'] }, now)).toBe('Final photos delivered');
		expect(gallerySummary({ ...base, states: ['delivered', 'archived'] }, now)).toBe('Archived');
	});

	it('says how long ago an expired gallery ended', () => {
		const expired = { ...base, states: ['shared', 'expired'] as never };

		expect(gallerySummary({ ...expired, expires_at: '2026-11-07T12:00:00Z' }, now)).toBe(
			'Expired 3 days ago'
		);
		expect(gallerySummary({ ...expired, expires_at: '2026-11-09T12:00:00Z' }, now)).toBe(
			'Expired yesterday'
		);
		expect(gallerySummary({ ...expired, expires_at: '2026-11-10T08:00:00Z' }, now)).toBe(
			'Expired today'
		);
		expect(gallerySummary(expired, now)).toBe('Expired');
	});
});
