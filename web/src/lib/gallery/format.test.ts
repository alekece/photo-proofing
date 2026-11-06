import { describe, expect, it } from 'vitest';
import { galleryInitials, galleryStatus } from './format.js';

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
