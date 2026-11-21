import { requestEmpty, requestJson } from '#lib/http.js';
import { commentCountsSchema, commentSchema, commentsSchema } from './schemas';

/** Both credentials use the same operations; the server decides the author. */
export class CommentsClient {
	constructor(
		private base: string,
		private fetcher: typeof fetch = fetch
	) {}

	async summaries() {
		const summaries = await requestJson(
			`${this.base}/comments`,
			{ method: 'GET', cache: 'no-store' },
			commentCountsSchema,
			this.fetcher
		);
		return new Map(summaries.map(({ photo_id, ...summary }) => [photo_id, summary]));
	}

	thread(photoId: string) {
		return requestJson(
			`${this.base}/photos/${encodeURIComponent(photoId)}/comments`,
			{ method: 'GET', cache: 'no-store' },
			commentsSchema,
			this.fetcher
		);
	}

	add(photoId: string, body: string) {
		return requestJson(
			`${this.base}/photos/${encodeURIComponent(photoId)}/comments`,
			{ method: 'POST', cache: 'no-store', body: JSON.stringify({ body }) },
			commentSchema,
			this.fetcher
		);
	}

	edit(id: string, body: string) {
		return requestJson(
			`${this.base}/comments/${encodeURIComponent(id)}`,
			{ method: 'PATCH', cache: 'no-store', body: JSON.stringify({ body }) },
			commentSchema,
			this.fetcher
		);
	}

	remove(id: string): Promise<void> {
		return requestEmpty(
			`${this.base}/comments/${encodeURIComponent(id)}`,
			{ method: 'DELETE', cache: 'no-store' },
			this.fetcher
		);
	}
}
