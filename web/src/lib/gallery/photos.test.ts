import { describe, expect, it, vi } from 'vitest';
import { GalleryApiError } from './api';
import {
	deletePhoto,
	deletePhotos,
	getUploadLimits,
	listPhotos,
	previewUrl,
	retryPhoto,
	thumbnailUrl,
	uploadPhoto
} from './photos';

const photo = {
	id: 'p1',
	original_name: 'a.jpg',
	size: 3,
	width: 2,
	height: 1,
	status: 'waiting',
	favorite: false,
	created_at: '2026-11-04T10:00:00Z'
};

function jpeg(name = 'été 1.jpg') {
	return new File([new Uint8Array([1, 2, 3])], name, { type: 'image/jpeg' });
}

function respond(status: number, body?: unknown, headers?: HeadersInit) {
	return vi
		.fn()
		.mockResolvedValue(
			new Response(body === undefined ? null : JSON.stringify(body), { status, headers })
		);
}

describe('uploadPhoto', () => {
	it('sends the raw file with its encoded name', async () => {
		const fetcher = respond(201, photo);
		const file = jpeg();
		const signal = new AbortController().signal;

		expect(await uploadPhoto('g 1', file, signal, fetcher)).toEqual({ kind: 'created', photo });
		expect(fetcher).toHaveBeenCalledWith('/api/v1/galleries/g%201/photos', {
			method: 'POST',
			body: file,
			signal,
			credentials: 'same-origin',
			headers: { 'Content-Type': 'image/jpeg', 'X-Filename': encodeURIComponent('été 1.jpg') }
		});
	});

	it.each([
		[429, { 'Retry-After': '3' }, { kind: 'busy', retryAfterMs: 3000 }],
		[429, {}, { kind: 'busy', retryAfterMs: 1000 }],
		[507, {}, { kind: 'storage-full' }],
		[413, {}, { kind: 'rejected', reason: 'Too large' }],
		[415, {}, { kind: 'rejected', reason: 'Not a JPEG' }],
		[422, {}, { kind: 'rejected', reason: 'Unreadable image' }],
		[404, {}, { kind: 'gone' }],
		[401, {}, { kind: 'signed-out' }],
		[500, {}, { kind: 'failed' }],
		[503, {}, { kind: 'failed' }]
	])('maps %i to its outcome', async (status, headers, outcome) => {
		const fetcher = respond(status, { error: 'x' }, headers);

		expect(await uploadPhoto('g1', jpeg(), new AbortController().signal, fetcher)).toEqual(outcome);
	});

	it('reports a network error as failed', async () => {
		const fetcher = vi.fn().mockRejectedValue(new TypeError('network'));

		expect(await uploadPhoto('g1', jpeg(), new AbortController().signal, fetcher)).toEqual({
			kind: 'failed'
		});
	});

	it('rethrows an abort', async () => {
		const fetcher = vi.fn().mockRejectedValue(new DOMException('aborted', 'AbortError'));

		await expect(
			uploadPhoto('g1', jpeg(), new AbortController().signal, fetcher)
		).rejects.toMatchObject({ name: 'AbortError' });
	});
});

describe('photo requests', () => {
	it('lists a gallery’s photos', async () => {
		const fetcher = respond(200, [photo]);

		expect(await listPhotos('g1', fetcher)).toEqual([photo]);
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/g1/photos',
			expect.objectContaining({ credentials: 'same-origin' })
		);
	});

	it('addresses finals with the finals query', async () => {
		const fetcher = respond(200, [photo]);
		const deleted = respond(200, { deleted: 1 });

		await listPhotos('g1', fetcher, 'finals');
		await deletePhotos('g1', ['p1'], deleted, 'finals');

		expect(fetcher.mock.calls[0][0]).toBe('/api/v1/galleries/g1/photos?finals=true');
		expect(deleted.mock.calls[0][0]).toBe('/api/v1/galleries/g1/photos/delete?finals=true');
		expect(thumbnailUrl('g1', 'p1', 'finals')).toBe(
			'/api/v1/galleries/g1/photos/p1/thumbnail?finals=true'
		);
		expect(previewUrl('g1', 'p1')).toBe('/api/v1/galleries/g1/photos/p1/preview');
	});

	it('deletes and retries one photo', async () => {
		const deleted = respond(204);
		const retried = respond(200, { requeued: 1 });

		await deletePhoto('g1', 'p 1', deleted);
		await retryPhoto('g1', 'p 1', retried);

		expect(deleted).toHaveBeenCalledWith(
			'/api/v1/galleries/g1/photos/p%201',
			expect.objectContaining({ method: 'DELETE' })
		);
		expect(retried).toHaveBeenCalledWith(
			'/api/v1/galleries/g1/photos/p%201/retry',
			expect.objectContaining({ method: 'POST' })
		);
	});

	it('deletes several photos in one request', async () => {
		const fetcher = respond(200, { deleted: 2 });

		expect(await deletePhotos('g 1', ['p1', 'p2'], fetcher)).toBe(2);
		expect(fetcher).toHaveBeenCalledWith('/api/v1/galleries/g%201/photos/delete', {
			method: 'POST',
			body: JSON.stringify({ photo_ids: ['p1', 'p2'] }),
			headers: { 'Content-Type': 'application/json' },
			credentials: 'same-origin'
		});
	});

	it('reads the upload limits', async () => {
		const limits = { concurrency: 4, max_size: 1024, max_pixels: 2048 };
		const fetcher = respond(200, limits);

		expect(await getUploadLimits(fetcher)).toEqual(limits);
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/upload/limits',
			expect.objectContaining({ credentials: 'same-origin' })
		);
	});

	it('throws a GalleryApiError with the status on failure', async () => {
		await expect(deletePhoto('g1', 'p1', respond(404, { error: 'not found' }))).rejects.toEqual(
			new GalleryApiError(404, 'This item no longer exists.')
		);
		await expect(listPhotos('g1', respond(500))).rejects.toMatchObject({ status: 500 });
	});

	it('points at the private derivative routes', () => {
		expect(thumbnailUrl('g 1', 'p1')).toBe('/api/v1/galleries/g%201/photos/p1/thumbnail');
		expect(previewUrl('g1', 'p1')).toBe('/api/v1/galleries/g1/photos/p1/preview');
	});
});

describe('malformed success bodies', () => {
	it('fails an upload whose created photo is not a photo', async () => {
		const fetcher = respond(201, { id: 'p' });

		expect(await uploadPhoto('g1', jpeg(), new AbortController().signal, fetcher)).toEqual({
			kind: 'failed'
		});
	});

	it.each([
		['listPhotos', (f: typeof fetch) => listPhotos('g1', f), { not: 'a list' }],
		['listPhotos entries', (f: typeof fetch) => listPhotos('g1', f), [{ id: 'p' }]],
		['deletePhotos', (f: typeof fetch) => deletePhotos('g1', ['p'], f), { deleted: 'many' }],
		['getUploadLimits', (f: typeof fetch) => getUploadLimits(f), { concurrency: 4 }]
	])('%s rejects an unexpected body', async (_name, call, body) => {
		await expect(call(respond(200, body))).rejects.toBeInstanceOf(GalleryApiError);
	});
});
