import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';
import type { Contact } from '#lib/contacts/api.js';
import GalleryContactsPanel from './GalleryContactsPanel.svelte';

const camille: Contact = {
	id: 'c1',
	name: 'Camille & Hugo',
	email: 'camille@example.test',
	phone: '+33 6 12 34 56 78',
	notes: null,
	galleries: [{ id: 'g1', title: 'Wedding' }],
	created_at: '2026-11-13T10:00:00Z',
	updated_at: '2026-11-13T10:00:00Z'
};

const zoe: Contact = {
	id: 'c2',
	name: 'Zoé Laurent',
	email: 'zoe@example.test',
	phone: null,
	notes: null,
	galleries: [],
	created_at: '2026-11-13T10:00:00Z',
	updated_at: '2026-11-13T10:00:00Z'
};

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('GalleryContactsPanel', () => {
	it('loads linked contacts, reports count, links an existing contact, and unlinks it', async () => {
		const user = userEvent.setup();
		const oncount = vi.fn();
		const zoeLinked: Contact = {
			...zoe,
			galleries: [{ id: 'g1', title: 'Wedding' }]
		};

		const fetcher = vi
			.fn()
			// Initial listGalleryContacts('g1')
			.mockResolvedValueOnce(new Response(JSON.stringify([camille]), { status: 200 }))
			// Open dialog -> listContacts()
			.mockResolvedValueOnce(new Response(JSON.stringify([camille, zoe]), { status: 200 }))
			// PUT linkGalleryContact('g1', 'c2')
			.mockResolvedValueOnce(new Response(JSON.stringify(zoeLinked), { status: 200 }))
			// DELETE unlinkGalleryContact('g1', 'c1')
			.mockResolvedValueOnce(new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetcher);

		const { container } = render(GalleryContactsPanel, { galleryId: 'g1', oncount });

		expect(await screen.findByText('Camille & Hugo')).toBeTruthy();
		expect(oncount).toHaveBeenLastCalledWith(1);
		expect((await axe(container)).violations).toEqual([]);

		// Open "+ Link contact" dialog and link Zoé.
		await user.click(screen.getByRole('button', { name: '+ Link contact' }));
		const dialog = await screen.findByRole('dialog');
		expect(within(dialog).getByText('Zoé Laurent')).toBeTruthy();
		expect(within(dialog).queryByText('Camille & Hugo')).toBeNull();

		await user.click(within(dialog).getByRole('button', { name: 'Link' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(await screen.findByText('Zoé Laurent')).toBeTruthy();
		expect(oncount).toHaveBeenLastCalledWith(2);

		// Unlink Camille without deleting her from workspace.
		const unlinkButtons = screen.getAllByRole('button', { name: 'Unlink' });
		await user.click(unlinkButtons[0]);
		await waitFor(() => expect(screen.queryByText('Camille & Hugo')).toBeNull());
		expect(oncount).toHaveBeenLastCalledWith(1);
	});

	it('creates a new contact inline and links it to the gallery in one step', async () => {
		const user = userEvent.setup();
		const oncount = vi.fn();
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
		const linked: Contact = {
			...created,
			galleries: [{ id: 'g1', title: 'Wedding' }]
		};

		const fetcher = vi
			.fn()
			// Initial listGalleryContacts('g1') -> empty
			.mockResolvedValueOnce(new Response(JSON.stringify([]), { status: 200 }))
			// Open dialog -> listContacts() -> empty, so inline form opens directly
			.mockResolvedValueOnce(new Response(JSON.stringify([]), { status: 200 }))
			// POST /api/v1/contacts
			.mockResolvedValueOnce(new Response(JSON.stringify(created), { status: 201 }))
			// PUT /api/v1/galleries/g1/contacts/c3
			.mockResolvedValueOnce(new Response(JSON.stringify(linked), { status: 200 }));
		vi.stubGlobal('fetch', fetcher);

		const { container } = render(GalleryContactsPanel, { galleryId: 'g1', oncount });

		expect(await screen.findByText('No contacts linked yet.')).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
		await user.click(screen.getAllByRole('button', { name: '+ Link contact' })[0]);

		const dialog = await screen.findByRole('dialog');
		await user.type(within(dialog).getByLabelText('Name'), 'Léo Martin');
		await user.type(within(dialog).getByLabelText('Email'), 'leo@example.test');
		await user.click(within(dialog).getByRole('button', { name: 'Create & link' }));

		expect(await screen.findByText('Léo Martin')).toBeTruthy();
		expect(oncount).toHaveBeenLastCalledWith(1);
	});

	it('lets the user retry linking without duplicating the contact if inline link fails after creation', async () => {
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
		const linked: Contact = {
			...created,
			galleries: [{ id: 'g1', title: 'Wedding' }]
		};

		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(new Response(JSON.stringify([]), { status: 200 }))
			.mockResolvedValueOnce(new Response(JSON.stringify([]), { status: 200 }))
			.mockResolvedValueOnce(new Response(JSON.stringify(created), { status: 201 }))
			.mockResolvedValueOnce(
				new Response(JSON.stringify({ code: 'unavailable' }), {
					status: 503,
					headers: { 'Content-Type': 'application/json' }
				})
			)
			.mockResolvedValueOnce(new Response(JSON.stringify(linked), { status: 200 }));
		vi.stubGlobal('fetch', fetcher);

		render(GalleryContactsPanel, { galleryId: 'g1' });

		expect(await screen.findByText('No contacts linked yet.')).toBeTruthy();
		await user.click(screen.getAllByRole('button', { name: '+ Link contact' })[0]);

		const dialog = await screen.findByRole('dialog');
		await user.type(within(dialog).getByLabelText('Name'), 'Léo Martin');
		await user.type(within(dialog).getByLabelText('Email'), 'leo@example.test');
		await user.click(within(dialog).getByRole('button', { name: 'Create & link' }));

		expect(await within(dialog).findByRole('alert')).toBeTruthy();
		await user.click(within(dialog).getByRole('button', { name: 'Link' }));

		expect(await screen.findByText('Léo Martin')).toBeTruthy();
		expect(
			fetcher.mock.calls.filter(
				([url, init]) => url === '/api/v1/contacts' && init?.method === 'POST'
			)
		).toHaveLength(1);
	});
});

describe('GalleryContactsPanel in French', () => {
	useLocale('fr');

	it('renders French labels and passes axe', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => new Response(JSON.stringify([camille]), { status: 200 }))
		);

		const { container } = render(GalleryContactsPanel, { galleryId: 'g1' });

		expect(await screen.findByText('Camille & Hugo')).toBeTruthy();
		expect(screen.getByRole('button', { name: '+ Associer un contact' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Dissocier' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});
});
