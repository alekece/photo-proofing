import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Contact } from '#lib/contacts/api.js';
import type { Gallery } from '#lib/gallery/api.js';
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
	shared_at: null,
	delivered_at: null,
	last_opened_at: null,
	shoot_date: '2026-06-14',
	states: ['draft'],
	created_at: '2026-06-12T10:00:00Z',
	updated_at: '2026-06-12T10:00:00Z'
};

const camille = {
	link_id: 'link-a',
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
		const user = userEvent.setup();

		const view = render(GallerySharePanel, {
			gallery: { ...baseGallery, access_code: '482910' },
			stage: 'proofing',
			visibleCount: 48,
			settings: smtpSettings,
			loadLinkedContacts,
			loadWorkspaceContacts
		});

		await vi.waitFor(() => expect(view.component.recipientNames()).toBe('Camille Laurent'));
		await view.component.toggleCustomization();
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
		expect(document.activeElement).toBe(within(dialog).getByRole('button', { name: 'Close' }));
		await vi.waitFor(() => expect(within(dialog).getByText('Lucas Martin')).toBeTruthy());
		await user.click(within(dialog).getByRole('button', { name: /Lucas Martin/ }));
		expect(screen.getByRole('button', { name: 'Remove lucas@example.com' })).toBeTruthy();

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

		await user.click(screen.getByRole('checkbox', { name: 'Send me a copy' }));
		expect(view.component.draft()).toEqual({
			stage: 'proofing',
			copy_self: true,
			to: [
				{ email: 'camille@example.com', name: 'Camille Laurent', link_id: 'link-a' },
				{ email: 'lucas@example.com', name: 'Lucas Martin' }
			],
			bcc: [],
			subject: 'Proofs <Studio>',
			message: 'Hello <friends>\nSee your proofs.'
		});
	});

	it('stays in customization with no recipient left and holds Share back', async () => {
		const user = userEvent.setup();
		const view = render(GallerySharePanel, {
			gallery: baseGallery,
			settings: smtpSettings,
			stage: 'proofing',
			loadLinkedContacts: vi.fn().mockResolvedValue([camille])
		});

		await vi.waitFor(() => expect(view.component.recipientNames()).toBe('Camille Laurent'));
		await view.component.toggleCustomization();
		await user.click(await screen.findByRole('button', { name: 'Remove camille@example.com' }));

		expect(screen.getByRole('region', { name: 'Invitation envelope' })).toBeTruthy();
		expect(screen.getByRole('button', { name: '+ Contacts' })).toBeTruthy();
	});

	it('uses the saved gallery title until the subject is customized', async () => {
		const view = render(GallerySharePanel, {
			gallery: baseGallery,
			settings: smtpSettings,
			stage: 'proofing',
			loadLinkedContacts: vi.fn().mockResolvedValue([camille])
		});
		await vi.waitFor(() => expect(view.component.recipientNames()).toBe('Camille Laurent'));
		await view.rerender({ gallery: { ...baseGallery, title: 'Updated title' } });
		view.component.toggleCustomization();
		expect(((await screen.findByLabelText('Subject')) as HTMLInputElement).value).toContain(
			'Updated title'
		);
	});

	it('distinguishes a failed lookup from an empty list and keeps manual confirmation explicit', async () => {
		const contacts = Promise.withResolvers<(typeof camille)[]>();
		const user = userEvent.setup();
		const view = render(GallerySharePanel, {
			gallery: baseGallery,
			settings: smtpSettings,
			stage: 'finals',
			loadLinkedContacts: () => contacts.promise
		});
		expect(screen.queryByText('No recipient')).toBeNull();
		contacts.reject(new Error('offline'));
		await user.click(await screen.findByRole('button', { name: 'Continue without email' }));
		expect(view.component.draft()).toBeUndefined();
	});
});
