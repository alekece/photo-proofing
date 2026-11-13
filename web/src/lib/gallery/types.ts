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

export type {
	CoverLayout,
	FocalPoint,
	GridDensity,
	GridSpacing,
	HeadingFont,
	SharedGallery,
	SharedPhoto
} from './schemas';
