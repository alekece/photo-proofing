import { describe, expect, it } from 'vitest';
import type { Contact } from './api.js';
import { contactInitials, sortContacts } from './format.js';

const makeContact = (id: string, name: string): Contact => ({
	id,
	name,
	email: `${id}@example.test`,
	phone: null,
	notes: null,
	galleries: [],
	created_at: '2026-11-13T10:00:00Z',
	updated_at: '2026-11-13T10:00:00Z'
});

describe('contactInitials', () => {
	it('builds a 1-2 character monogram skipping connectors', () => {
		expect(contactInitials('Camille & Hugo')).toBe('CH');
		expect(contactInitials('Zoé')).toBe('Z');
		expect(contactInitials('  Jean-Luc Picard  ')).toBe('JP');
		expect(contactInitials(' & - ')).toBe('?');
	});
});

describe('sortContacts', () => {
	it('orders case-insensitively by name then id without mutating input', () => {
		const input = [
			makeContact('c2', 'zoé'),
			makeContact('c3', 'Alice'),
			makeContact('c1', 'alice')
		];
		const sorted = sortContacts(input);

		expect(sorted.map((item) => item.id)).toEqual(['c1', 'c3', 'c2']);
		expect(input.map((item) => item.id)).toEqual(['c2', 'c3', 'c1']);
	});
});
