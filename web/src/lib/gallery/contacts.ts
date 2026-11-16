import { contactListSchema, contactSchema, type Contact } from '#lib/contacts/api.js';
import { requestEmpty, requestJson } from '#lib/http.js';

export function listGalleryContacts(
	galleryId: string,
	fetcher: typeof fetch = fetch
): Promise<Contact[]> {
	return requestJson(
		`/api/v1/galleries/${encodeURIComponent(galleryId)}/contacts`,
		{ method: 'GET' },
		contactListSchema,
		fetcher
	);
}

export function linkGalleryContact(
	galleryId: string,
	contactId: string,
	fetcher: typeof fetch = fetch
): Promise<Contact> {
	return requestJson(
		`/api/v1/galleries/${encodeURIComponent(galleryId)}/contacts/${encodeURIComponent(contactId)}`,
		{ method: 'PUT' },
		contactSchema,
		fetcher
	);
}

export function unlinkGalleryContact(
	galleryId: string,
	contactId: string,
	fetcher: typeof fetch = fetch
): Promise<void> {
	return requestEmpty(
		`/api/v1/galleries/${encodeURIComponent(galleryId)}/contacts/${encodeURIComponent(contactId)}`,
		{ method: 'DELETE' },
		fetcher
	);
}
