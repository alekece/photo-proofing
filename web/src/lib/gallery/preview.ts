import { listClientPreviewPhotos, listPhotos, type Photo } from './photos.js';
import { toClientPreviewProofs, toOwnerProofs } from './proofs.js';
import type { Proof } from './types.js';

export type PreviewViewport = 'desktop' | 'phone';

export interface ClientPreviewData {
	proofPhotos: Photo[];
	proofs: Proof[];
	favorites: string[];
}

/**
 * Loads the photos and favorite IDs rendered in owner-side client previews (`ClientPreview` and
 * `GalleryDesignPanel`). Delivered galleries show only finals without proof favorites; studio
 * design passes `includeOwnerProofs: true` so the cover picker still lists proof photos.
 */
export async function loadClientPreviewData(
	galleryId: string,
	options: {
		delivered: boolean;
		watermarked: boolean;
		includeOwnerProofs?: boolean;
	},
	fetcher: typeof fetch = fetch
): Promise<ClientPreviewData> {
	if (options.delivered) {
		const [proofPhotos, finalPhotos] = await Promise.all([
			options.includeOwnerProofs ? listPhotos(galleryId, fetcher) : Promise.resolve([] as Photo[]),
			listPhotos(galleryId, fetcher, 'finals')
		]);

		return {
			proofPhotos,
			proofs: toOwnerProofs(galleryId, finalPhotos, 'finals').map((proof) => ({
				...proof,
				downloadUrl: proof.previewUrl
			})),
			favorites: []
		};
	}

	const [proofPhotos, visibleProofs] = await Promise.all([
		listPhotos(galleryId, fetcher),
		listClientPreviewPhotos(galleryId, options.watermarked, fetcher)
	]);

	return {
		proofPhotos,
		proofs: toClientPreviewProofs(galleryId, visibleProofs, options.watermarked),
		favorites: proofPhotos.filter(({ favorited_at }) => favorited_at).map(({ id }) => id)
	};
}
