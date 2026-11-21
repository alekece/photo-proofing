import * as v from 'valibot';
import { ApiError, requestEmpty, requestJson } from '#lib/http.js';

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
	expected_email?: string;
	phone?: string | null;
	notes?: string | null;
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

export async function deleteContact(id: string, fetcher: typeof fetch = fetch): Promise<void> {
	try {
		await requestEmpty(`/api/v1/contacts/${encodeURIComponent(id)}`, { method: 'DELETE' }, fetcher);
	} catch (error) {
		if (!(error instanceof ApiError) || error.status !== 404) throw error;
	}
}
