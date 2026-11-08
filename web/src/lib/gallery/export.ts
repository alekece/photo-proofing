import { GalleryApiError } from './api';
import { parseOrNull, exportSchema, type Export } from './schemas';
import { apiErrorMessage } from '#lib/i18n/errors.js';

export type { Export } from './schemas';

function exportPath(galleryId: string) {
	return `/api/v1/galleries/${encodeURIComponent(galleryId)}/export`;
}

async function call(
	path: string,
	method: 'GET' | 'POST',
	fetcher: typeof fetch,
	photoIds?: string[]
) {
	const response = await fetcher(path, {
		method,
		credentials: 'same-origin',
		...(photoIds === undefined
			? {}
			: {
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify({ photo_ids: photoIds })
				})
	});

	if (!response.ok) {
		throw new GalleryApiError(response.status, apiErrorMessage(response.status));
	}

	return response;
}

async function parse(response: Response) {
	const parsed = parseOrNull(exportSchema, await response.json().catch(() => null));

	if (!parsed) throw new GalleryApiError(502, apiErrorMessage(502));

	return parsed;
}

/** Stores favorite proofs and queues their ZIP; an active preparation rejects a competing request. */
export async function requestExport(
	galleryId: string,
	fetcher: typeof fetch = fetch,
	photoIds?: string[]
) {
	return parse(await call(exportPath(galleryId), 'POST', fetcher, photoIds));
}

/** The newest ZIP of the gallery, or `null` when there is none or it expired. */
export async function latestExport(
	galleryId: string,
	fetcher: typeof fetch = fetch
): Promise<Export | null> {
	const response = await call(exportPath(galleryId), 'GET', fetcher);

	return response.status === 204 ? null : parse(response);
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
