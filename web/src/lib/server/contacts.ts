import { contactListSchema, type Contact } from '#lib/contacts/api.js';
import { parseOrNull } from '#lib/gallery/schemas.js';
import { readWorkspaceJson } from './http.js';

const UNAVAILABLE = 'Contacts are temporarily unavailable. Please try again.';

/** Server-side read of workspace contacts; 401 redirects to sign-in, other failures become 503. */
export function listWorkspaceContacts(
	fetcher: typeof fetch,
	cookie: string | null
): Promise<Contact[]> {
	return readWorkspaceJson(
		fetcher,
		cookie,
		'/api/v1/contacts',
		(body) => parseOrNull(contactListSchema, body),
		{ unavailable: UNAVAILABLE }
	);
}
