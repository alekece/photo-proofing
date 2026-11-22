import { describe, expect, it, vi } from 'vitest';
import type { Contact } from '#lib/contacts/api.js';
import { ApiError } from '#lib/http.js';
import { linkGalleryContact, listGalleryContacts, unlinkGalleryContact } from './contacts.js';

const sample: Contact = {
	id: 'c1',
	name: 'Camille & Hugo',
	email: 'camille@example.test',
	phone: null,
	notes: null,
	galleries: [{ id: 'g1', title: 'Wedding' }],
	created_at: '2026-11-13T10:00:00Z',
	updated_at: '2026-11-13T10:00:00Z'
};

describe('gallery contacts API', () => {
	it('lists, links, and unlinks contacts on a gallery', async () => {
		const linked = { ...sample, link_id: 'link-a' };
		const listFetch = vi.fn(async () => Response.json([linked]));
		expect(await listGalleryContacts('g 1', listFetch)).toEqual([linked]);
		expect(listFetch).toHaveBeenCalledWith(
			'/api/v1/galleries/g%201/contacts',
			expect.objectContaining({ method: 'GET', credentials: 'same-origin' })
		);

		const linkFetch = vi.fn(async () => Response.json(sample));
		expect(await linkGalleryContact('g 1', 'c 1', linkFetch)).toEqual(sample);
		expect(linkFetch).toHaveBeenCalledWith(
			'/api/v1/galleries/g%201/contacts/c%201',
			expect.objectContaining({ method: 'PUT', credentials: 'same-origin' })
		);

		const unlinkFetch = vi.fn(async () => new Response(null, { status: 204 }));
		await unlinkGalleryContact('g 1', 'c 1', unlinkFetch);
		expect(unlinkFetch).toHaveBeenCalledWith(
			'/api/v1/galleries/g%201/contacts/c%201',
			expect.objectContaining({ method: 'DELETE', credentials: 'same-origin' })
		);
	});

	it('throws ApiError on failed link', async () => {
		const errFetch = vi.fn(async () => new Response('{}', { status: 404 }));
		await expect(linkGalleryContact('g1', 'c1', errFetch)).rejects.toBeInstanceOf(ApiError);
	});
});
