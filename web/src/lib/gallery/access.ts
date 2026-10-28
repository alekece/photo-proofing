import type { ClientGalleryView, GalleryRecord, GalleryState } from './types';

const PROOF_STATES: ReadonlySet<GalleryState> = new Set([
	'proofing',
	'selection_received',
	'retouching'
]);

/**
 * Client-facing mirror of the API access rules. The API stays authoritative;
 * this only keeps the UI from rendering anything the backend would refuse.
 */
export function toClientView(record: GalleryRecord | null, now: Date): ClientGalleryView {
	if (
		!record ||
		record.state === 'draft' ||
		record.state === 'expired' ||
		new Date(record.expiresAt).getTime() <= now.getTime()
	) {
		return { access: 'unavailable' };
	}
	return {
		access: 'granted',
		title: record.title,
		state: record.state,
		expiresAt: record.expiresAt,
		proofs: PROOF_STATES.has(record.state) ? record.proofs : []
	};
}
