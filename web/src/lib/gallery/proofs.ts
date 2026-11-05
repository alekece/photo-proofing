import { previewUrl, thumbnailUrl, type Photo } from './photos';
import type { Proof, SharedPhoto } from './types';

/** Points each photo at the same-origin client media routes behind the private link. */
export function toProofs(token: string, photos: SharedPhoto[]): Proof[] {
	const base = `/api/v1/galleries/shared/${encodeURIComponent(token)}/photos`;

	return photos.map(({ id, width, height }, i) => ({
		id,
		alt: `Proof ${i + 1}`,
		thumbnailUrl: `${base}/${encodeURIComponent(id)}/thumbnail`,
		previewUrl: `${base}/${encodeURIComponent(id)}/preview`,
		width,
		height
	}));
}

/** The owner's ready photos as proofs, served from their authenticated preview routes. */
export function toOwnerProofs(galleryId: string, photos: Photo[]): Proof[] {
	return photos
		.filter(({ status }) => status === 'ready')
		.map(({ id, width, height }, i) => ({
			id,
			alt: `Proof ${i + 1}`,
			thumbnailUrl: thumbnailUrl(galleryId, id),
			previewUrl: previewUrl(galleryId, id),
			width,
			height
		}));
}
