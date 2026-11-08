import { m } from '#lib/paraglide/messages.js';

const INVALID_FIELDS: Record<string, () => string> = {
	title: m.api_error_invalid_title,
	included_photos: m.api_error_invalid_included_photos,
	studio_name: m.api_error_invalid_studio_name,
	logo: m.api_error_invalid_logo,
	position: m.api_error_invalid_position,
	size: m.api_error_invalid_size
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

/**
 * The sentence shown for a refused API call, in the workspace language. The API's own `error`
 * text is English and meant for logs, so only the status and the refused `field` are used.
 */
export function apiErrorMessage(status: number, field?: string): string {
	const message =
		(status === 422 && field !== undefined ? INVALID_FIELDS[field] : undefined) ??
		STATUSES[status] ??
		m.api_error_generic;

	return message();
}
