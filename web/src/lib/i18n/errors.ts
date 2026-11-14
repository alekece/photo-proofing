import { m } from '#lib/paraglide/messages.js';

const CODES: Record<string, () => string> = {
	unauthorized: m.api_error_unauthorized,
	invalid_login: m.api_error_invalid_login,
	forbidden_origin: m.api_error_forbidden,
	not_found: m.api_error_not_found,
	setup_closed: m.api_error_setup_closed,
	gallery_locked: m.api_error_gallery_locked,
	invalid_access_code: m.api_error_invalid_access_code,
	access_code_locked: m.api_error_access_code_locked,
	access_code_unavailable: m.api_error_access_code_unavailable,
	email_unavailable: m.api_error_email_unavailable,
	gallery_archived: m.api_error_gallery_archived,
	gallery_expired: m.api_error_gallery_expired,
	too_many_recipients: m.api_error_too_many_recipients,
	gallery_delivered: m.api_error_gallery_delivered,
	contact_email_taken: m.api_error_contact_email_taken,
	thread_full: m.api_error_thread_full,
	gallery_not_shared: m.api_error_gallery_not_shared,
	selection_empty: m.api_error_selection_empty,
	zip_not_ready: m.api_error_zip_not_ready,
	zip_in_progress: m.api_error_zip_in_progress,
	request_timeout: m.api_error_request_timeout,
	payload_too_large: m.api_error_too_large,
	selection_too_large: m.api_error_selection_too_large,
	unsupported_media_type: m.api_error_unsupported,
	invalid_image: m.api_error_invalid_image,
	dimensions_too_large: m.api_error_dimensions_too_large,
	invalid_logo: m.api_error_invalid_logo,
	logo_too_large: m.api_error_logo_too_large,
	logo_required: m.api_error_logo_required,
	invalid_title: m.api_error_invalid_title,
	invalid_included_photos: m.api_error_invalid_included_photos,
	invalid_studio_name: m.api_error_invalid_studio_name,
	invalid_position: m.api_error_invalid_position,
	invalid_size: m.api_error_invalid_size,
	invalid_request_body: m.api_error_invalid,
	too_many_attempts: m.api_error_too_many_attempts,
	busy: m.api_error_busy,
	storage_full: m.api_error_storage_full,
	unavailable: m.api_error_unavailable,
	photo_removed: m.api_error_photo_removed,
	export_failed: m.api_error_export_failed
};

const STATUSES: Record<number, () => string> = {
	401: m.api_error_unauthorized,
	403: m.api_error_forbidden,
	404: m.api_error_not_found,
	409: m.api_error_conflict,
	413: m.api_error_too_large,
	415: m.api_error_unsupported,
	422: m.api_error_invalid,
	429: m.api_error_busy,
	502: m.api_error_unexpected,
	503: m.api_error_unavailable,
	507: m.api_error_storage_full
};

/** The `code` of an error body, if it has one. */
export function readErrorCode(data: unknown): string | undefined {
	const code = (data as { code?: unknown } | null)?.code;

	return typeof code === 'string' ? code : undefined;
}

/** The `code` of a refused response's JSON body, if it has one. */
export async function readResponseErrorCode(response: Response): Promise<string | undefined> {
	return readErrorCode(await response.json().catch(() => null));
}

/**
 * The sentence shown for a refused API call, in the workspace language. A code the web knows
 * wins; an unknown or missing one falls back to the status.
 */
export function apiErrorMessage(status: number, code?: string): string {
	const message =
		(code !== undefined && Object.hasOwn(CODES, code) ? CODES[code] : undefined) ??
		STATUSES[status] ??
		m.api_error_generic;

	return message();
}
