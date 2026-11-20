import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Contact } from '#lib/contacts/api.js';
import type { Gallery } from '#lib/gallery/api.js';
import type { Photo } from '#lib/gallery/photos.js';
import { DEFAULT_SETTINGS, LOGO_URL, type WorkspaceSettings } from '#lib/settings/settings.js';
import { toasts } from './ui/Toast/toasts.svelte.js';
import GallerySharePanel from './GallerySharePanel.svelte';

const baseGallery: Gallery = {
	id: 'g1',
	title: 'Camille & Lucas',
	favorite: false,
	layout: 'grid',
	cover_layout: 'full_bleed',
	focal_point: { x: 40, y: 25 },
	heading_font: 'editorial',
	grid_density: 'compact',
	grid_spacing: 'compact',
	expires_at: '2026-09-30',
	included_photos: 20,
	watermark_enabled: true,
	hero_id: 'p1',
	share_token: null,
	last_opened_at: null,
	shoot_date: '2026-06-14',
	states: ['draft'],
	created_at: '2026-06-12T10:00:00Z',
	updated_at: '2026-06-12T10:00:00Z'
};

const camille: Contact = {
	id: 'c1',
	name: 'Camille Laurent',
	email: 'camille@example.com',
	phone: null,
	notes: null,
	galleries: [{ id: 'g1', title: 'Camille & Lucas' }],
	created_at: '2026-06-10T10:00:00Z',
	updated_at: '2026-06-10T10:00:00Z'
};

const lucas: Contact = {
	id: 'c2',
	name: 'Lucas Martin',
	email: 'lucas@example.com',
	phone: null,
	notes: null,
	galleries: [],
	created_at: '2026-06-10T10:00:00Z',
	updated_at: '2026-06-10T10:00:00Z'
};

const readyPhoto: Photo = {
	id: 'p1',
	original_name: 'IMG_0001.jpg',
	size: 1024,
	width: 1500,
	height: 1000,
	status: 'ready',
	created_at: '2026-06-12T10:00:00Z'
};

const smtpSettings: WorkspaceSettings = {
	...DEFAULT_SETTINGS,
	studioName: 'Atelier Lumière',
	logo: LOGO_URL,
	signature: 'both',
	emailEnabled: true
};

afterEach(() => {
	toasts.items = [];
});

describe('GallerySharePanel', () => {
	it('edits in the envelope and renders a noninteractive live preview', async () => {
		const loadLinkedContacts = vi.fn().mockResolvedValue([camille]);
		const loadWorkspaceContacts = vi.fn().mockResolvedValue([camille, lucas]);
		const loadPhotos = vi.fn().mockResolvedValue([readyPhoto]);
		const sharedGallery: Gallery = {
			...baseGallery,
			share_token: 'T'.repeat(22),
			states: ['shared']
		};
		const sendInvitations = vi.fn().mockResolvedValue({ gallery: sharedGallery, queued_count: 3 });
		const onbeforesend = vi.fn().mockResolvedValue(true);
		const onsent = vi.fn();
		const user = userEvent.setup();

		const view = render(GallerySharePanel, {
			gallery: baseGallery,
			settings: smtpSettings,
			coverId: 'p1',
			focalPoint: { x: 40, y: 25 },
			photoCount: 48,
			accessCode: '482910',
			loadLinkedContacts,
			loadWorkspaceContacts,
			loadPhotos,
			sendInvitations,
			onbeforesend,
			onsent
		});

		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Remove camille@example.com' })).toBeTruthy()
		);

		// Live email preview renders signature, hero image with focal point, title, access code, expiry, CTA
		expect(screen.getByTestId('studio-signature').textContent).toContain('Atelier Lumière');
		const coverImg = screen.getByAltText('Gallery cover') as HTMLImageElement;
		expect(coverImg.getAttribute('src')).toBe('/api/v1/galleries/g1/photos/p1/preview');
		expect(coverImg.style.objectPosition).toBe('40% 25%');
		expect(screen.getByText('482910')).toBeTruthy();
		expect(screen.getByText(/Available until/)).toBeTruthy();
		expect(screen.getByText('View gallery')).toBeTruthy();

		// Add workspace contact via + Contacts picker
		await user.click(screen.getByRole('button', { name: '+ Contacts' }));
		const dialog = screen.getByRole('dialog');
		await vi.waitFor(() => expect(within(dialog).getByText('Lucas Martin')).toBeTruthy());
		await user.click(within(dialog).getByRole('button', { name: /Lucas Martin/ }));
		expect(screen.getByRole('button', { name: 'Remove lucas@example.com' })).toBeTruthy();

		// Add BCC recipient
		await user.click(screen.getByRole('button', { name: '+ BCC' }));
		const bccInput = screen.getByLabelText('BCC');
		await user.type(bccInput, 'archive@example.com{Enter}');
		await user.type(bccInput, 'camille@example.com{Enter}');
		expect(screen.getByRole('button', { name: 'Remove archive@example.com' })).toBeTruthy();

		const editor = within(screen.getByRole('region', { name: 'Invitation envelope' }));
		const preview = within(screen.getByRole('region', { name: /email preview/i }));
		const subject = editor.getByRole('textbox', { name: 'Subject' });
		const message = editor.getByRole('textbox', { name: 'Personal message' });
		await user.clear(subject);
		await user.type(subject, 'Proofs <Studio>');
		await user.clear(message);
		await user.type(message, 'Hello <friends>{Enter}See your proofs.');
		expect(preview.getByText('Proofs <Studio>')).toBeTruthy();
		expect(preview.getByText(/Hello <friends>/).textContent).toBe(
			'Hello <friends>\nSee your proofs.'
		);
		expect(preview.queryByRole('textbox')).toBeNull();
		expect(preview.queryByRole('link')).toBeNull();
		expect(preview.queryByText(/@example\.com/)).toBeNull();
		expect(preview.queryByRole('button', { name: /^Remove / })).toBeNull();
		expect((await axe(view.container)).violations).toEqual([]);

		await user.click(editor.getByRole('button', { name: 'Send invitations (3)' }));

		expect(onbeforesend).toHaveBeenCalledOnce();
		expect(sendInvitations).toHaveBeenCalledWith('g1', {
			to: [
				{ email: 'camille@example.com', name: 'Camille Laurent' },
				{ email: 'lucas@example.com', name: 'Lucas Martin' }
			],
			bcc: ['archive@example.com'],
			subject: 'Proofs <Studio>',
			message: 'Hello <friends>\nSee your proofs.'
		});
		expect(onsent).toHaveBeenCalledWith({ gallery: sharedGallery, queued_count: 3 });
	});

	it('updates untouched subject and message when switching to Delivered, while keeping custom edits', async () => {
		const loadLinkedContacts = vi.fn().mockResolvedValue([]);
		const user = userEvent.setup();
		const props = {
			gallery: baseGallery,
			settings: smtpSettings,
			loadLinkedContacts
		};
		const view = render(GallerySharePanel, props);

		const subjectInput = screen.getByLabelText('Subject') as HTMLInputElement;
		const messageInput = screen.getByLabelText('Personal message') as HTMLTextAreaElement;
		expect(subjectInput.value).toBe('Camille & Lucas — Your proofing gallery is ready');
		expect(messageInput.value).toContain('Your proofing gallery is ready to view.');

		// Stage transitions to delivered before manual edits -> updates defaults automatically
		await view.rerender({
			...props,
			gallery: { ...baseGallery, share_token: 'T'.repeat(22), states: ['delivered'] },
			finalCount: 24
		});
		expect(subjectInput.value).toBe('Camille & Lucas — Your final photos are ready');
		expect(messageInput.value).toContain('Your finished photos are ready to view and download');
		expect(screen.getByText('View final photos')).toBeTruthy();

		// Manual edit is preserved across gallery prop updates
		await user.clear(subjectInput);
		await user.type(subjectInput, 'Custom subject line');
		await view.rerender({
			...props,
			gallery: {
				...baseGallery,
				title: 'Renamed Gallery',
				share_token: 'T'.repeat(22),
				states: ['delivered']
			}
		});
		expect(subjectInput.value).toBe('Custom subject line');
	});

	it.each([
		{ photos: [], notice: /no proofs are ready/i, dialog: 'Send invitation with no ready photos?' },
		{
			photos: [readyPhoto, { ...readyPhoto, id: 'p2', status: 'processing' }],
			notice: /some proofs aren’t ready/i,
			dialog: 'Send invitation while photos aren’t all ready?'
		}
	])(
		'shows proof readiness inline without blocking confirmation: $dialog',
		async ({ photos, notice, dialog }) => {
			const loadLinkedContacts = vi.fn().mockResolvedValue([camille]);
			const loadPhotos = vi.fn().mockResolvedValue(photos);
			const sharedGallery: Gallery = {
				...baseGallery,
				share_token: 'T'.repeat(22),
				states: ['shared']
			};
			const sendInvitations = vi
				.fn()
				.mockResolvedValue({ gallery: sharedGallery, queued_count: 1 });
			const onsent = vi.fn();
			const user = userEvent.setup();

			render(GallerySharePanel, {
				gallery: baseGallery,
				settings: smtpSettings,
				photoCount: 0,
				loadLinkedContacts,
				loadPhotos,
				sendInvitations,
				onsent
			});

			await vi.waitFor(() =>
				expect(screen.getByRole('button', { name: 'Remove camille@example.com' })).toBeTruthy()
			);

			expect(await screen.findByText(notice)).toBeTruthy();
			await user.click(screen.getByRole('button', { name: 'Send invitation (1)' }));

			expect(sendInvitations).not.toHaveBeenCalled();
			expect(await screen.findByRole('dialog', { name: dialog })).toBeTruthy();

			await user.click(screen.getByRole('button', { name: 'Send anyway' }));

			await vi.waitFor(() => expect(sendInvitations).toHaveBeenCalledOnce());
			expect(onsent).toHaveBeenCalledOnce();
		}
	);

	it('reserves one send across preparation and rechecks the gallery before sending', async () => {
		let resolvePreparation!: (ok: boolean) => void;
		const onbeforesend = vi.fn(
			() => new Promise<boolean>((resolve) => (resolvePreparation = resolve))
		);
		const sendInvitations = vi.fn();
		const props = {
			gallery: baseGallery,
			settings: smtpSettings,
			loadLinkedContacts: vi.fn().mockResolvedValue([camille]),
			loadPhotos: vi.fn().mockResolvedValue([readyPhoto]),
			sendInvitations,
			onbeforesend
		};
		const view = render(GallerySharePanel, props);
		await screen.findByRole('button', { name: 'Remove camille@example.com' });
		const first = view.component.requestSend();
		const duplicate = view.component.requestSend();
		await vi.waitFor(() => expect(onbeforesend).toHaveBeenCalledOnce());
		await view.rerender({ gallery: { ...baseGallery, states: ['archived'] } });
		resolvePreparation(true);
		await Promise.all([first, duplicate]);
		expect(sendInvitations).not.toHaveBeenCalled();
	});

	it('rechecks access-code preparation and eligibility on Send anyway', async () => {
		const sendInvitations = vi.fn();
		const onbeforesend = vi.fn().mockResolvedValueOnce(true).mockResolvedValueOnce(false);
		const user = userEvent.setup();
		const view = render(GallerySharePanel, {
			gallery: baseGallery,
			settings: smtpSettings,
			loadLinkedContacts: vi.fn().mockResolvedValue([camille]),
			loadPhotos: vi.fn().mockResolvedValue([]),
			sendInvitations,
			onbeforesend
		});
		await screen.findByRole('button', { name: 'Remove camille@example.com' });
		await view.component.requestSend();
		await user.click(screen.getByRole('button', { name: 'Send anyway' }));
		expect(onbeforesend).toHaveBeenCalledTimes(2);
		expect(sendInvitations).not.toHaveBeenCalled();
		expect(await screen.findByRole('alert')).toBeTruthy();
	});

	it('shows informational notices and disables sending when SMTP is unconfigured or the gallery is expired', async () => {
		const onopensettings = vi.fn();
		const sendInvitations = vi.fn();
		const user = userEvent.setup();

		const noSmtp = render(GallerySharePanel, {
			gallery: baseGallery,
			settings: { ...smtpSettings, emailEnabled: false },
			loadLinkedContacts: vi.fn().mockResolvedValue([camille]),
			sendInvitations,
			onopensettings
		});

		expect(screen.getByText('Email sending isn’t configured')).toBeTruthy();
		await user.click(screen.getByRole('button', { name: 'Open Settings' }));
		expect(onopensettings).toHaveBeenCalledOnce();
		await noSmtp.component.requestSend();
		expect(sendInvitations).not.toHaveBeenCalled();
		expect((await axe(noSmtp.container)).violations).toEqual([]);
		noSmtp.unmount();

		const expired = render(GallerySharePanel, {
			gallery: {
				...baseGallery,
				share_token: 'T'.repeat(22),
				states: ['shared', 'expired']
			},
			settings: smtpSettings,
			loadLinkedContacts: vi.fn().mockResolvedValue([camille]),
			sendInvitations,
			onopensettings
		});

		expect(screen.getByText('This gallery has expired')).toBeTruthy();
		await expired.component.requestSend();
		expect(sendInvitations).not.toHaveBeenCalled();
		expect((await axe(expired.container)).violations).toEqual([]);
	});

	it('enables Send for an uncommitted draft input and syncs contacts after send', async () => {
		let linked: Contact[] = [];
		const loadLinkedContacts = vi.fn().mockImplementation(async () => linked);
		const sharedGallery: Gallery = {
			...baseGallery,
			share_token: 'T'.repeat(22),
			states: ['shared']
		};
		const sendInvitations = vi.fn().mockResolvedValue({ gallery: sharedGallery, queued_count: 1 });
		const user = userEvent.setup();

		const props = {
			gallery: baseGallery,
			settings: smtpSettings,
			contactsRevision: 0,
			loadLinkedContacts,
			loadPhotos: vi.fn().mockResolvedValue([readyPhoto]),
			sendInvitations
		};
		const view = render(GallerySharePanel, props);

		await vi.waitFor(() => expect(loadLinkedContacts).toHaveBeenCalledTimes(1));
		expect(
			(screen.getByRole('button', { name: 'Send invitation' }) as HTMLButtonElement).disabled
		).toBe(true);

		const toInput = screen.getByLabelText('To');
		await user.type(toInput, 'camille@example.com');
		expect(
			(screen.getByRole('button', { name: 'Send invitation (1)' }) as HTMLButtonElement).disabled
		).toBe(false);

		await view.component.requestSend();
		expect(sendInvitations).toHaveBeenCalledOnce();

		// Follow-up contactsRevision after send reloads newly linked/updated contacts because recipientsDirty was reset
		linked = [camille, lucas];
		await view.rerender({
			...props,
			gallery: sharedGallery,
			contactsRevision: 1
		});
		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Remove lucas@example.com' })).toBeTruthy()
		);
	});

	it('surfaces a single localized inline error inside ConfirmDialog on network failure and redirects on 401', async () => {
		const loadLinkedContacts = vi.fn().mockResolvedValue([camille]);
		const loadPhotos = vi.fn().mockResolvedValue([]);
		const sendInvitations = vi
			.fn()
			.mockRejectedValueOnce(new TypeError('Failed to fetch'))
			.mockRejectedValueOnce(
				new (await import('#lib/http.js')).ApiError(401, 'unauthorized', 'Expired')
			);
		const nav = await import('$app/navigation');
		const gotoSpy = vi.spyOn(nav, 'goto').mockResolvedValue();
		const user = userEvent.setup();

		const view = render(GallerySharePanel, {
			gallery: baseGallery,
			settings: smtpSettings,
			photoCount: 0,
			loadLinkedContacts,
			loadPhotos,
			sendInvitations
		});

		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Remove camille@example.com' })).toBeTruthy()
		);

		await view.component.requestSend();
		const sendAnyway = screen.getByRole('button', { name: 'Send anyway' });

		// 1st attempt: network TypeError inside ConfirmDialog -> 0 toasts, localized inline alert (not raw "Failed to fetch")
		await user.click(sendAnyway);
		await vi.waitFor(() => expect(sendInvitations).toHaveBeenCalledTimes(1));
		expect(toasts.items).toHaveLength(0);
		const alert = screen.getByRole('alert');
		expect(alert.textContent).toBe('Couldn’t send invitations. Please try again.');
		expect(alert.textContent).not.toContain('Failed to fetch');

		// 2nd attempt: 401 unauthorized redirects to /login?expired=1
		await user.click(sendAnyway);
		await vi.waitFor(() =>
			expect(gotoSpy).toHaveBeenCalledWith('/login?expired=1', { refreshAll: true })
		);
		gotoSpy.mockRestore();
	});
});
