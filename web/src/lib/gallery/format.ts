import { m } from '#lib/paraglide/messages.js';
import { getLocale } from '#lib/paraglide/runtime.js';

/** Longest gallery title the API accepts, in characters. */
export const TITLE_MAX = 255;

const pad = (value: number) => String(value).padStart(2, '0');

/** Short "last edited" text: relative for the last day, otherwise a local date. */
export function formatEdited(iso: string, now: Date = new Date()): string {
	const date = new Date(iso);
	const minutes = Math.floor((now.getTime() - date.getTime()) / 60_000);
	if (minutes < 1) return m.time_just_now();
	if (minutes < 60) return m.time_minutes_ago({ count: minutes });
	if (minutes < 24 * 60) return m.time_hours_ago({ count: Math.floor(minutes / 60) });
	return formatLongDate(iso);
}

/** `YYYY-MM-DD` of an instant in the browser's time zone, as `<input type=date>` expects. */
export function toLocalDate(iso: string): string {
	const date = new Date(iso);
	return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** The regional format of each app language; English keeps the US month names. */
const DATE_LOCALES = { en: 'en-US', fr: 'fr-FR' } as const;

/** The regional format of the app language, for components that format dates themselves. */
export function dateLocale(): string {
	return DATE_LOCALES[getLocale()];
}

/** "7 Sep" or "7 sept.": the day first in both languages. */
export function formatShortDate(iso: string): string {
	const date = new Date(iso);

	return `${date.getDate()} ${date.toLocaleString(DATE_LOCALES[getLocale()], { month: 'short' })}`;
}

/** "7 Sep 2026": like `formatShortDate`, with the year. */
export function formatLongDate(iso: string): string {
	return `${formatShortDate(iso)} ${new Date(iso).getFullYear()}`;
}

/** "September 7, 2026" or "7 septembre 2026", in `timeZone` or the browser's. */
export function formatFullDate(
	iso: string,
	timeZone?: string,
	dateStyle: 'long' | 'medium' = 'long'
): string {
	return new Intl.DateTimeFormat(DATE_LOCALES[getLocale()], { dateStyle, timeZone }).format(
		new Date(iso)
	);
}

/** "Nov 9, 2026, 2:38 PM" or "9 nov. 2026, 14:38", in `timeZone` or the browser's. */
export function formatDateTime(iso: string, timeZone?: string): string {
	return new Intl.DateTimeFormat(DATE_LOCALES[getLocale()], {
		dateStyle: 'medium',
		timeStyle: 'short',
		timeZone
	}).format(new Date(iso));
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

function expiredAgo(iso: string | null, now: Date): string {
	if (iso === null) return m.summary_expired();

	const days = Math.floor((now.getTime() - new Date(iso).getTime()) / DAY_MS);

	if (days < 1) return m.summary_expired_today();
	if (days === 1) return m.summary_expired_yesterday();

	return m.summary_expired_days_ago({ days });
}

const plain = (text: string): SummaryPart[] => [{ text, over: false }];

/**
 * The one line on a gallery card, as runs so the selection can be highlighted.
 */
export function gallerySummary(
	gallery: {
		states: Parameters<typeof galleryStatus>[0];
		included_photos: number | null;
		expires_at: string | null;
		photo_count: number;
		favorite_count: number;
		final_count: number;
		downloaded_count: number;
	},
	now: Date = new Date()
): SummaryPart[] {
	const status = galleryStatus(gallery.states);
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
	const selected: SummaryPart[] = [
		{ text: `${photos} · `, over: false },
		{ text: String(gallery.favorite_count), over },
		{ text: m.summary_selected({ count: gallery.favorite_count }), over: false }
	];

	if (status === 'expired') {
		return [...selected, { text: ` · ${expiredAgo(gallery.expires_at, now)}`, over: false }];
	}

	return selected;
}
