import * as v from 'valibot';
import { requestEmpty, requestJson } from '#lib/http.js';

export const ACTIVITY_KINDS = [
	'open',
	'favorite',
	'unfavorite',
	'comment',
	'download',
	'email',
	'email_failed'
] as const;
export const ACTIVITY_RANGES = ['today', '7d', '30d'] as const;

export type ActivityKind = (typeof ACTIVITY_KINDS)[number];
export type ActivityRange = (typeof ACTIVITY_RANGES)[number];
export type ActivitySort = 'newest' | 'oldest';

export const activityEventSchema = v.object({
	id: v.string(),
	gallery_id: v.string(),
	gallery_title: v.nullable(v.string()),
	/** `null` for an anonymous client. */
	contact: v.nullable(v.object({ id: v.string(), name: v.string() })),
	kind: v.picklist(ACTIVITY_KINDS),
	can_retry: v.optional(v.boolean()),
	count: v.pipe(v.number(), v.integer(), v.minValue(0)),
	photo_ids: v.array(v.string()),
	detail: v.nullable(v.string()),
	first_at: v.string(),
	last_at: v.string()
});

export const activityPageSchema = v.object({
	events: v.array(activityEventSchema),
	next: v.nullable(v.string()),
	tail: v.nullish(v.string())
});

export type ActivityEvent = v.InferOutput<typeof activityEventSchema>;
export type ActivityPage = v.InferOutput<typeof activityPageSchema>;

/** Requeues the single invitation associated with this Activity line. */
export function retryEmail(event: ActivityEvent, fetcher: typeof fetch = fetch): Promise<void> {
	return requestEmpty(
		`/api/v1/galleries/${encodeURIComponent(event.gallery_id)}/timeline/${encodeURIComponent(event.id)}/retry`,
		{ method: 'POST' },
		fetcher
	);
}

/** The filters the API takes; `contact` is a contact id or `anonymous`. */
export interface ActivityFilters {
	q?: string;
	gallery?: string;
	contact?: string;
	kind?: ActivityKind;
	range?: ActivityRange;
}

/** One page of the activity log: the workspace's, or a single gallery's when `galleryId` is given. */
export function listActivity(
	params: ActivityFilters & { sort?: ActivitySort; cursor?: string },
	galleryId?: string,
	fetcher: typeof fetch = fetch
): Promise<ActivityPage> {
	const query = new URLSearchParams();

	for (const [key, value] of Object.entries(params)) {
		if (value) query.set(key, value);
	}

	const path =
		galleryId === undefined
			? '/api/v1/timeline'
			: `/api/v1/galleries/${encodeURIComponent(galleryId)}/timeline`;
	const suffix = query.size > 0 ? `?${query}` : '';

	return requestJson(`${path}${suffix}`, { method: 'GET' }, activityPageSchema, fetcher);
}
