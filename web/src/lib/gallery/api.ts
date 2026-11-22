import {
	gallerySchema,
	queuedInvitationsSchema,
	type CoverLayout,
	type FocalPoint,
	type Gallery,
	type GalleryLayout,
	type GridDensity,
	type GridSpacing,
	type HeadingFont,
	type QueuedInvitations
} from './schemas';
import { requestEmpty, requestJson } from '#lib/http.js';

export type {
	CoverLayout,
	FocalPoint,
	Gallery,
	GalleryLayout,
	GalleryOverview,
	GallerySort,
	GridDensity,
	GridSpacing,
	HeadingFont,
	QueuedInvitations
} from './schemas';

export interface InvitationRecipientInput {
	email: string;
	link_id?: string;
	name?: string | null;
}

export interface SendInvitationsInput {
	stage: 'proofing' | 'finals';
	copy_self?: boolean;
	to: InvitationRecipientInput[];
	bcc?: string[];
	subject: string;
	message: string;
}

/** Subset of settings sent by a PATCH: a missing key is kept, `null` clears the optional ones. */
export interface GalleryChanges {
	title?: string | null;
	favorite?: boolean;
	layout?: GalleryLayout;
	cover_layout?: CoverLayout;
	focal_point?: FocalPoint;
	heading_font?: HeadingFont;
	grid_density?: GridDensity;
	grid_spacing?: GridSpacing;
	expires_at?: string | null;
	shoot_date?: string | null;
	included_photos?: number | null;
	watermark_enabled?: boolean;
	access_code?: string | null;
	hero_id?: string | null;
}

export function createGallery(fetcher: typeof fetch = fetch): Promise<Gallery> {
	return requestJson('/api/v1/galleries', { method: 'POST' }, gallerySchema, fetcher);
}

export function readGallery(id: string, fetcher: typeof fetch = fetch): Promise<Gallery> {
	return requestJson(
		`/api/v1/galleries/${encodeURIComponent(id)}`,
		{ cache: 'no-store' },
		gallerySchema,
		fetcher
	);
}

export function updateGallery(
	id: string,
	changes: GalleryChanges,
	fetcher: typeof fetch = fetch
): Promise<Gallery> {
	return requestJson(
		`/api/v1/galleries/${encodeURIComponent(id)}`,
		{ method: 'PATCH', body: JSON.stringify(changes), keepalive: true },
		gallerySchema,
		fetcher
	);
}

/** Creates the private link on first call and returns the same one afterwards. */
export function shareGallery(id: string, fetcher: typeof fetch = fetch): Promise<Gallery> {
	return requestJson(
		`/api/v1/galleries/${encodeURIComponent(id)}/share`,
		{ method: 'POST' },
		gallerySchema,
		fetcher
	);
}

/** Shares the gallery if needed, upserts/links `To` contacts, and queues invitation emails. */
export function sendGalleryInvitations(
	id: string,
	payload: SendInvitationsInput,
	fetcher: typeof fetch = fetch
): Promise<QueuedInvitations> {
	return requestJson(
		`/api/v1/galleries/${encodeURIComponent(id)}/invitations`,
		{ method: 'POST', body: JSON.stringify(payload) },
		queuedInvitationsSchema,
		fetcher
	);
}

/** Switches the private link to the finished photos; delivering twice changes nothing. */
export function deliverGallery(id: string, fetcher: typeof fetch = fetch): Promise<Gallery> {
	return requestJson(
		`/api/v1/galleries/${encodeURIComponent(id)}/deliver`,
		{ method: 'POST' },
		gallerySchema,
		fetcher
	);
}

/** Hides the gallery from its client; the gallery and its photos are kept. */
export function archiveGallery(id: string, fetcher: typeof fetch = fetch): Promise<Gallery> {
	return requestJson(
		`/api/v1/galleries/${encodeURIComponent(id)}/archive`,
		{ method: 'POST' },
		gallerySchema,
		fetcher
	);
}

/** Makes an archived gallery reachable by its client again. */
export function restoreGallery(id: string, fetcher: typeof fetch = fetch): Promise<Gallery> {
	return requestJson(
		`/api/v1/galleries/${encodeURIComponent(id)}/restore`,
		{ method: 'POST' },
		gallerySchema,
		fetcher
	);
}

/** Deletes the gallery and its photos for good; there is no response body to read. */
export function deleteGallery(id: string, fetcher: typeof fetch = fetch): Promise<void> {
	return requestEmpty(`/api/v1/galleries/${encodeURIComponent(id)}`, { method: 'DELETE' }, fetcher);
}

/** Submits a 6-digit client access code to unlock a shared gallery (`204` sets `pp_gallery_unlock`). */
export function unlockSharedGallery(
	token: string,
	code: string,
	fetcher: typeof fetch = fetch
): Promise<void> {
	return requestEmpty(
		`/api/v1/galleries/shared/${encodeURIComponent(token)}/unlock`,
		{ method: 'POST', body: JSON.stringify({ code }) },
		fetcher
	);
}
