import { describe, expect, it } from 'vitest';
import { toOwnerProofs, toProofs } from './proofs';
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
	const photo = (id: string, status: Photo['status']): Photo => ({
		id,
		original_name: `${id}.jpg`,
		size: 1,
		width: 300,
		height: 200,
		status,
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
