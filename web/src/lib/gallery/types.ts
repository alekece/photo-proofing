export interface Proof {
	id: string;
	alt: string;
	/** Proof thumbnail and preview URLs only; source and final assets must never appear here. */
	thumbnailUrl: string;
	previewUrl: string;
	/** Displayed size in pixels, so the page reserves the photo's real shape. */
	width: number;
	height: number;
}

/** A photo a client may see; mirrors the API's `ReadyPhoto`. */
export interface SharedPhoto {
	id: string;
	width: number;
	height: number;
}

/** What the private link reveals to a client; mirrors the API's `SharedGallery`. */
export interface SharedGallery {
	title: string | null;
	layout: 'grid' | 'masonry';
	/** ISO 8601 access expiry; `null` means the link never expires. */
	expires_at: string | null;
}
