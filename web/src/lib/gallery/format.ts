/** Shown wherever a gallery has no title; the server stores `null`. */
export const UNTITLED_GALLERY = 'Untitled gallery';

/** Longest gallery title the API accepts, in characters. */
export const TITLE_MAX = 255;

const pad = (value: number) => String(value).padStart(2, '0');

/** Short "last edited" text: relative for the last day, otherwise a local date. */
export function formatEdited(iso: string, now: Date = new Date()): string {
	const date = new Date(iso);
	const minutes = Math.floor((now.getTime() - date.getTime()) / 60_000);
	if (minutes < 1) return 'just now';
	if (minutes < 60) return `${minutes} min ago`;
	if (minutes < 24 * 60) return `${Math.floor(minutes / 60)} h ago`;
	return date.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
}

/** `YYYY-MM-DD` of an instant in the browser's time zone, as `<input type=date>` expects. */
export function toLocalDate(iso: string): string {
	const date = new Date(iso);
	return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** 23:59:59 of the chosen day in the browser's time zone, as an RFC 3339 instant. */
export function endOfLocalDay(day: string): string {
	const [year, month, date] = day.split('-').map(Number);
	return new Date(year, month - 1, date, 23, 59, 59).toISOString();
}

/** The one status a gallery shows: archived and expired win over its progress state. */
export function galleryStatus(
	states: readonly ('draft' | 'shared' | 'delivered' | 'archived' | 'expired')[]
) {
	if (states.includes('archived')) return 'archived';
	if (states.includes('expired')) return 'expired';

	return states[0] ?? 'draft';
}

/** Up to two capitals for a gallery without a hero photo; "Camille & Hugo" gives "CH". */
export function galleryInitials(title: string | null): string {
	const words = (title ?? '').match(/[\p{L}\p{N}]+/gu) ?? [];

	return words
		.slice(0, 2)
		.map((word) => word[0].toUpperCase())
		.join('');
}
