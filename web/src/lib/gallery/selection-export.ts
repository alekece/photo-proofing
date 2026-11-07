import { GalleryApiError } from './api';
import { parseOrNull, selectionExportSchema, type SelectionExport } from './schemas';

export type { SelectionExport } from './schemas';

function exportPath(galleryId: string) {
	return `/api/v1/galleries/${encodeURIComponent(galleryId)}/selection-export`;
}

async function call(path: string, method: 'GET' | 'POST', fetcher: typeof fetch) {
	const response = await fetcher(path, { method, credentials: 'same-origin' });

	if (!response.ok) {
		const data = await response.json().catch(() => ({}));

		throw new GalleryApiError(
			response.status,
			typeof data.error === 'string' ? data.error : 'Unable to prepare the ZIP.'
		);
	}

	return response;
}

async function parse(response: Response) {
	const parsed = parseOrNull(selectionExportSchema, await response.json().catch(() => null));

	if (!parsed) throw new GalleryApiError(502, 'Unexpected response from the server.');

	return parsed;
}

/** Stores the current favorites and queues their ZIP; a ZIP already being prepared is returned as is. */
export async function requestSelectionExport(galleryId: string, fetcher: typeof fetch = fetch) {
	return parse(await call(exportPath(galleryId), 'POST', fetcher));
}

/** The newest ZIP of the gallery, or `null` when there is none or it expired. */
export async function latestSelectionExport(
	galleryId: string,
	fetcher: typeof fetch = fetch
): Promise<SelectionExport | null> {
	const response = await call(exportPath(galleryId), 'GET', fetcher);

	return response.status === 204 ? null : parse(response);
}

export function selectionExportUrl(galleryId: string, exportId: string) {
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
