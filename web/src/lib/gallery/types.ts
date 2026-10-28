export type GalleryState =
	| 'draft'
	| 'proofing'
	| 'selection_received'
	| 'retouching'
	| 'delivery'
	| 'expired';

export interface Proof {
	id: string;
	alt: string;
	/** Proof preview URL only; source and final assets must never appear here. */
	previewUrl: string;
}

export interface GalleryRecord {
	title: string;
	state: GalleryState;
	/** ISO 8601 access expiry. */
	expiresAt: string;
	proofs: Proof[];
}

export interface GrantedGalleryView {
	access: 'granted';
	title: string;
	state: Exclude<GalleryState, 'draft' | 'expired'>;
	expiresAt: string;
	proofs: Proof[];
}

/** Deliberately carries no gallery data so it cannot leak anything. */
export interface UnavailableGalleryView {
	access: 'unavailable';
}

export type ClientGalleryView = GrantedGalleryView | UnavailableGalleryView;
