import type * as v from 'valibot';
import { GalleryApiError } from './api';
import { commentCountsSchema, commentSchema, commentsSchema, parseOrNull } from './schemas';
import { apiErrorMessage, readResponseErrorCode } from '#lib/i18n/errors.js';

/** Both credentials use the same operations; the server decides the author. */
export class CommentsClient {
	constructor(
		private base: string,
		private fetcher: typeof fetch = fetch
	) {}

	private async request(path: string, method = 'GET', body?: string): Promise<Response> {
		const response = await this.fetcher(`${this.base}${path}`, {
			method,
			credentials: 'same-origin',
			cache: 'no-store',
			...(body !== undefined
				? { headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ body }) }
				: {})
		});
		if (!response.ok) {
			const code = await readResponseErrorCode(response);
			throw new GalleryApiError(response.status, apiErrorMessage(response.status, code), code);
		}
		return response;
	}

	private async read<S extends v.GenericSchema>(
		response: Response,
		schema: S
	): Promise<v.InferOutput<S>> {
		const result = parseOrNull(schema, await response.json().catch(() => null));
		if (result === null) throw new GalleryApiError(502, apiErrorMessage(502));
		return result;
	}

	async counts(): Promise<Map<string, number>> {
		const counts = await this.read(await this.request('/comments'), commentCountsSchema);
		return new Map(counts.map(({ photo_id, count }) => [photo_id, count]));
	}

	async thread(photoId: string) {
		return this.read(
			await this.request(`/photos/${encodeURIComponent(photoId)}/comments`),
			commentsSchema
		);
	}

	async add(photoId: string, body: string) {
		return this.read(
			await this.request(`/photos/${encodeURIComponent(photoId)}/comments`, 'POST', body),
			commentSchema
		);
	}

	async edit(id: string, body: string) {
		return this.read(
			await this.request(`/comments/${encodeURIComponent(id)}`, 'PATCH', body),
			commentSchema
		);
	}

	async remove(id: string): Promise<void> {
		await this.request(`/comments/${encodeURIComponent(id)}`, 'DELETE');
	}
}
