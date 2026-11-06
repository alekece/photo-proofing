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

/**
 * Seam for the workspace settings and watermark storage from #67: until that API exists nothing
 * is persisted, so the page starts from the defaults and a save only resolves.
 */
export class SettingsApi {
	async save(settings: WorkspaceSettings): Promise<WorkspaceSettings> {
		return settings;
	}
}
