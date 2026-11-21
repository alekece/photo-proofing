import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';
import type { Contact } from '#lib/contacts/api.js';

vi.mock('$app/navigation', () => ({
	afterNavigate: vi.fn(),
	beforeNavigate: vi.fn(),
	goto: vi.fn()
}));

import Page from './+page.svelte';

const contact = (over: Partial<Contact> = {}): Contact => ({
	id: 'c1',
	name: 'Camille & Hugo',
	email: 'camille@example.test',
	phone: '+33 6 12 34 56 78',
	notes: 'Prefers matte paper',
	galleries: [
		{ id: 'g1', title: 'Summer Wedding' },
		{ id: 'g2', title: null }
	],
	created_at: '2026-11-13T10:00:00Z',
	updated_at: '2026-11-13T10:00:00Z',
	...over
});

function open(contacts: Contact[] | null) {
	return render(Page, {
		props: {
			data: {
				identity: { account_id: 'a', workspace_id: 'w' },
				idleTimeoutMs: 2592000000,
				contacts
			} as never
		}
	});
}

beforeEach(() => {
	vi.stubGlobal('matchMedia', () => ({ matches: true }));
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('/contacts page', () => {
	it('deletes only the selected contacts after explicit confirmation', async () => {
		const user = userEvent.setup();
		const fetcher = vi.fn<typeof fetch>(async () => new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetcher);
		open([contact(), contact({ id: 'c2', name: 'Zoé Laurent' })]);

		expect(screen.queryAllByRole('button', { name: /Delete/ })).toHaveLength(0);
		expect(screen.getByRole('checkbox', { name: 'Select Camille & Hugo' })).toHaveProperty(
			'checked',
			false
		);
		await user.click(screen.getByRole('checkbox', { name: 'Select Camille & Hugo' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));
		const dialog = screen.getByRole('dialog', { name: 'Delete Camille & Hugo?' });
		expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(0);
		await user.click(within(dialog).getByRole('button', { name: 'Delete' }));

		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/contacts/c1',
			expect.objectContaining({ method: 'DELETE' })
		);
		expect(screen.queryByText('Camille & Hugo')).toBeNull();
		expect(screen.getByText('Zoé Laurent')).toBeTruthy();
	});

	it('retries a partial deletion and continues past a contact that is already gone', async () => {
		const user = userEvent.setup();
		const requests: string[] = [];
		vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) => {
			if (init?.method !== 'DELETE') return new Response(null, { status: 204 });

			requests.push(String(input));
			return new Response(null, { status: [204, 503, 404, 204][requests.length - 1] ?? 500 });
		});
		open([
			contact(),
			contact({ id: 'c2', name: 'Louis Laurent' }),
			contact({ id: 'c3', name: 'Zoé Laurent' })
		]);

		await user.click(screen.getByRole('checkbox', { name: 'Select Camille & Hugo' }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));
		let dialog = screen.getByRole('dialog', { name: 'Delete 3 contacts?' });
		await user.click(within(dialog).getByRole('button', { name: 'Delete' }));
		expect((await screen.findByRole('alert')).textContent).toContain(
			'Deleted 1 of 3 contacts. Please try again.'
		);
		expect(screen.getByText('2 selected')).toBeTruthy();
		expect(screen.queryByText('Camille & Hugo')).toBeNull();
		dialog = screen.getByRole('dialog', { name: 'Delete 2 contacts?' });
		await user.click(within(dialog).getByRole('button', { name: 'Delete' }));

		expect(requests).toEqual([
			'/api/v1/contacts/c1',
			'/api/v1/contacts/c2',
			'/api/v1/contacts/c2',
			'/api/v1/contacts/c3'
		]);
		expect(screen.getByText('No contacts yet.')).toBeTruthy();
	});

	it('returns keyboard focus to search when all contacts are unselected', async () => {
		const user = userEvent.setup();
		open([contact()]);

		await user.click(screen.getByRole('checkbox', { name: 'Select Camille & Hugo' }));
		screen.getByRole('button', { name: 'Unselect all' }).focus();
		await user.keyboard('{Enter}');

		expect(document.activeElement).toBe(screen.getByRole('searchbox'));
	});

	it('cancels deletion without writes and drops contacts hidden by search from the selection', async () => {
		const user = userEvent.setup();
		const fetcher = vi.fn<typeof fetch>(async () => new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetcher);
		open([contact(), contact({ id: 'c2', name: 'Zoé Laurent' })]);

		await user.click(screen.getByRole('checkbox', { name: 'Select Camille & Hugo' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));
		const dialog = screen.getByRole('dialog', { name: 'Delete Camille & Hugo?' });
		await user.click(within(dialog).getByRole('button', { name: 'Cancel' }));
		expect(screen.getByText('1 selected')).toBeTruthy();
		await user.type(screen.getByRole('searchbox'), 'zoé');
		expect(screen.queryByText('1 selected')).toBeNull();
		expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(0);
	});

	it('renders the empty directory state with disabled search and no axe violations', async () => {
		const { container } = open([]);

		expect(screen.getByRole('heading', { level: 1, name: 'Contacts' })).toBeTruthy();
		expect(screen.getByText('No contacts yet.')).toBeTruthy();
		expect(screen.getAllByRole('button', { name: 'New contact' })).toHaveLength(1);
		const search = screen.getByRole('searchbox', {
			name: 'Search by name, email or phone'
		}) as HTMLInputElement;
		expect(search.disabled).toBe(true);
		expect((await axe(container)).violations).toEqual([]);
	});

	it('shows contact details and linked galleries without opening a row', async () => {
		const user = userEvent.setup();
		const camille = contact();
		const zoe = contact({
			id: 'c2',
			name: 'Zoé Laurent',
			email: 'zoe@studio.test',
			phone: '0601020304',
			notes: null,
			galleries: []
		});
		const { container } = open([camille, zoe]);

		const row = screen.getByText(camille.name).closest('li')!;
		expect(within(row).getByText(camille.email)).toBeTruthy();
		expect(within(row).getByText(camille.phone!)).toBeTruthy();
		expect(within(row).getByText('Prefers matte paper')).toBeTruthy();
		expect(screen.queryByLabelText('Name')).toBeNull();
		expect(within(row).getByRole('link', { name: 'Summer Wedding' }).getAttribute('href')).toBe(
			'/dashboard/g1'
		);
		expect(within(row).getByRole('link', { name: 'Untitled gallery' }).getAttribute('href')).toBe(
			'/dashboard/g2'
		);
		expect(within(row).queryByRole('button', { name: /show details/i })).toBeNull();

		const search = screen.getByRole('searchbox', { name: 'Search by name, email or phone' });
		await user.type(search, '0601');

		const list = screen.getByRole('list', { name: 'Workspace contacts' });
		expect(within(list).queryByText('Camille & Hugo')).toBeNull();
		expect(within(list).getByText('Zoé Laurent')).toBeTruthy();

		await user.clear(search);
		await user.type(search, 'nobody-here');
		expect(screen.getByText('No contact matching “nobody-here”')).toBeTruthy();
		await user.click(screen.getByRole('button', { name: 'Clear search' }));
		expect(
			within(screen.getByRole('list', { name: 'Workspace contacts' })).getByText('Camille & Hugo')
		).toBeTruthy();

		await user.type(search, 'no match');
		await user.click(screen.getByRole('button', { name: 'Reset' }));
		expect(search).toHaveProperty('value', '');
		expect(screen.getByRole('list', { name: 'Workspace contacts' })).toBeTruthy();

		expect((await axe(container)).violations).toEqual([]);
	});

	it('sorts the full-width directory toolbar without clearing its search filter', async () => {
		const user = userEvent.setup();
		const { container } = open([
			contact(),
			contact({ id: 'c2', name: 'Zoé Laurent', email: 'zoe@example.test' }),
			contact({ id: 'c3', name: 'Alex Martin', email: 'alex@other.test' })
		]);
		const search = screen.getByRole('searchbox', { name: 'Search by name, email or phone' });
		const sort = screen.getByRole('button', { name: /^Sort by:/ });
		const toolbar = container.querySelector('[data-slot="workspace-toolbar"]') as HTMLElement;
		expect(toolbar.contains(search)).toBe(true);
		expect(toolbar.contains(sort)).toBe(true);
		expect(toolbar.className).toContain('w-full');
		expect(toolbar.className).not.toContain('max-w-');
		const names = () =>
			within(screen.getByRole('list', { name: 'Workspace contacts' }))
				.getAllByRole('listitem')
				.map((row) => row.textContent);

		expect(names()[0]).toContain('Alex Martin');
		await user.type(search, 'example.test');
		await user.click(sort);
		await user.click(screen.getByRole('menuitemradio', { name: 'Name Z–A' }));
		expect(names()).toHaveLength(2);
		expect(names()[0]).toContain('Zoé Laurent');
		expect(names()[1]).toContain('Camille & Hugo');
		expect((search as HTMLInputElement).value).toBe('example.test');
		await user.click(sort);
		await user.click(screen.getByRole('menuitemradio', { name: 'Name A–Z' }));
		expect(names()[0]).toContain('Camille & Hugo');
	});

	it('opens a blank New contact dialog and restores focus without saving on cancel', async () => {
		const user = userEvent.setup();
		const fetcher = vi.fn<typeof fetch>(async () => new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetcher);
		open([contact()]);
		const search = screen.getByRole('searchbox');
		await user.type(search, 'no match');

		const newContact = screen.getByRole('button', { name: 'New contact' });
		await user.click(newContact);
		const dialog = within(screen.getByRole('dialog', { name: 'New contact' }));
		expect(document.activeElement).toBe(dialog.getByLabelText('Name'));
		expect(dialog.getByLabelText('Name')).toHaveProperty('value', '');
		await user.click(dialog.getByRole('button', { name: 'Create contact' }));
		expect(screen.getByRole('alert').textContent).toBe('Enter a name.');
		await user.type(screen.getByLabelText('Name'), 'Unsaved');
		await user.click(screen.getByRole('button', { name: 'Cancel' }));

		expect(screen.queryByLabelText('Name')).toBeNull();
		expect(document.activeElement).toBe(newContact);
		expect(search).toHaveProperty('value', 'no match');
		expect(fetcher.mock.calls.every(([url]) => !String(url).includes('/contacts'))).toBe(true);
	});

	it('creates a new contact and handles duplicate-email 409 conflicts', async () => {
		const user = userEvent.setup();
		const created = contact({
			id: 'c3',
			name: 'Léo Martin',
			email: 'leo@example.test',
			phone: null,
			notes: null,
			galleries: []
		});

		let attempts = 0;
		const fetcher = vi.fn(async (input: RequestInfo | URL) => {
			const url = typeof input === 'string' ? input : input.toString();
			if (url.includes('/auth/')) return new Response(null, { status: 204 });
			attempts += 1;
			if (attempts === 1) {
				return new Response(JSON.stringify({ code: 'contact_email_taken' }), {
					status: 409,
					headers: { 'Content-Type': 'application/json' }
				});
			}
			return new Response(JSON.stringify(created), { status: 201 });
		});
		vi.stubGlobal('fetch', fetcher);

		open([]);

		await user.click(screen.getByRole('button', { name: 'New contact' }));
		await user.type(screen.getByLabelText('Name'), 'Léo Martin');
		await user.type(screen.getByLabelText('Email'), 'leo@example.test');
		await user.click(screen.getByRole('button', { name: 'Create contact' }));

		expect(await screen.findByText('A contact with this email already exists.')).toBeTruthy();
		expect(screen.getByLabelText('Email')).toHaveProperty('value', 'leo@example.test');

		await user.click(screen.getByRole('button', { name: 'Create contact' }));
		expect(
			await within(screen.getByRole('list', { name: 'Workspace contacts' })).findByText(
				'Léo Martin'
			)
		).toBeTruthy();
		await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New contact' })).toBeNull());
	});

	it('keeps the edit dialog open during a pending save', async () => {
		const user = userEvent.setup();
		const camille = contact();
		let resolvePatch: (value: Response) => void = () => {};
		const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			const url = typeof input === 'string' ? input : input.toString();
			if (url.includes('/auth/')) return new Response(null, { status: 204 });
			if (init?.method === 'PATCH') {
				return new Promise<Response>((resolve) => {
					resolvePatch = resolve;
				});
			}
			return new Response(null, { status: 204 });
		});
		vi.stubGlobal('fetch', fetcher);

		open([camille]);

		await user.click(screen.getByRole('button', { name: 'Edit Camille & Hugo' }));
		const modal = screen.getByRole('dialog', { name: 'Edit Camille & Hugo' });
		const nameInput = screen.getByLabelText('Name') as HTMLInputElement;
		await user.clear(nameInput);
		await user.type(nameInput, 'Camille Laurent');
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		await user.keyboard('{Escape}');
		expect(modal.isConnected).toBe(true);
		expect(within(modal).getByRole('button', { name: 'Cancel' })).toHaveProperty('disabled', true);

		resolvePatch(
			new Response(JSON.stringify(contact({ name: 'Camille Laurent', phone: null })), {
				status: 200
			})
		);
		expect(await screen.findByText('Camille Laurent')).toBeTruthy();

		await waitFor(() => expect(modal.isConnected).toBe(false));
	});

	it('edits an existing contact and restores focus on cancel', async () => {
		const user = userEvent.setup();
		const initial = contact();
		const updated = contact({ name: 'Camille Laurent', phone: null });
		const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			const url = typeof input === 'string' ? input : input.toString();
			if (url.includes('/auth/')) return new Response(null, { status: 204 });
			if (init?.method === 'PATCH') return new Response(JSON.stringify(updated), { status: 200 });
			return new Response(null, { status: 204 });
		});
		vi.stubGlobal('fetch', fetcher);

		open([initial]);

		await user.click(screen.getByRole('button', { name: 'Edit Camille & Hugo' }));
		const modal = screen.getByRole('dialog', { name: 'Edit Camille & Hugo' });
		const nameInput = within(modal).getByLabelText('Name') as HTMLInputElement;
		const phoneInput = within(modal).getByLabelText('Phone') as HTMLInputElement;
		expect(nameInput.value).toBe('Camille & Hugo');

		await user.clear(nameInput);
		await user.type(nameInput, 'Camille Laurent');
		await user.clear(phoneInput);
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		expect(await screen.findByText('Camille Laurent')).toBeTruthy();
		await waitFor(() => expect(modal.isConnected).toBe(false));
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/contacts/c1',
			expect.objectContaining({
				method: 'PATCH',
				body: JSON.stringify({ name: 'Camille Laurent', phone: null })
			})
		);
		const edit = screen.getByRole('button', { name: 'Edit Camille Laurent' });
		await user.click(edit);
		await user.keyboard('{Escape}');
		expect(screen.queryByLabelText('Name')).toBeNull();
		expect(document.activeElement).toBe(
			screen.getByRole('button', { name: 'Edit Camille Laurent' })
		);
	});

	it('puts edit actions after fields in keyboard order', async () => {
		const user = userEvent.setup();
		open([contact()]);
		await user.click(screen.getByRole('button', { name: 'Edit Camille & Hugo' }));
		await user.type(screen.getByLabelText('Name'), ' Martin');
		screen.getByLabelText('Notes').focus();
		await user.tab();
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Cancel' }));
		await user.tab();
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Save changes' }));
	});

	it('pairs an explicit recipient change with the email shown when Edit opened', async () => {
		const user = userEvent.setup();
		const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) =>
			String(input).endsWith('/contacts/c1') && init?.method === 'PATCH'
				? new Response(JSON.stringify({ code: 'contact_email_changed' }), { status: 409 })
				: new Response(null, { status: 204 })
		);
		vi.stubGlobal('fetch', fetcher);
		open([contact()]);
		await user.click(screen.getByRole('button', { name: 'Edit Camille & Hugo' }));
		const email = screen.getByLabelText('Email');
		await user.clear(email);
		await user.type(email, 'new@example.test');
		await user.click(screen.getByRole('button', { name: 'Save changes' }));

		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/contacts/c1',
			expect.objectContaining({
				method: 'PATCH',
				body: JSON.stringify({ email: 'new@example.test', expected_email: 'camille@example.test' })
			})
		);
		expect(await screen.findByRole('alert')).toHaveProperty(
			'textContent',
			'This contact’s email has changed. Reload Contacts and try again.'
		);
		expect(email).toHaveProperty('value', 'new@example.test');
		expect(screen.getByRole('dialog', { name: 'Edit Camille & Hugo' })).toBeTruthy();
	});
});

describe('/contacts page in French', () => {
	useLocale('fr');

	it('renders localized French copy and passes axe', async () => {
		const { container } = open([contact()]);

		expect(screen.getByRole('heading', { level: 1, name: 'Contacts' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Nouveau contact' })).toBeTruthy();
		await userEvent.setup().click(screen.getByRole('button', { name: 'Modifier Camille & Hugo' }));
		expect(screen.getByLabelText('Nom')).toBeTruthy();
		expect(screen.getByLabelText('Téléphone')).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});
});
