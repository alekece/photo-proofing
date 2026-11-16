import { describe, expect, it, vi } from 'vitest';
import { loadClientPreviewData } from './preview.js';

const makePhoto = (id: string, over: Record<string, unknown> = {}) => ({
	id,
	original_name: `${id}.jpg`,
	size: 100,
	width: 1200,
	height: 800,
	status: 'ready',
	clean_ready: true,
	favorited_at: null,
	created_at: '2026-06-01T10:00:00Z',
	...over
});

describe('loadClientPreviewData', () => {
	it('loads proof photos, watermarked client proofs, and favorite IDs without fetching settings', async () => {
		const p1 = makePhoto('p1', { favorited_at: '2026-06-02T10:00:00Z' });
		const p2 = makePhoto('p2', { status: 'waiting', clean_ready: false });
		const fetcher = vi.fn<typeof fetch>(async (url) => {
			const path = String(url);
			if (path === '/api/v1/galleries/g1/photos') return Response.json([p1, p2]);
			if (path === '/api/v1/galleries/g1/photos?watermark=true') {
				return Response.json([{ id: 'p1', width: 1200, height: 800 }]);
			}
			throw new Error(`Unexpected URL: ${path}`);
		});

		const data = await loadClientPreviewData(
			'g1',
			{ delivered: false, watermarked: true },
			fetcher
		);

		expect(data.proofPhotos.map((p) => p.id)).toEqual(['p1', 'p2']);
		expect(data.favorites).toEqual(['p1']);
		expect(data.proofs).toHaveLength(1);
		expect(data.proofs[0]?.thumbnailUrl).toBe(
			'/api/v1/galleries/g1/photos/p1/thumbnail?watermark=true'
		);
		expect(fetcher.mock.calls.map(([url]) => String(url))).toEqual([
			'/api/v1/galleries/g1/photos',
			'/api/v1/galleries/g1/photos?watermark=true'
		]);
	});

	it('loads only finals when delivered and includeOwnerProofs is omitted', async () => {
		const f1 = makePhoto('f1');
		const fetcher = vi.fn<typeof fetch>(async (url) => {
			const path = String(url);
			if (path === '/api/v1/galleries/g1/photos?finals=true') return Response.json([f1]);
			throw new Error(`Unexpected URL: ${path}`);
		});

		const data = await loadClientPreviewData('g1', { delivered: true, watermarked: true }, fetcher);

		expect(data.proofPhotos).toEqual([]);
		expect(data.favorites).toEqual([]);
		expect(data.proofs).toHaveLength(1);
		expect(data.proofs[0]?.downloadUrl).toBe('/api/v1/galleries/g1/photos/f1/preview?finals=true');
		expect(fetcher).toHaveBeenCalledTimes(1);
	});

	it('also loads owner proof photos for cover picking when delivered and includeOwnerProofs is true', async () => {
		const p1 = makePhoto('p1', { favorited_at: '2026-06-02T10:00:00Z' });
		const f1 = makePhoto('f1');
		const fetcher = vi.fn<typeof fetch>(async (url) => {
			const path = String(url);
			if (path === '/api/v1/galleries/g1/photos') return Response.json([p1]);
			if (path === '/api/v1/galleries/g1/photos?finals=true') return Response.json([f1]);
			throw new Error(`Unexpected URL: ${path}`);
		});

		const data = await loadClientPreviewData(
			'g1',
			{ delivered: true, watermarked: false, includeOwnerProofs: true },
			fetcher
		);

		expect(data.proofPhotos.map((p) => p.id)).toEqual(['p1']);
		expect(data.favorites).toEqual([]);
		expect(data.proofs.map((p) => p.id)).toEqual(['f1']);
	});
});
