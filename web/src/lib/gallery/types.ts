export interface Proof {
	id: string;
	alt: string;
	/** Proof preview URL only; source and final assets must never appear here. */
	previewUrl: string;
}

/** What the private link reveals to a client; mirrors the API's `SharedGallery`. */
export interface SharedGallery {
	title: string | null;
	layout: 'grid' | 'masonry';
	/** ISO 8601 access expiry; `null` means the link never expires. */
	expires_at: string | null;
}
