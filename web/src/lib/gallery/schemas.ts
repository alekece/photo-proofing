import * as v from 'valibot';

/**
 * The shapes the API returns, described once: each type is inferred from its schema. Schemas
 * check only the fields the web app reads, so the API can add fields freely.
 */

const finite = v.pipe(v.number(), v.finite());

export const galleryLayoutSchema = v.picklist(['grid', 'masonry']);
export const gallerySortSchema = v.picklist(['updated', 'created', 'oldest', 'title']);
export const galleryStateSchema = v.picklist([
	'draft',
	'shared',
	'delivered',
	'archived',
	'expired'
]);

export const gallerySchema = v.object({
	id: v.string(),
	title: v.nullable(v.string()),
	layout: galleryLayoutSchema,
	expires_at: v.nullable(v.string()),
	included_photos: v.nullable(finite),
	watermark_enabled: v.boolean(),
	/** The ready photo starred as the gallery's cover; `null` means none. */
	hero_id: v.nullable(v.string()),
	/** The private client link's token once shared; the link never changes afterwards. */
	share_token: v.nullable(v.string()),
	/** The progress state first, then `archived` and `expired` when they apply. */
	states: v.array(galleryStateSchema),
	created_at: v.string(),
	updated_at: v.string()
});

/** A gallery in the dashboard list, with totals over its ready photos. */
export const galleryOverviewSchema = v.object({
	...gallerySchema.entries,
	photo_count: v.pipe(finite, v.integer(), v.minValue(0)),
	/** How many of the ready photos the client chose. */
	favorite_count: v.pipe(finite, v.integer(), v.minValue(0))
});

/** What the private link reveals to a client; mirrors the API's `SharedGallery`. */
export const sharedGallerySchema = v.object({
	title: v.nullable(v.string()),
	layout: galleryLayoutSchema,
	/** ISO 8601 access expiry; `null` means the link never expires. */
	expires_at: v.nullable(v.string()),
	/** How many photos the package includes; `null` means no limit. */
	included_photos: v.nullable(finite),
	/** Once delivered, the selection is final and read-only. */
	delivered: v.boolean(),
	/** The cover photo, only while it is ready; `null` means no cover. */
	hero_id: v.nullable(v.string())
});

/** A photo a client may see; mirrors the API's `ReadyPhoto`. */
export const sharedPhotoSchema = v.object({ id: v.string(), width: finite, height: finite });

export const photoStatusSchema = v.union([
	v.picklist(['waiting', 'processing', 'ready']),
	v.object({ failed: v.string() })
]);

export const photoSchema = v.object({
	id: v.string(),
	original_name: v.string(),
	size: finite,
	width: finite,
	height: finite,
	status: photoStatusSchema,
	/** Whether the client selected the photo. */
	favorite: v.boolean(),
	created_at: v.string()
});

export const uploadLimitsSchema = v.object({
	concurrency: finite,
	max_size: finite,
	max_pixels: finite
});

export const deletedPhotosSchema = v.object({ deleted: v.pipe(v.number(), v.integer()) });

export const photoChangeSchema = v.picklist([
	'waiting',
	'processing',
	'ready',
	'failed',
	'deleted'
]);

/** One committed change to a photo of the signed-in workspace; details come from the photo list. */
export const photoEventSchema = v.object({
	gallery_id: v.string(),
	photo_id: v.string(),
	kind: v.optional(v.picklist(['proof', 'final']), 'proof'),
	status: photoChangeSchema
});

/** The favorite ids of a shared gallery, in the photographer's photo order. */
export const favoriteIdsSchema = v.array(v.string());

/** A change to the selection of a gallery of the signed-in workspace; the favorites come from a refetch. */
export const favoritesEventSchema = v.object({ gallery_id: v.string() });

/** A change to a selection ZIP of a gallery of the signed-in workspace; its state comes from a refetch. */
export const exportEventSchema = v.object({ gallery_id: v.string() });

/** Mirrors the API's `ExportStatus`: the failure reason travels with the `failed` kind. */
export const exportStatusSchema = v.variant('kind', [
	v.object({ kind: v.literal('preparing') }),
	v.object({ kind: v.literal('ready') }),
	v.object({ kind: v.literal('failed'), reason: v.string() })
]);

export const exportSchema = v.object({
	id: v.string(),
	status: exportStatusSchema,
	photo_count: v.number(),
	created_at: v.string()
});

export type Gallery = v.InferOutput<typeof gallerySchema>;
export type Export = v.InferOutput<typeof exportSchema>;
export type ExportEvent = v.InferOutput<typeof exportEventSchema>;
export type GalleryOverview = v.InferOutput<typeof galleryOverviewSchema>;
export type GalleryLayout = v.InferOutput<typeof galleryLayoutSchema>;
export type GallerySort = v.InferOutput<typeof gallerySortSchema>;
export type GalleryState = v.InferOutput<typeof galleryStateSchema>;
export type SharedGallery = v.InferOutput<typeof sharedGallerySchema>;
export type SharedPhoto = v.InferOutput<typeof sharedPhotoSchema>;
export type Photo = v.InferOutput<typeof photoSchema>;
export type PhotoStatus = v.InferOutput<typeof photoStatusSchema>;
export type UploadLimits = v.InferOutput<typeof uploadLimitsSchema>;
export type PhotoChange = v.InferOutput<typeof photoChangeSchema>;
export type PhotoEvent = v.InferOutput<typeof photoEventSchema>;
export type FavoritesEvent = v.InferOutput<typeof favoritesEventSchema>;

/** The value as `schema` describes it, or `null` when it does not match. */
export function parseOrNull<const S extends v.GenericSchema>(
	schema: S,
	value: unknown
): v.InferOutput<S> | null {
	const result = v.safeParse(schema, value);

	return result.success ? result.output : null;
}

/** An unknown sort, such as one typed into the URL, means the default order. */
export function parseGallerySort(value: unknown): GallerySort {
	return parseOrNull(gallerySortSchema, value) ?? 'updated';
}

/** The valid states among `values`, once each and in order; unknown ones, such as a typo in the URL, are dropped. */
export function parseGalleryStates(values: unknown[]): GalleryState[] {
	const states: GalleryState[] = [];

	for (const value of values) {
		const state = parseOrNull(galleryStateSchema, value);

		if (state !== null && !states.includes(state)) states.push(state);
	}

	return states;
}
