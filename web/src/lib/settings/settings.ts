import * as v from 'valibot';

export const MARKS = ['name', 'logo', 'both'] as const;
export const PATTERNS = ['repeat', 'once'] as const;
export const SIGNATURES = ['name', 'logo', 'both', 'none'] as const;
export const SIZE_STEPS = 5;

export type Mark = (typeof MARKS)[number];
export type Pattern = (typeof PATTERNS)[number];
export type Signature = (typeof SIGNATURES)[number];

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
};

export const DEFAULT_SETTINGS: WorkspaceSettings = {
	studioName: '',
	logo: null,
	mark: 'name',
	pattern: 'repeat',
	position: 8,
	size: 2,
	signature: 'name'
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

export function settingsChanged(a: WorkspaceSettings, b: WorkspaceSettings) {
	return watermarkChanged(a, b) || a.signature !== b.signature;
}

/** Where the saved logo is served; the query makes the browser refetch after a replacement. */
export const LOGO_URL = '/api/v1/settings/logo';

/** What the API accepts, mirrored here to refuse a logo before uploading it. */
export const LOGO_MAX_BYTES = 1024 * 1024;
/** The image types accepted for a logo. */
export const LOGO_TYPES = ['image/png', 'image/jpeg'];

const settingsResponseSchema = v.object({
	studio_name: v.string(),
	mark: v.picklist(MARKS),
	pattern: v.picklist(PATTERNS),
	position: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(8)),
	size: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(SIZE_STEPS - 1)),
	signature: v.picklist(SIGNATURES),
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
		signature: settings.signature
	};
}

export class SettingsApiError extends Error {
	constructor(
		public status: number,
		message: string,
		/** The setting a 422 is about (`context[0]` of the error body). */
		public field?: string
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
				logo: logoChange(settings.logo, saved.logo)
			})
		});

		if (!response.ok) {
			const data = await response.json().catch(() => ({}));

			throw new SettingsApiError(
				response.status,
				typeof data.error === 'string' ? data.error : 'Unable to complete the request.',
				Array.isArray(data.context) && typeof data.context[0] === 'string'
					? data.context[0]
					: undefined
			);
		}

		const logoReplaced = settings.logo?.startsWith('data:') ?? false;
		const parsed = parseSettings(
			await response.json().catch(() => null),
			logoReplaced ? `${LOGO_URL}?v=${Date.now()}` : (saved.logo ?? LOGO_URL)
		);

		if (!parsed) throw new SettingsApiError(502, 'Unexpected response from the server.');

		return parsed;
	}
}
