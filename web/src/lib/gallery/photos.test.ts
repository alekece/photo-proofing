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

	it('deletes and retries one photo', async () => {
		const deleted = respond(204);
		const retried = respond(200, { count: 1 });

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
		const fetcher = respond(200, { count: 2 });

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
			new GalleryApiError(404, 'not found')
		);
		await expect(listPhotos('g1', respond(500))).rejects.toMatchObject({ status: 500 });
	});

	it('points at the private derivative routes', () => {
		expect(thumbnailUrl('g 1', 'p1')).toBe('/api/v1/galleries/g%201/photos/p1/thumbnail');
		expect(previewUrl('g1', 'p1')).toBe('/api/v1/galleries/g1/photos/p1/preview');
	});
});
