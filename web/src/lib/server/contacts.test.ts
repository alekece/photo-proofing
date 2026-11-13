import { describe, expect, it, vi } from 'vitest';
import { listWorkspaceContacts } from './contacts.js';

const sample = {
	id: 'c1',
	name: 'Camille & Hugo',
	email: 'camille@example.test',
	phone: null,
	notes: null,
	galleries: [],
	created_at: '2026-11-13T10:00:00Z',
	updated_at: '2026-11-13T10:00:00Z'
};

describe('listWorkspaceContacts', () => {
	it('forwards the session cookie and validates the payload', async () => {
		const fetcher = vi.fn(async () => new Response(JSON.stringify([sample]), { status: 200 }));
		const result = await listWorkspaceContacts(fetcher, 'pp_session=abc');

		expect(result).toEqual([sample]);
		expect(fetcher).toHaveBeenCalledWith(
			expect.stringContaining('/api/v1/contacts'),
			expect.objectContaining({
				headers: { Cookie: 'pp_session=abc' },
				cache: 'no-store'
			})
		);
	});

	it('redirects 401 responses to /login', async () => {
		const fetcher = vi.fn(async () => new Response(null, { status: 401 }));

		await expect(listWorkspaceContacts(fetcher, null)).rejects.toMatchObject({
			status: 303,
			location: '/login'
		});
	});

	it('surfaces malformed payloads or server errors as 503', async () => {
		const badPayload = vi.fn(
			async () => new Response(JSON.stringify([{ bad: true }]), { status: 200 })
		);

		await expect(listWorkspaceContacts(badPayload, 'pp_session=abc')).rejects.toMatchObject({
			status: 503
		});
	});
});
