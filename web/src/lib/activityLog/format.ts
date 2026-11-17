import { formatLongDate } from '#lib/gallery/format.js';
import { m } from '#lib/paraglide/messages.js';

const DAY_MS = 86_400_000;

/** "Today", "Yesterday", or the long date, by the viewer's own calendar days. */
export function dayHeading(iso: string, now: Date = new Date()): string {
	const day = new Date(iso);
	const today = new Date(now);

	today.setHours(0, 0, 0, 0);
	day.setHours(0, 0, 0, 0);

	const daysAgo = Math.round((today.getTime() - day.getTime()) / DAY_MS);

	if (daysAgo === 0) return m.activity_day_today();
	if (daysAgo === 1) return m.activity_day_yesterday();

	return formatLongDate(iso);
}
