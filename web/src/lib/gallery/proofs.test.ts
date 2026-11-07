import { describe, expect, it } from 'vitest';
import { toClientPreviewProofs, toOwnerProofs, toProofs } from './proofs';
import type { Photo } from './photos';

describe('toProofs', () => {
	it('points each proof at the client media routes, in order, with its size', () => {
		const photos = [
			{ id: 'a', width: 300, height: 200 },
			{ id: 'b', width: 200, height: 300 }
		];

		expect(toProofs('to/ken', photos)).toEqual([
			{
				id: 'a',
				alt: 'Proof 1',
				thumbnailUrl: '/api/v1/galleries/shared/to%2Fken/photos/a/thumbnail',
				previewUrl: '/api/v1/galleries/shared/to%2Fken/photos/a/preview',
				width: 300,
				height: 200
			},
			{
				id: 'b',
				alt: 'Proof 2',
				thumbnailUrl: '/api/v1/galleries/shared/to%2Fken/photos/b/thumbnail',
				previewUrl: '/api/v1/galleries/shared/to%2Fken/photos/b/preview',
				width: 200,
				height: 300
			}
		]);
	});
});

describe('toOwnerProofs', () => {
	it('keeps clean photos visible during marked processing and failure', () => {
		const photos: Photo[] = ['processing', { failed: 'Marked render failed' }].map((status, i) => ({
			id: `${i}`,
			original_name: 'photo.jpg',
			size: 1,
			width: 300,
			height: 200,
			status: status as Photo['status'],
			clean_ready: true,
			favorite: false,
			created_at: ''
		}));

		expect(toOwnerProofs('g1', photos).map(({ id }) => id)).toEqual(['0', '1']);
	});

	it('changes the first preview URL when its rendered asset changes', () => {
		const original = {
			id: 'a',
			original_name: 'a.jpg',
			size: 1,
			width: 300,
			height: 200,
			status: 'ready' as const,
			favorite: false,
			created_at: '2026-01-01T10:00:00Z'
		};
		const first = toOwnerProofs('g1', [{ ...original, revision: 1 }]);
		const next = toOwnerProofs('g1', [{ ...original, revision: 2 }]);

		expect(first[0].previewUrl).not.toBe(next[0].previewUrl);
		expect(next[0].previewUrl).toBe('/api/v1/galleries/g1/photos/a/preview?revision=2');
	});

	describe('client preview media', () => {
		it('uses authenticated client policy URLs and changes identity on a toggle', () => {
			const photos = [{ id: 'p1', width: 300, height: 200, revision: 2 }];
			const off = toClientPreviewProofs('g1', photos, false);
			const on = toClientPreviewProofs('g1', photos, true);

			expect(on[0].previewUrl).toBe(
				'/api/v1/galleries/g1/client-preview/photos/p1/preview?watermark=on&revision=2'
			);
			expect(off[0].previewUrl).not.toBe(on[0].previewUrl);
		});
	});

	const photo = (id: string, status: Photo['status']): Photo => ({
		id,
		original_name: `${id}.jpg`,
		size: 1,
		width: 300,
		height: 200,
		status,
		favorite: false,
		created_at: '2026-01-01T10:00:00Z'
	});

	it('keeps only ready photos and points them at the owner preview routes', () => {
		const proofs = toOwnerProofs('g/1', [
			photo('a', 'ready'),
			photo('b', 'processing'),
			photo('c', { failed: 'bad' }),
			photo('d', 'ready')
		]);

		expect(proofs).toEqual([
			{
				id: 'a',
				alt: 'Proof 1',
				thumbnailUrl: '/api/v1/galleries/g%2F1/photos/a/thumbnail',
				previewUrl: '/api/v1/galleries/g%2F1/photos/a/preview',
				width: 300,
				height: 200
			},
			{
				id: 'd',
				alt: 'Proof 2',
				thumbnailUrl: '/api/v1/galleries/g%2F1/photos/d/thumbnail',
				previewUrl: '/api/v1/galleries/g%2F1/photos/d/preview',
				width: 300,
				height: 200
			}
		]);
	});
});
