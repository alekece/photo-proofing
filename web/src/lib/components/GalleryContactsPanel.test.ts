import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
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

/** Resolves on demand, so a test can control exactly when a fetch "arrives". */
function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((res) => {
		resolve = res;
	});

	return { promise, resolve };
}

afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe('GalleryContactsPanel', () => {
	it('links an existing contact, briefly shows Linked, then settles to Unlink without closing the dialog', async () => {
		vi.useFakeTimers();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const oncount = vi.fn();
		const zoeLinked: Contact = { ...zoe, galleries: [{ id: 'g1', title: 'Wedding' }] };

		const fetcher = vi
			.fn()
			// Initial listGalleryContacts('g1')
			.mockResolvedValueOnce(new Response(JSON.stringify([camille]), { status: 200 }))
			// Open dialog -> listContacts()
			.mockResolvedValueOnce(new Response(JSON.stringify([camille, zoe]), { status: 200 }))
			// PUT linkGalleryContact('g1', 'c2')
			.mockResolvedValueOnce(new Response(JSON.stringify(zoeLinked), { status: 200 }));
		vi.stubGlobal('fetch', fetcher);

		render(GalleryContactsPanel, { galleryId: 'g1', oncount });

		expect(await screen.findByText('Camille & Hugo')).toBeTruthy();
		expect(oncount).toHaveBeenLastCalledWith(1);

		await user.click(screen.getByRole('button', { name: '+ Link contact' }));
		const dialog = await screen.findByRole('dialog');

		// The merged directory shows both the already-linked and the unlinked contact.
		const camilleRow = within(dialog).getByText('Camille & Hugo').closest('li');
		const zoeRow = within(dialog).getByText('Zoé Laurent').closest('li') as HTMLElement;
		expect(camilleRow).toBeTruthy();
		expect(zoeRow).toBeTruthy();
		expect(within(camilleRow as HTMLElement).getByRole('button', { name: 'Unlink' })).toBeTruthy();

		fireEvent.click(within(zoeRow).getByRole('button', { name: 'Link' }));
		// Flush the pending fetch's microtask without advancing fake time.
		await vi.advanceTimersByTimeAsync(0);

		// Immediately after the request resolves, the row shows a transient "Linked" confirmation...
		expect(within(zoeRow).getByRole('button', { name: 'Linked' })).toBeTruthy();
		expect(oncount).toHaveBeenLastCalledWith(2);

		// ...for 600ms, after which it settles to Unlink in the same position, the dialog untouched.
		await vi.advanceTimersByTimeAsync(599);
		expect(within(zoeRow).getByRole('button', { name: 'Linked' })).toBeTruthy();
		await vi.advanceTimersByTimeAsync(1);
		expect(within(zoeRow).getByRole('button', { name: 'Unlink' })).toBeTruthy();
		expect(screen.getByRole('dialog')).toBeTruthy();
	});

	it('unlinks a contact from the always-visible list without deleting it, allowing immediate undo', async () => {
		const user = userEvent.setup();
		const oncount = vi.fn();
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(new Response(JSON.stringify([camille]), { status: 200 }))
			// DELETE unlinkGalleryContact('g1', 'c1')
			.mockResolvedValueOnce(new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetcher);

		render(GalleryContactsPanel, { galleryId: 'g1', oncount });

		expect(await screen.findByText('Camille & Hugo')).toBeTruthy();

		const unlinkButton = screen.getByRole('button', { name: 'Unlink' });
		await user.click(unlinkButton);
		await waitFor(() => expect(oncount).toHaveBeenLastCalledWith(0));
		expect(screen.queryByText('Camille & Hugo')).toBeNull();
	});

	it('creates a new contact inline and links it without closing the dialog', async () => {
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
		const linked: Contact = { ...created, galleries: [{ id: 'g1', title: 'Wedding' }] };

		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(new Response(JSON.stringify([]), { status: 200 }))
			.mockResolvedValueOnce(new Response(JSON.stringify([]), { status: 200 }))
			.mockResolvedValueOnce(new Response(JSON.stringify(created), { status: 201 }))
			.mockResolvedValueOnce(new Response(JSON.stringify(linked), { status: 200 }));
		vi.stubGlobal('fetch', fetcher);

		render(GalleryContactsPanel, { galleryId: 'g1', oncount });

		expect(await screen.findByText('No contacts linked yet.')).toBeTruthy();
		await user.click(screen.getAllByRole('button', { name: '+ Link contact' })[0]);

		const dialog = await screen.findByRole('dialog');
		await user.type(within(dialog).getByLabelText('Name'), 'Léo Martin');
		await user.type(within(dialog).getByLabelText('Email'), 'leo@example.test');
		await user.click(within(dialog).getByRole('button', { name: 'Create & link' }));

		expect(await within(dialog).findByText('Léo Martin')).toBeTruthy();
		expect(oncount).toHaveBeenLastCalledWith(1);
		// Still open: the dialog is a persistent panel, not a one-shot modal.
		expect(screen.getByRole('dialog')).toBeTruthy();
		expect(within(screen.getByRole('dialog')).getByText('Léo Martin')).toBeTruthy();
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
		const linked: Contact = { ...created, galleries: [{ id: 'g1', title: 'Wedding' }] };

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

		expect(await within(dialog).findByText('Léo Martin')).toBeTruthy();
		expect(
			fetcher.mock.calls.filter(
				([url, init]) => url === '/api/v1/contacts' && init?.method === 'POST'
			)
		).toHaveLength(1);
	});

	it('guards a row against a duplicate unlink request while an earlier one is still in flight', async () => {
		const user = userEvent.setup();
		const oncount = vi.fn();
		const slowUnlink = deferred<Response>();
		const fetcher = vi
			.fn()
			.mockResolvedValueOnce(new Response(JSON.stringify([camille]), { status: 200 }))
			// DELETE unlinkGalleryContact('g1', 'c1') -- stays pending until the test resolves it.
			.mockReturnValueOnce(slowUnlink.promise)
			// Open dialog -> listContacts()
			.mockResolvedValueOnce(new Response(JSON.stringify([camille]), { status: 200 }));
		vi.stubGlobal('fetch', fetcher);

		render(GalleryContactsPanel, { galleryId: 'g1', oncount });

		expect(await screen.findByText('Camille & Hugo')).toBeTruthy();
		const unlinkButton = screen.getByRole('button', { name: 'Unlink' });
		await user.click(unlinkButton);

		// Open the dialog while the first unlink is still in flight and try to unlink the same row
		// again: the shared per-contact guard must not fire a second request for it.
		await user.click(screen.getByRole('button', { name: '+ Link contact' }));
		const dialog = await screen.findByRole('dialog');
		const row = within(dialog).getByText('Camille & Hugo').closest('li') as HTMLElement;
		await user.click(within(row).getByRole('button', { name: 'Unlink' }));

		const unlinkCalls = fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE');
		expect(unlinkCalls).toHaveLength(1);

		slowUnlink.resolve(new Response(null, { status: 204 }));
		await waitFor(() => expect(oncount).toHaveBeenLastCalledWith(0));
		// The directory dialog still lists Camille (it shows the whole directory), now as Link;
		// the always-visible linked summary behind it no longer does.
		await waitFor(() => expect(within(row).getByRole('button', { name: 'Link' })).toBeTruthy());
		expect(await screen.findByText('No contacts linked yet.')).toBeTruthy();
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
