import { error, redirect } from '@sveltejs/kit';
import { contactListSchema, type Contact } from '#lib/contacts/api.js';
import { parseOrNull } from '#lib/gallery/schemas.js';
import { apiUrl } from './api';

const UNAVAILABLE = 'Contacts are temporarily unavailable. Please try again.';

/** Server-side read of workspace contacts; 401 redirects to sign-in, other failures become 503. */
export async function listWorkspaceContacts(
	fetcher: typeof fetch,
	cookie: string | null
): Promise<Contact[]> {
	const url = apiUrl('/api/v1/contacts');
	let response: Response;

	try {
		response = await fetcher(url, {
			headers: cookie ? { Cookie: cookie } : {},
			cache: 'no-store',
			redirect: 'error'
		});
	} catch {
		error(503, UNAVAILABLE);
	}

	if (response.status === 401) redirect(303, '/login');
	if (!response.ok) error(503, UNAVAILABLE);

	let body: unknown;

	try {
		body = await response.json();
	} catch {
		error(503, UNAVAILABLE);
	}

	const parsed = parseOrNull(contactListSchema, body);

	if (parsed === null) error(503, UNAVAILABLE);

	return parsed;
}
