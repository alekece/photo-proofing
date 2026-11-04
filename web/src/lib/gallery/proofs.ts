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
