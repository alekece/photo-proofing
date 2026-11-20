import { requestJson } from '#lib/http.js';
import type { PhotoSet } from './photos';
import { exportSchema, type Export } from './schemas';

export type { Export } from './schemas';

function exportPath(galleryId: string) {
	return `/api/v1/galleries/${encodeURIComponent(galleryId)}/export`;
}

const setQuery = (set: PhotoSet) => (set === 'finals' ? '?finals=true' : '');

/** Queues an owner ZIP of favorite proofs or ready finals, optionally as an exact subset. */
export function requestExport(
	galleryId: string,
	fetcher: typeof fetch = fetch,
	photoIds?: string[],
	set: PhotoSet = 'photos'
): Promise<Export> {
	return requestJson(
		exportPath(galleryId) + setQuery(set),
		{
			method: 'POST',
			...(photoIds === undefined ? {} : { body: JSON.stringify({ photo_ids: photoIds }) })
		},
		exportSchema,
		fetcher
	);
}

/** The newest ZIP of the gallery, or `null` when there is none or it expired. */
export function latestExport(
	galleryId: string,
	fetcher: typeof fetch = fetch,
	set: PhotoSet = 'photos'
): Promise<Export | null> {
	return requestJson(
		exportPath(galleryId) + setQuery(set),
		{ method: 'GET' },
		exportSchema,
		fetcher,
		{ emptyAsNull: true }
	);
}

export function exportUrl(galleryId: string, exportId: string, set: PhotoSet = 'photos') {
	return `${exportPath(galleryId)}/${encodeURIComponent(exportId)}/download${setQuery(set)}`;
}

/** Starts the browser's download of `url`; the response is an attachment, so the page stays. */
export function startDownload(url: string) {
	const link = document.createElement('a');

	link.href = url;
	link.download = '';
	link.hidden = true;
	document.body.append(link);
	link.click();
	link.remove();
}
