export interface Proof {
	id: string;
	alt: string;
	/** Proof thumbnail and preview URLs only; source and final assets must never appear here. */
	thumbnailUrl: string;
	previewUrl: string;
	/** Displayed size in pixels, so the page reserves the photo's real shape. */
	width: number;
	height: number;
	/** Defaults to ready; the owner's viewer also shows photos still processing or failed. */
	/** Set once the gallery is delivered: the finished, unwatermarked photo to download. */
	downloadUrl?: string;
	status?: 'ready' | 'processing' | 'failed';
}

/** One explicit link-sharing or invitation action, bound to the access-code intent at its start. */
export interface ShareAttempt {
	prepare(): Promise<boolean>;
	isReady(): boolean;
}

export type {
	CoverLayout,
	FocalPoint,
	GridDensity,
	GridSpacing,
	HeadingFont,
	LockedSharedGallery,
	SharedGallery,
	SharedGalleryResponse,
	SharedPhoto
} from './schemas';
