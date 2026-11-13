import * as v from 'valibot';
import { parseOrNull } from '#lib/gallery/schemas.js';
import { apiErrorMessage, readResponseErrorCode } from '#lib/i18n/errors.js';

export const linkedGallerySchema = v.object({
	id: v.string(),
	title: v.nullable(v.string())
});

export const contactSchema = v.object({
	id: v.string(),
	name: v.string(),
	email: v.string(),
	phone: v.nullable(v.string()),
	notes: v.nullable(v.string()),
	galleries: v.array(linkedGallerySchema),
	created_at: v.string(),
	updated_at: v.string()
});

export const contactListSchema = v.array(contactSchema);

export type LinkedGallery = v.InferOutput<typeof linkedGallerySchema>;
export type Contact = v.InferOutput<typeof contactSchema>;

export interface NewContactInput {
	name: string;
	email: string;
	phone?: string | null;
	notes?: string | null;
}

export interface ContactChangesInput {
	name?: string;
	email?: string;
	phone?: string | null;
	notes?: string | null;
}

export class ContactApiError extends Error {
	constructor(
		public status: number,
		message: string,
		public code?: string
	) {
		super(message);
	}
}

/** Up to two uppercase monogram letters from the contact's name, skipping connectors like `&`. */
export function contactInitials(name: string): string {
	const words = name
		.trim()
		.split(/\s+/)
		.map((part) => Array.from(part).find((ch) => /\p{L}|\p{N}/u.test(ch)))
		.filter((ch): ch is string => ch !== undefined);

	if (words.length === 0) return '?';
	if (words.length === 1) return words[0].toUpperCase();

	return `${words[0]}${words[words.length - 1]}`.toUpperCase();
}

/** Alphabetical order matching the backend (`lower(name), id`). */
export function sortContacts(list: Contact[]): Contact[] {
	return [...list].sort((a, b) => {
		const byName = a.name.toLowerCase().localeCompare(b.name.toLowerCase());

		return byName !== 0 ? byName : a.id.localeCompare(b.id);
	});
}

async function requestJson<const S extends v.GenericSchema>(
	url: string,
	init: RequestInit,
	schema: S,
	fetcher: typeof fetch
): Promise<v.InferOutput<S>> {
	const headers = new Headers(init.headers);

	if (init.body !== undefined && !headers.has('Content-Type')) {
		headers.set('Content-Type', 'application/json');
	}

	const response = await fetcher(url, {
		...init,
		credentials: 'same-origin',
		headers
	});

	if (!response.ok) {
		const code = await readResponseErrorCode(response);

		throw new ContactApiError(response.status, apiErrorMessage(response.status, code), code);
	}

	const parsed = parseOrNull(schema, await response.json().catch(() => null));

	if (parsed === null) throw new ContactApiError(502, apiErrorMessage(502));

	return parsed;
}

async function requestEmpty(url: string, init: RequestInit, fetcher: typeof fetch): Promise<void> {
	const response = await fetcher(url, {
		...init,
		credentials: 'same-origin'
	});

	if (!response.ok) {
		const code = await readResponseErrorCode(response);

		throw new ContactApiError(response.status, apiErrorMessage(response.status, code), code);
	}
}

export function listContacts(fetcher: typeof fetch = fetch): Promise<Contact[]> {
	return requestJson('/api/v1/contacts', { method: 'GET' }, contactListSchema, fetcher);
}

export function createContact(
	input: NewContactInput,
	fetcher: typeof fetch = fetch
): Promise<Contact> {
	return requestJson(
		'/api/v1/contacts',
		{ method: 'POST', body: JSON.stringify(input) },
		contactSchema,
		fetcher
	);
}

export function updateContact(
	id: string,
	changes: ContactChangesInput,
	fetcher: typeof fetch = fetch
): Promise<Contact> {
	return requestJson(
		`/api/v1/contacts/${encodeURIComponent(id)}`,
		{ method: 'PATCH', body: JSON.stringify(changes) },
		contactSchema,
		fetcher
	);
}

export function deleteContact(id: string, fetcher: typeof fetch = fetch): Promise<void> {
	return requestEmpty(`/api/v1/contacts/${encodeURIComponent(id)}`, { method: 'DELETE' }, fetcher);
}

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
