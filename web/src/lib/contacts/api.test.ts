import { describe, expect, it, vi } from 'vitest';
import {
	ContactApiError,
	contactInitials,
	createContact,
	deleteContact,
	linkGalleryContact,
	listContacts,
	listGalleryContacts,
	unlinkGalleryContact,
	updateContact,
	type Contact
} from './api.js';

const sample: Contact = {
	id: 'c1',
	name: 'Camille & Hugo',
	email: 'camille@example.test',
	phone: '+33 6 12 34 56 78',
	notes: 'Bride & groom',
	galleries: [{ id: 'g1', title: 'Wedding' }],
	created_at: '2026-11-13T10:00:00Z',
	updated_at: '2026-11-13T10:00:00Z'
};

describe('contactInitials', () => {
	it('builds a 1-2 character monogram skipping connectors', () => {
		expect(contactInitials('Camille & Hugo')).toBe('CH');
		expect(contactInitials('Zoé')).toBe('Z');
		expect(contactInitials('  Jean-Luc Picard  ')).toBe('JP');
	});
});

describe('contacts client API', () => {
	it('lists, creates, updates, deletes, and manages gallery links', async () => {
		const listFetch = vi.fn(async () => new Response(JSON.stringify([sample]), { status: 200 }));
		expect(await listContacts(listFetch)).toEqual([sample]);
		expect(listFetch).toHaveBeenCalledWith(
			'/api/v1/contacts',
			expect.objectContaining({ method: 'GET' })
		);

		const createFetch = vi.fn(async () => new Response(JSON.stringify(sample), { status: 201 }));
		expect(
			await createContact({ name: 'Camille & Hugo', email: 'camille@example.test' }, createFetch)
		).toEqual(sample);

		const updateFetch = vi.fn(async () => new Response(JSON.stringify(sample), { status: 200 }));
		expect(await updateContact('c1', { phone: null }, updateFetch)).toEqual(sample);

		const deleteFetch = vi.fn(async () => new Response(null, { status: 204 }));
		await deleteContact('c1', deleteFetch);
		expect(deleteFetch).toHaveBeenCalledWith(
			'/api/v1/contacts/c1',
			expect.objectContaining({ method: 'DELETE' })
		);

		expect(await listGalleryContacts('g1', listFetch)).toEqual([sample]);

		const linkFetch = vi.fn(async () => new Response(JSON.stringify(sample), { status: 200 }));
		expect(await linkGalleryContact('g1', 'c1', linkFetch)).toEqual(sample);

		const unlinkFetch = vi.fn(async () => new Response(null, { status: 204 }));
		await unlinkGalleryContact('g1', 'c1', unlinkFetch);
		expect(unlinkFetch).toHaveBeenCalledWith(
			'/api/v1/galleries/g1/contacts/c1',
			expect.objectContaining({ method: 'DELETE' })
		);
	});

	it('translates duplicate email 409 into ContactApiError with code contact_email_taken', async () => {
		const conflictFetch = vi.fn(
			async () =>
				new Response(JSON.stringify({ code: 'contact_email_taken' }), {
					status: 409,
					headers: { 'Content-Type': 'application/json' }
				})
		);

		await expect(
			createContact({ name: 'Dup', email: 'camille@example.test' }, conflictFetch)
		).rejects.toMatchObject({
			status: 409,
			code: 'contact_email_taken',
			message: 'A contact with this email already exists.'
		} satisfies Partial<ContactApiError>);
	});
});
