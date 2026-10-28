// MOCK DATA: replaced by the API once it exists. Only gallery/load.ts imports this, and it
// must only run server-side (+page.server.ts) so draft/expired records never reach the browser.
import type { GalleryRecord, GalleryState } from '../gallery/types';
import { placeholderImage } from './placeholder';

const proofs = Array.from({ length: 9 }, (_, i) => ({
	id: `proof-${i + 1}`,
	alt: `Proof ${i + 1} of 9`,
	previewUrl: placeholderImage(`Proof ${i + 1}`, i, i % 3 !== 2)
}));

const gallery = (state: GalleryState, expiresAt: string): GalleryRecord => ({
	title: 'Sample session',
	state,
	expiresAt,
	proofs
});

const FAR_FUTURE = '2099-01-01T00:00:00Z';

export const MOCK_GALLERIES: Record<string, GalleryRecord> = {
	'demo-proofing': gallery('proofing', FAR_FUTURE),
	'demo-selection': gallery('selection_received', FAR_FUTURE),
	'demo-retouching': gallery('retouching', FAR_FUTURE),
	'demo-delivery': gallery('delivery', FAR_FUTURE),
	'demo-draft': gallery('draft', FAR_FUTURE),
	'demo-expired': gallery('expired', '2020-01-01T00:00:00Z')
};
