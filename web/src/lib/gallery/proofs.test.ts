import { describe, expect, it } from 'vitest';
import { toProofs } from './proofs';

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
				previewUrl: '/api/v1/galleries/shared/to%2Fken/photos/a/image',
				width: 300,
				height: 200
			},
			{
				id: 'b',
				alt: 'Proof 2',
				thumbnailUrl: '/api/v1/galleries/shared/to%2Fken/photos/b/thumbnail',
				previewUrl: '/api/v1/galleries/shared/to%2Fken/photos/b/image',
				width: 200,
				height: 300
			}
		]);
	});
});
