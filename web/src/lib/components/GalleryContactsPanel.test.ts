import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';
import type { Contact } from '#lib/contacts/api.js';
import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
import GalleryContactsPanel from './GalleryContactsPanel.svelte';

const camille: Contact & { link_id: string } = {
	link_id: 'link-a',
	id: 'c1',
	name: 'Camille & Hugo',
	email: 'camille@example.test',
	phone: '+33 6 12 34 56 78',
	notes: 'Prefers a warmer edit.',
	galleries: [{ id: 'g1', title: 'Wedding' }],
	created_at: '2026-11-13T10:00:00Z',
	updated_at: '2026-11-13T10:00:00Z'
};

const zoe: Contact & { link_id: string } = {
	link_id: 'link-b',
	id: 'c2',
	name: 'Zoé Laurent',
	email: 'zoe@example.test',
	phone: null,
	notes: null,
	galleries: [],
	created_at: '2026-11-13T10:00:00Z',
	updated_at: '2026-11-13T10:00:00Z'
};

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});

	return { promise, resolve };
}

function fixture(directory: Contact[], linkedIds: string[] = []) {
	const contacts = directory.map((contact) => ({ ...contact }));
	const linked = new Set(linkedIds);
	const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
		const association = String(url).match(/\/galleries\/g1\/contacts\/([^/]+)$/);
		if (String(url) === '/api/v1/contacts' && (!init || init.method === 'GET')) {
			return Response.json(contacts);
		}
		if (String(url) === '/api/v1/galleries/g1/contacts') {
			return Response.json(
				contacts
					.filter(({ id }) => linked.has(id))
					.map((contact) => ({ ...contact, link_id: `link-${contact.id}` }))
			);
		}
		if (association && init?.method === 'PUT') {
			linked.add(association[1]);
			return Response.json(contacts.find(({ id }) => id === association[1]));
		}
		if (association && init?.method === 'DELETE') {
			linked.delete(association[1]);
			return new Response(null, { status: 204 });
		}

		return new Response(null, { status: 404 });
	});

	vi.stubGlobal('fetch', fetcher);

	return { fetcher, contacts, linked };
}

afterEach(() => {
	vi.unstubAllGlobals();
	toasts.items = [];
});

describe('GalleryContactsPanel', () => {
	it('applies the associations toggled in Manage contacts only when Done is clicked', async () => {
		const user = userEvent.setup();
		const oncount = vi.fn();
		const { fetcher } = fixture([camille, zoe], ['c1']);
		const writes = () =>
			fetcher.mock.calls.filter(([, init]) => init?.method === 'PUT' || init?.method === 'DELETE');
		render(GalleryContactsPanel, { galleryId: 'g1', oncount });

		expect(screen.queryByRole('heading', { name: 'Contacts', level: 2 })).toBeNull();
		expect(await screen.findByText('Camille & Hugo')).toBeTruthy();
		expect(screen.queryByText('Zoé Laurent')).toBeNull();

		await user.click(screen.getByRole('button', { name: 'Manage contacts' }));
		const dialog = within(screen.getByRole('dialog', { name: 'Link a contact' }));
		const camilleSwitch = dialog.getByRole('switch', { name: 'Link Camille & Hugo' });
		const zoeSwitch = dialog.getByRole('switch', { name: 'Link Zoé Laurent' });
		expect(camilleSwitch.getAttribute('aria-checked')).toBe('true');
		expect(zoeSwitch.getAttribute('aria-checked')).toBe('false');

		await user.click(zoeSwitch);
		await user.click(camilleSwitch);
		expect(zoeSwitch.getAttribute('aria-checked')).toBe('true');
		expect(camilleSwitch.getAttribute('aria-checked')).toBe('false');
		expect(writes()).toHaveLength(0);

		await user.click(camilleSwitch);
		await user.click(dialog.getByRole('button', { name: 'Done' }));
		await waitFor(() =>
			expect(fetcher).toHaveBeenCalledWith(
				'/api/v1/galleries/g1/contacts/c2',
				expect.objectContaining({ method: 'PUT' })
			)
		);
		expect(writes()).toHaveLength(1);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(await screen.findByText('Zoé Laurent')).toBeTruthy();
		expect(oncount).toHaveBeenLastCalledWith(2);
	});

	it('discards the toggled associations when Manage contacts is cancelled', async () => {
		const user = userEvent.setup();
		const { fetcher } = fixture([camille, zoe], ['c1']);
		render(GalleryContactsPanel, { galleryId: 'g1' });

		await screen.findByText('Camille & Hugo');
		await user.click(screen.getByRole('button', { name: 'Manage contacts' }));
		const dialog = within(screen.getByRole('dialog', { name: 'Link a contact' }));
		await user.click(dialog.getByRole('switch', { name: 'Link Zoé Laurent' }));
		await user.click(dialog.getByRole('button', { name: 'Cancel' }));

		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(
			fetcher.mock.calls.filter(([, init]) => init?.method === 'PUT' || init?.method === 'DELETE')
		).toHaveLength(0);
		expect(screen.queryByText('Zoé Laurent')).toBeNull();
	});

	it('searches the linked contacts and unlinks the selection, removing it from the list', async () => {
		const user = userEvent.setup();
		const oncount = vi.fn();
		const { fetcher } = fixture([camille, zoe], ['c1', 'c2']);
		render(GalleryContactsPanel, { galleryId: 'g1', oncount });

		await screen.findByText('Zoé Laurent');
		const search = screen.getByRole('searchbox', { name: 'Search by name, email or phone' });
		await user.type(search, 'ZOE');
		expect(screen.queryByText('Camille & Hugo')).toBeNull();
		expect(screen.getByText('Zoé Laurent')).toBeTruthy();
		await user.clear(search);
		await user.click(screen.getByRole('checkbox', { name: 'Select Camille & Hugo' }));
		await user.click(screen.getByRole('button', { name: 'Unlink selected (1)' }));
		await waitFor(() =>
			expect(fetcher).toHaveBeenCalledWith(
				'/api/v1/galleries/g1/contacts/c1',
				expect.objectContaining({ method: 'DELETE' })
			)
		);

		await waitFor(() => expect(screen.queryByText('Camille & Hugo')).toBeNull());
		expect(screen.getByText('Zoé Laurent')).toBeTruthy();
		expect(oncount).toHaveBeenLastCalledWith(1);
	});

	it('creates a new contact and links it in one action without closing on association failure', async () => {
		const user = userEvent.setup();
		const created: Contact = {
			id: 'c3',
			name: 'Léo Martin',
			email: 'leo@example.test',
			phone: null,
			notes: null,
			galleries: [],
			created_at: '2026-11-13T10:00:00Z',
			updated_at: '2026-11-13T10:00:00Z'
		};
		const directory: Contact[] = [];
		const linked = new Set<string>();
		let associationAttempts = 0;
		const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
			if (url === '/api/v1/contacts' && (!init || init.method === 'GET')) {
				return Response.json(directory);
			}
			if (url === '/api/v1/galleries/g1/contacts') return Response.json([]);
			if (url === '/api/v1/contacts' && init?.method === 'POST') {
				directory.push(created);
				return Response.json(created, { status: 201 });
			}
			if (url === '/api/v1/contacts/c3' && init?.method === 'PATCH') {
				const changes = JSON.parse(String(init.body));
				Object.assign(created, changes);
				return Response.json(created);
			}
			if (url === '/api/v1/galleries/g1/contacts/c3' && init?.method === 'PUT') {
				associationAttempts++;
				if (associationAttempts === 1) return Response.json({}, { status: 503 });
				linked.add('c3');
				return Response.json({ ...created, galleries: [{ id: 'g1', title: 'Wedding' }] });
			}

			return new Response(null, { status: 404 });
		});
		vi.stubGlobal('fetch', fetcher);
		render(GalleryContactsPanel, { galleryId: 'g1' });

		await screen.findByText('No contacts yet.');
		await user.click(screen.getByRole('button', { name: 'New contact' }));
		const dialog = screen.getByRole('dialog', { name: 'New contact' });
		await user.type(within(dialog).getByLabelText('Name'), created.name);
		await user.type(within(dialog).getByLabelText('Email'), created.email);
		await user.click(within(dialog).getByRole('button', { name: 'Create & link' }));

		expect(await within(dialog).findByRole('alert')).toBeTruthy();
		expect(fetcher.mock.calls.filter(([url]) => url === '/api/v1/contacts')).toHaveLength(2);
		await waitFor(() =>
			expect(within(dialog).getByRole('button', { name: 'Create & link' })).toBeTruthy()
		);
		const name = within(dialog).getByLabelText('Name');
		await user.clear(name);
		await user.type(name, 'Léo Martin-Rivage');
		await user.click(within(dialog).getByRole('button', { name: 'Create & link' }));
		await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New contact' })).toBeNull());
		expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'POST')).toHaveLength(1);
		expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'PATCH')).toHaveLength(1);
		expect(linked).toEqual(new Set(['c3']));
		expect(screen.getByText('Léo Martin-Rivage')).toBeTruthy();
	});

	it('keeps the create dialog non-dismissible while its contact is linking', async () => {
		const user = userEvent.setup();
		const linking = deferred<Response>();
		const created: Contact = {
			...zoe,
			id: 'c3',
			name: 'Léo Martin',
			email: 'leo@example.test',
			galleries: []
		};
		const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
			if (url === '/api/v1/contacts' && (!init || init.method === 'GET')) return Response.json([]);
			if (url === '/api/v1/galleries/g1/contacts') return Response.json([]);
			if (url === '/api/v1/contacts' && init?.method === 'POST') {
				return Response.json(created, { status: 201 });
			}
			if (url === '/api/v1/galleries/g1/contacts/c3' && init?.method === 'PUT') {
				return linking.promise;
			}
			return new Response(null, { status: 404 });
		});
		vi.stubGlobal('fetch', fetcher);
		render(GalleryContactsPanel, { galleryId: 'g1' });

		await screen.findByText('No contacts yet.');
		await user.click(screen.getByRole('button', { name: 'New contact' }));
		const dialog = screen.getByRole('dialog', { name: 'New contact' });
		await user.type(within(dialog).getByLabelText('Name'), created.name);
		await user.type(within(dialog).getByLabelText('Email'), created.email);
		await user.click(within(dialog).getByRole('button', { name: 'Create & link' }));
		await waitFor(() =>
			expect(
				fetcher.mock.calls.some(
					([url, init]) => url === '/api/v1/galleries/g1/contacts/c3' && init?.method === 'PUT'
				)
			).toBe(true)
		);
		expect(
			(within(dialog).getByRole('button', { name: 'Close' }) as HTMLButtonElement).disabled
		).toBe(true);
		linking.resolve(Response.json(created));
		await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New contact' })).toBeNull());
	});

	it('quick-edits several selected contacts and keeps them linked', async () => {
		const user = userEvent.setup();
		const directory = [camille, zoe];
		const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
			if (url === '/api/v1/contacts' && (!init || init.method === 'GET')) {
				return Response.json(directory);
			}
			if (url === '/api/v1/galleries/g1/contacts') {
				return Response.json([
					{ ...camille, link_id: 'link-a' },
					{ ...zoe, link_id: 'link-b' }
				]);
			}
			if (String(url).startsWith('/api/v1/contacts/') && init?.method === 'PATCH') {
				const id = String(url).split('/').at(-1)!;
				const contact = directory.find((item) => item.id === id)!;
				const patch = JSON.parse(String(init.body));
				Object.assign(contact, patch);
				return Response.json(contact);
			}

			return new Response(null, { status: 404 });
		});
		vi.stubGlobal('fetch', fetcher);
		render(GalleryContactsPanel, { galleryId: 'g1' });

		await screen.findByText('Camille & Hugo');
		await user.click(screen.getByRole('checkbox', { name: 'Select Camille & Hugo' }));
		await user.click(screen.getByRole('checkbox', { name: 'Select Zoé Laurent' }));
		await user.click(screen.getByRole('button', { name: 'Quick edit' }));
		const dialog = screen.getByRole('dialog', { name: 'Quick edit contacts' });
		const camilleRow = within(dialog).getByRole('group', { name: 'Camille & Hugo' });
		const zoeRow = within(dialog).getByRole('group', { name: 'Zoé Laurent' });
		await user.clear(within(camilleRow).getByLabelText('Notes'));
		await user.type(within(camilleRow).getByLabelText('Notes'), 'Warm finish');
		await user.type(within(zoeRow).getByLabelText('Notes'), 'Matte finish');
		await user.click(within(dialog).getByRole('button', { name: 'Save changes' }));

		await waitFor(() =>
			expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'PATCH')).toHaveLength(2)
		);
		await waitFor(() =>
			expect(screen.queryByRole('dialog', { name: 'Quick edit contacts' })).toBeNull()
		);
		expect(screen.getByText('Camille & Hugo')).toBeTruthy();
		expect(screen.getByText('Zoé Laurent')).toBeTruthy();
	});

	it('retains failed bulk unlinks for a direct retry', async () => {
		const user = userEvent.setup();
		const linked = new Set(['c1', 'c2']);
		let zRetry = false;
		const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
			if (url === '/api/v1/contacts') return Response.json([camille, zoe]);
			if (url === '/api/v1/galleries/g1/contacts') {
				return Response.json(
					[camille, zoe].filter(({ id }) => linked.has(id)).map((c) => ({ ...c, link_id: c.id }))
				);
			}
			if (url.endsWith('/c1') && init?.method === 'DELETE') {
				linked.delete('c1');
				return new Response(null, { status: 204 });
			}
			if (url.endsWith('/c2') && init?.method === 'DELETE') {
				if (!zRetry) {
					zRetry = true;
					return Response.json({}, { status: 503 });
				}
				linked.delete('c2');
				return new Response(null, { status: 204 });
			}

			return new Response(null, { status: 404 });
		});
		vi.stubGlobal('fetch', fetcher);
		render(GalleryContactsPanel, { galleryId: 'g1' });

		await screen.findByText('Zoé Laurent');
		await user.click(screen.getByRole('checkbox', { name: 'Select Camille & Hugo' }));
		await user.click(screen.getByRole('checkbox', { name: 'Select Zoé Laurent' }));
		await user.click(screen.getByRole('button', { name: 'Unlink selected (2)' }));

		await waitFor(() => expect(screen.getByText('1 selected')).toBeTruthy());
		expect(screen.queryByText('Camille & Hugo')).toBeNull();
		await user.click(screen.getByRole('button', { name: 'Unlink selected (1)' }));
		await waitFor(() => expect(linked.size).toBe(0));
	});

	it('guards an in-flight unlink against duplicate requests', async () => {
		const slow = deferred<Response>();
		const { fetcher } = fixture([camille], ['c1']);
		fetcher.mockImplementation(async (url, init) => {
			if (url === '/api/v1/contacts') return Response.json([camille]);
			if (url === '/api/v1/galleries/g1/contacts') {
				return Response.json([{ ...camille, link_id: 'link-a' }]);
			}
			if (String(url).endsWith('/contacts/c1') && init?.method === 'DELETE') return slow.promise;
			return new Response(null, { status: 404 });
		});
		const user = userEvent.setup();
		const oncount = vi.fn();
		render(GalleryContactsPanel, { galleryId: 'g1', oncount });

		await screen.findByText('Camille & Hugo');
		await user.click(screen.getByRole('checkbox', { name: 'Select Camille & Hugo' }));
		const unlink = screen.getByRole('button', { name: 'Unlink selected (1)' });
		await user.click(unlink);
		fireEvent.click(unlink);
		expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1);
		slow.resolve(new Response(null, { status: 204 }));

		await waitFor(() => expect(screen.queryByText('Camille & Hugo')).toBeNull());
		expect(oncount).toHaveBeenLastCalledWith(0);
	});

	it('shows an empty gallery with one create action', async () => {
		fixture([]);
		const { container } = render(GalleryContactsPanel, { galleryId: 'g1' });

		expect(await screen.findByText('No contacts yet.')).toBeTruthy();
		expect(screen.getAllByRole('button', { name: 'New contact' })).toHaveLength(1);
		expect(screen.getByRole('button', { name: 'Manage contacts' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('GalleryContactsPanel in French', () => {
	useLocale('fr');

	it('renders the linked contacts and the accessible actions', async () => {
		fixture([camille], ['c1']);
		const { container } = render(GalleryContactsPanel, { galleryId: 'g1' });

		expect(await screen.findByText('Camille & Hugo')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Nouveau contact' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Gérer les contacts' })).toBeTruthy();
		expect(screen.queryByText('Associé')).toBeNull();
		expect((await axe(container)).violations).toEqual([]);
	});
});
