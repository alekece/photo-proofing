import * as v from 'valibot';
import { requestJson } from '#lib/http.js';
import { rgbHexSchema } from './color.js';

export const MARKS = ['name', 'logo', 'both'] as const;
export const PATTERNS = ['repeat', 'once'] as const;
export const SIGNATURES = ['name', 'logo', 'both', 'none'] as const;
export const QUALITIES = ['low', 'medium', 'high'] as const;
const MODES = ['full_image', 'print'] as const;
const PALETTES = ['warm', 'neutral'] as const;
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
	studioMode: (typeof MODES)[number];
	studioPalette: (typeof PALETTES)[number];
	studioName: string;
	/** Image URL of the studio logo; `null` until one is uploaded. */
	logo: string | null;
	mark: Mark;
	pattern: Pattern;
	/** Cell of the 3 × 3 grid, 0 (top left) to 8 (bottom right); ignored while the pattern repeats. */
	position: number;
	/** Step from 0 (small) to `SIZE_STEPS - 1` (large). */
	size: number;
	watermarkOpacity: number;
	watermarkTextColor: string;
	signature: Signature;
	/** Step from 0 (small) to `SIZE_STEPS - 1` (large) of the cover signature. */
	signatureSize: number;
	/** Overrides the cover's inherited text color only for the studio signature. */
	signatureTextColor: string | null;
	/** How sharp thumbnails and previews are; changing it regenerates all of them. */
	proofQuality: Quality;
	/** Whether `[smtp]` is configured on the server so email invitations can be queued. */
	emailEnabled?: boolean;
};

export const DEFAULT_SETTINGS: WorkspaceSettings = {
	studioMode: 'full_image',
	studioPalette: 'warm',
	studioName: '',
	logo: null,
	mark: 'name',
	pattern: 'repeat',
	position: 8,
	size: DEFAULT_SIZE,
	watermarkOpacity: 50,
	watermarkTextColor: '#FFFFFF',
	signature: 'name',
	signatureSize: DEFAULT_SIZE,
	signatureTextColor: null,
	proofQuality: 'medium',
	emailEnabled: false
};

/** Whether any persisted watermark control has changed, even if it is inactive. */
export function watermarkChanged(a: WorkspaceSettings, b: WorkspaceSettings) {
	return (
		a.studioName !== b.studioName ||
		a.logo !== b.logo ||
		a.mark !== b.mark ||
		a.pattern !== b.pattern ||
		a.position !== b.position ||
		a.size !== b.size ||
		a.watermarkOpacity !== b.watermarkOpacity ||
		a.watermarkTextColor !== b.watermarkTextColor
	);
}

/** Whether saving needs every thumbnail and preview rebuilt. */
export function qualityChanged(a: WorkspaceSettings, b: WorkspaceSettings) {
	return a.proofQuality !== b.proofQuality;
}

export function settingsChanged(a: WorkspaceSettings, b: WorkspaceSettings) {
	return (
		watermarkChanged(a, b) ||
		a.studioMode !== b.studioMode ||
		a.studioPalette !== b.studioPalette ||
		qualityChanged(a, b) ||
		a.signature !== b.signature ||
		a.signatureSize !== b.signatureSize ||
		a.signatureTextColor !== b.signatureTextColor
	);
}

/** Where the saved logo is served; the query makes the browser refetch after a replacement. */
export const LOGO_URL = '/api/v1/settings/logo';

/** What the API accepts, mirrored here to refuse a logo before uploading it. */
/** The image types accepted for a logo. */
export const LOGO_TYPES = ['image/png', 'image/jpeg'];

/** A UI draft may omit the leading hash but must describe a complete RGB color. */
export function normalizeWatermarkHex(value: string): string | null {
	const hex = value.trim().replace(/^#/, '');

	return /^[0-9a-f]{6}$/i.test(hex) ? `#${hex.toUpperCase()}` : null;
}

const settingsResponseSchema = v.object({
	studio_mode: v.optional(v.picklist(MODES), 'full_image'),
	studio_palette: v.optional(v.picklist(PALETTES), 'warm'),
	studio_name: v.string(),
	mark: v.picklist(MARKS),
	pattern: v.picklist(PATTERNS),
	position: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(8)),
	size: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(SIZE_STEPS - 1)),
	watermark_opacity: v.optional(
		v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(100)),
		DEFAULT_SETTINGS.watermarkOpacity
	),
	watermark_text_color: v.optional(rgbHexSchema, DEFAULT_SETTINGS.watermarkTextColor),
	signature: v.picklist(SIGNATURES),
	signature_size: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(SIZE_STEPS - 1)),
	signature_color: v.nullish(rgbHexSchema, null),
	proof_quality: v.picklist(QUALITIES),
	has_logo: v.boolean(),
	email_enabled: v.optional(v.boolean(), false)
});

function toWorkspaceSettings(
	settings: v.InferOutput<typeof settingsResponseSchema>,
	logoUrl: string
): WorkspaceSettings {
	return {
		studioName: settings.studio_name,
		studioMode: settings.studio_mode,
		studioPalette: settings.studio_palette,
		logo: settings.has_logo ? logoUrl : null,
		mark: settings.mark,
		pattern: settings.pattern,
		position: settings.position,
		size: settings.size,
		watermarkOpacity: settings.watermark_opacity,
		watermarkTextColor: settings.watermark_text_color,
		signature: settings.signature,
		signatureSize: settings.signature_size,
		signatureTextColor: settings.signature_color,
		proofQuality: settings.proof_quality,
		emailEnabled: settings.email_enabled
	};
}

/** The settings as the page uses them, or `null` when `body` is not what the API returns. */
export function parseSettings(body: unknown, logoUrl: string = LOGO_URL): WorkspaceSettings | null {
	const result = v.safeParse(settingsResponseSchema, body);

	return result.success ? toWorkspaceSettings(result.output, logoUrl) : null;
}

/** How much larger than the original look a cover signature of this size step is drawn. */
export function signatureScale(step: number) {
	return SIGNATURE_SCALES[Math.min(Math.max(Math.trunc(step), 0), SIZE_STEPS - 1)];
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
		const changedDisplay =
			settings.studioMode !== saved.studioMode || settings.studioPalette !== saved.studioPalette;
		const display = { studio_mode: settings.studioMode, studio_palette: settings.studioPalette };
		if (
			changedDisplay &&
			!settingsChanged(settings, {
				...saved,
				studioMode: settings.studioMode,
				studioPalette: settings.studioPalette
			})
		) {
			const stored = await requestJson(
				'/api/v1/settings/display',
				{
					method: 'PUT',
					body: JSON.stringify(display)
				},
				v.object({ studio_mode: v.picklist(MODES), studio_palette: v.picklist(PALETTES) }),
				this.fetcher
			);
			return { ...saved, studioMode: stored.studio_mode, studioPalette: stored.studio_palette };
		}

		const raw = await requestJson(
			'/api/v1/settings',
			{
				method: 'PUT',
				body: JSON.stringify({
					...(changedDisplay ? display : {}),
					studio_name: settings.studioName,
					mark: settings.mark,
					pattern: settings.pattern,
					position: settings.position,
					size: settings.size,
					watermark_opacity: settings.watermarkOpacity,
					watermark_text_color: settings.watermarkTextColor,
					signature: settings.signature,
					signature_size: settings.signatureSize,
					signature_color: settings.signatureTextColor,
					proof_quality: settings.proofQuality,
					logo: logoChange(settings.logo, saved.logo)
				})
			},
			settingsResponseSchema,
			this.fetcher
		);

		const logoReplaced = settings.logo?.startsWith('data:') ?? false;

		return toWorkspaceSettings(
			raw,
			logoReplaced ? `${LOGO_URL}?v=${Date.now()}` : (saved.logo ?? LOGO_URL)
		);
	}
}
