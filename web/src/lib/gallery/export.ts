import { requestJson } from '#lib/http.js';
import { exportSchema, type Export } from './schemas';

export type { Export } from './schemas';

function exportPath(galleryId: string) {
	return `/api/v1/galleries/${encodeURIComponent(galleryId)}/export`;
}

/** Stores favorite proofs and queues their ZIP; an active preparation rejects a competing request. */
export function requestExport(
	galleryId: string,
	fetcher: typeof fetch = fetch,
	photoIds?: string[]
): Promise<Export> {
	return requestJson(
		exportPath(galleryId),
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
	fetcher: typeof fetch = fetch
): Promise<Export | null> {
	return requestJson(exportPath(galleryId), { method: 'GET' }, exportSchema, fetcher, {
		emptyAsNull: true
	});
}

export function exportUrl(galleryId: string, exportId: string) {
	return `${exportPath(galleryId)}/${encodeURIComponent(exportId)}/download`;
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
