import { hasCleanPhoto, previewUrl, thumbnailUrl, type Photo } from './photos';
import type { Proof, SharedPhoto } from './types';

/** Points each photo at the same-origin client media routes behind the private link. */
export function toProofs(token: string, photos: SharedPhoto[]): Proof[] {
	const base = `/api/v1/galleries/shared/${encodeURIComponent(token)}/photos`;

	return mediaProofs(base, photos);
}

function mediaProofs(
	base: string,
	photos: SharedPhoto[],
	variant?: 'watermarked' | 'clean'
): Proof[] {
	return photos.map(({ id, width, height, revision }, i) => ({
		id,
		alt: `Proof ${i + 1}`,
		thumbnailUrl: `${base}/${encodeURIComponent(id)}/thumbnail${imageRevision(revision, variant)}`,
		previewUrl: `${base}/${encodeURIComponent(id)}/preview${imageRevision(revision, variant)}`,
		width,
		height
	}));
}

function imageRevision(revision?: number, variant?: 'watermarked' | 'clean'): string {
	const query = new URLSearchParams();

	if (variant !== undefined) query.set('variant', variant);
	if (revision !== undefined) query.set('revision', String(revision));

	return query.size ? `?${query}` : '';
}

export function toClientPreviewProofs(
	galleryId: string,
	photos: SharedPhoto[],
	watermarkEnabled: boolean
): Proof[] {
	return mediaProofs(
		`/api/v1/galleries/${encodeURIComponent(galleryId)}/photos`,
		photos,
		watermarkEnabled ? 'watermarked' : 'clean'
	);
}

/** The owner's ready photos as proofs, served from their authenticated preview routes. */
export function toOwnerProofs(galleryId: string, photos: Photo[]): Proof[] {
	return photos.filter(hasCleanPhoto).map(({ id, width, height, revision }, i) => ({
		id,
		alt: `Proof ${i + 1}`,
		thumbnailUrl: thumbnailUrl(galleryId, id, 'photos', revision),
		previewUrl: previewUrl(galleryId, id, 'photos', revision),
		width,
		height
	}));
}
