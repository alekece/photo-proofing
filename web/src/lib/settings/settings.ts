import * as v from 'valibot';
import { apiErrorMessage, readResponseErrorCode } from '#lib/i18n/errors.js';

export const MARKS = ['name', 'logo', 'both'] as const;
export const PATTERNS = ['repeat', 'once'] as const;
export const SIGNATURES = ['name', 'logo', 'both', 'none'] as const;
export const QUALITIES = ['low', 'medium', 'high'] as const;
export const SIZE_STEPS = 10;
/** The size step that matches the original fixed look. */
export const DEFAULT_SIZE = 4;
/** Cover signature scale per size step; the default step is the original size. */
const SIGNATURE_SCALES = [0.5, 0.6, 0.7, 0.85, 1, 1.2, 1.45, 1.75, 2.1, 2.5];

export type Mark = (typeof MARKS)[number];
export type Pattern = (typeof PATTERNS)[number];
export type Signature = (typeof SIGNATURES)[number];
export type Quality = (typeof QUALITIES)[number];

/** Workspace settings shared by every gallery. */
export type WorkspaceSettings = {
	studioName: string;
	/** Image URL of the studio logo; `null` until one is uploaded. */
	logo: string | null;
	mark: Mark;
	pattern: Pattern;
	/** Cell of the 3 × 3 grid, 0 (top left) to 8 (bottom right); ignored while the pattern repeats. */
	position: number;
	/** Step from 0 (small) to `SIZE_STEPS - 1` (large). */
	size: number;
	signature: Signature;
	/** Step from 0 (small) to `SIZE_STEPS - 1` (large) of the cover signature. */
	signatureSize: number;
	/** How sharp thumbnails and previews are; changing it regenerates all of them. */
	proofQuality: Quality;
};

export const DEFAULT_SETTINGS: WorkspaceSettings = {
	studioName: '',
	logo: null,
	mark: 'name',
	pattern: 'repeat',
	position: 8,
	size: DEFAULT_SIZE,
	signature: 'name',
	signatureSize: DEFAULT_SIZE,
	proofQuality: 'medium'
};

/** Whether saving needs the watermarked proofs rebuilt. */
export function watermarkChanged(a: WorkspaceSettings, b: WorkspaceSettings) {
	return (
		a.studioName !== b.studioName ||
		a.logo !== b.logo ||
		a.mark !== b.mark ||
		a.pattern !== b.pattern ||
		a.position !== b.position ||
		a.size !== b.size
	);
}

/** Whether saving needs every thumbnail and preview rebuilt. */
export function qualityChanged(a: WorkspaceSettings, b: WorkspaceSettings) {
	return a.proofQuality !== b.proofQuality;
}

export function settingsChanged(a: WorkspaceSettings, b: WorkspaceSettings) {
	return (
		watermarkChanged(a, b) ||
		qualityChanged(a, b) ||
		a.signature !== b.signature ||
		a.signatureSize !== b.signatureSize
	);
}

/** Where the saved logo is served; the query makes the browser refetch after a replacement. */
export const LOGO_URL = '/api/v1/settings/logo';

/** What the API accepts, mirrored here to refuse a logo before uploading it. */
/** The image types accepted for a logo. */
export const LOGO_TYPES = ['image/png', 'image/jpeg'];

const settingsResponseSchema = v.object({
	studio_name: v.string(),
	mark: v.picklist(MARKS),
	pattern: v.picklist(PATTERNS),
	position: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(8)),
	size: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(SIZE_STEPS - 1)),
	signature: v.picklist(SIGNATURES),
	signature_size: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(SIZE_STEPS - 1)),
	proof_quality: v.picklist(QUALITIES),
	has_logo: v.boolean()
});

/** The settings as the page uses them, or `null` when `body` is not what the API returns. */
export function parseSettings(body: unknown, logoUrl: string = LOGO_URL): WorkspaceSettings | null {
	const result = v.safeParse(settingsResponseSchema, body);

	if (!result.success) return null;

	const settings = result.output;

	return {
		studioName: settings.studio_name,
		logo: settings.has_logo ? logoUrl : null,
		mark: settings.mark,
		pattern: settings.pattern,
		position: settings.position,
		size: settings.size,
		signature: settings.signature,
		signatureSize: settings.signature_size,
		proofQuality: settings.proof_quality
	};
}

/** How much larger than the original look a cover signature of this size step is drawn. */
export function signatureScale(step: number) {
	return SIGNATURE_SCALES[Math.min(Math.max(Math.trunc(step), 0), SIZE_STEPS - 1)];
}

export class SettingsApiError extends Error {
	constructor(
		public status: number,
		message: string,
		/** The API's error `code`, when it sent one. */
		public code?: string
	) {
		super(message);
	}
}

/** The `logo` field of a save: a new image is sent, a removed one dropped, anything else kept. */
function logoChange(next: string | null, saved: string | null) {
	if (next === null) return saved === null ? { action: 'keep' } : { action: 'remove' };

	const match = /^data:image\/(?:png|jpeg);base64,(.+)$/.exec(next);

	return match ? { action: 'replace', image_base64: match[1] } : { action: 'keep' };
}

export class SettingsApi {
	constructor(private fetcher: typeof fetch = (...args) => fetch(...args)) {}

	/** Saves every setting; `saved` is what the server holds now, so a removal can be told. */
	async save(settings: WorkspaceSettings, saved: WorkspaceSettings): Promise<WorkspaceSettings> {
		const response = await this.fetcher('/api/v1/settings', {
			method: 'PUT',
			credentials: 'same-origin',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({
				studio_name: settings.studioName,
				mark: settings.mark,
				pattern: settings.pattern,
				position: settings.position,
				size: settings.size,
				signature: settings.signature,
				signature_size: settings.signatureSize,
				proof_quality: settings.proofQuality,
				logo: logoChange(settings.logo, saved.logo)
			})
		});

		if (!response.ok) {
			const code = await readResponseErrorCode(response);

			throw new SettingsApiError(response.status, apiErrorMessage(response.status, code), code);
		}

		const logoReplaced = settings.logo?.startsWith('data:') ?? false;
		const parsed = parseSettings(
			await response.json().catch(() => null),
			logoReplaced ? `${LOGO_URL}?v=${Date.now()}` : (saved.logo ?? LOGO_URL)
		);

		if (!parsed) throw new SettingsApiError(502, apiErrorMessage(502));

		return parsed;
	}
}
