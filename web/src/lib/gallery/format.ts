import { m } from '#lib/paraglide/messages.js';
import { getLocale } from '#lib/paraglide/runtime.js';

/** Longest gallery title the API accepts, in characters. */
export const TITLE_MAX = 255;

export function defaultInvitationSubject(
	stage: 'proofing' | 'finals',
	galleryTitle: string | null
) {
	const title = galleryTitle?.trim() || m.gallery_untitled();

	return stage === 'finals'
		? m.share_default_subject_finals({ title })
		: m.share_default_subject_proofing({ title });
}

const CALENDAR_DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

function parseDateInput(iso: string): { date: Date; calendar: boolean } {
	return CALENDAR_DATE_RE.test(iso)
		? { date: new Date(`${iso}T00:00:00Z`), calendar: true }
		: { date: new Date(iso), calendar: false };
}

/** Short "last edited" text: relative for the last day, otherwise a local date. */
export function formatEdited(iso: string, now: Date = new Date()): string {
	const date = new Date(iso);
	const minutes = Math.floor((now.getTime() - date.getTime()) / 60_000);
	if (minutes < 1) return m.time_just_now();
	if (minutes < 60) return m.time_minutes_ago({ count: minutes });
	if (minutes < 24 * 60) return m.time_hours_ago({ count: Math.floor(minutes / 60) });
	return formatLongDate(iso);
}

/** The regional format of each app language; English keeps the US month names. */
const DATE_LOCALES = { en: 'en-US', fr: 'fr-FR' } as const;

/** The regional format of the app language, for components that format dates themselves. */
export function dateLocale(): string {
	return DATE_LOCALES[getLocale()];
}

/** "7 Sep" or "7 sept.": the day first in both languages; `YYYY-MM-DD` stays in UTC. */
export function formatShortDate(iso: string): string {
	const { date, calendar } = parseDateInput(iso);

	return calendar
		? `${date.getUTCDate()} ${date.toLocaleString(DATE_LOCALES[getLocale()], { month: 'short', timeZone: 'UTC' })}`
		: `${date.getDate()} ${date.toLocaleString(DATE_LOCALES[getLocale()], { month: 'short' })}`;
}

/** "7 Sep 2026": like `formatShortDate`, with the year. */
export function formatLongDate(iso: string): string {
	const { date, calendar } = parseDateInput(iso);

	return `${formatShortDate(iso)} ${calendar ? date.getUTCFullYear() : date.getFullYear()}`;
}

/** "September 7, 2026" or "7 septembre 2026"; `YYYY-MM-DD` calendar dates never shift across time zones. */
export function formatFullDate(
	iso: string,
	timeZone?: string,
	dateStyle: 'long' | 'medium' = 'long'
): string {
	const { date, calendar } = parseDateInput(iso);

	return new Intl.DateTimeFormat(DATE_LOCALES[getLocale()], {
		dateStyle,
		timeZone: calendar ? 'UTC' : timeZone
	}).format(date);
}

/** "Nov 9, 2026, 2:38 PM" or "9 nov. 2026, 14:38", in `timeZone` or the browser's. */
export function formatDateTime(iso: string, timeZone?: string): string {
	return new Intl.DateTimeFormat(DATE_LOCALES[getLocale()], {
		dateStyle: 'medium',
		timeStyle: 'short',
		timeZone
	}).format(new Date(iso));
}

/** The current filter buckets: archived and expired win over the progress state. */
export function galleryStatus(
	states: readonly ('draft' | 'shared' | 'delivered' | 'archived' | 'expired')[]
) {
	if (states.includes('archived')) return 'archived';
	if (states.includes('expired')) return 'expired';

	return states[0] ?? 'draft';
}

/** Expiry is an access flag, not a lifecycle stage. */
export function galleryLifecycle(
	states: Parameters<typeof galleryStatus>[0]
): Exclude<ReturnType<typeof galleryStatus>, 'expired'> {
	if (states.includes('archived')) return 'archived';
	if (states.includes('delivered')) return 'delivered';
	if (states.includes('shared')) return 'shared';

	return 'draft';
}

/** Up to two capitals for a gallery without a hero photo; "Camille & Hugo" gives "CH". */
export function galleryInitials(title: string | null): string {
	const words = (title ?? '').match(/[\p{L}\p{N}]+/gu) ?? [];

	return words
		.slice(0, 2)
		.map((word) => word[0].toUpperCase())
		.join('');
}

/** Splits `text` into runs, flagging each case-insensitive literal occurrence of `needle`. */
export function highlightParts(text: string, needle: string) {
	const wanted = needle.trim().toLowerCase();
	const lower = text.toLowerCase();
	const parts: { text: string; match: boolean }[] = [];
	let from = 0;

	if (wanted !== '') {
		for (let at = lower.indexOf(wanted); at !== -1; at = lower.indexOf(wanted, from)) {
			if (at > from) parts.push({ text: text.slice(from, at), match: false });
			parts.push({ text: text.slice(at, at + wanted.length), match: true });
			from = at + wanted.length;
		}
	}

	if (from < text.length) parts.push({ text: text.slice(from), match: false });

	return parts;
}

const DAY_MS = 24 * 60 * 60 * 1000;

/** A run of the card line; `over` marks a selection above the included photos. */
export interface SummaryPart {
	text: string;
	over: boolean;
}

/** Relative expiry used to explain closed access on the card. */
export function galleryExpiryDescription(iso: string | null, now: Date = new Date()): string {
	if (iso === null) return m.summary_expired();

	const days = CALENDAR_DATE_RE.test(iso)
		? Math.floor(
				(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()) -
					(Date.parse(`${iso}T00:00:00Z`) + DAY_MS)) /
					DAY_MS
			)
		: Math.floor((now.getTime() - new Date(iso).getTime()) / DAY_MS);

	if (days < 1) return m.summary_expired_today();
	if (days === 1) return m.summary_expired_yesterday();

	return m.summary_expired_days_ago({ days });
}

const plain = (text: string): SummaryPart[] => [{ text, over: false }];

/**
 * The one line on a gallery card, as runs so the selection can be highlighted.
 */
export function gallerySummary(gallery: {
	states: Parameters<typeof galleryStatus>[0];
	included_photos: number | null;
	photo_count: number;
	favorite_count: number;
	final_count: number;
	downloaded_count: number;
}): SummaryPart[] {
	const status = galleryLifecycle(gallery.states);
	const photos = m.summary_photos({ count: gallery.photo_count });
	const finalsText = m.summary_finals({ count: gallery.final_count });

	if (status === 'archived') {
		return plain(m.summary_archived({ items: gallery.final_count > 0 ? finalsText : photos }));
	}
	if (status === 'delivered') {
		const { final_count: finals, downloaded_count: downloaded } = gallery;

		if (finals === 0) return plain(m.summary_finals_delivered());

		return plain(
			downloaded === finals
				? m.summary_all_downloaded({ finals: finalsText })
				: m.summary_some_downloaded({ finals: finalsText, count: downloaded })
		);
	}
	if (status === 'draft') {
		return plain(gallery.photo_count === 0 ? m.summary_no_photos() : photos);
	}

	const over = gallery.included_photos !== null && gallery.favorite_count > gallery.included_photos;
	return [
		{ text: String(gallery.favorite_count), over },
		{ text: m.summary_selected({ count: gallery.favorite_count }), over: false }
	];
}
