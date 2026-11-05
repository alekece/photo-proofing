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

export type { SharedGallery, SharedPhoto } from './schemas';
