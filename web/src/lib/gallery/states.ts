import { m } from '#lib/paraglide/messages.js';
import type { GalleryState } from './schemas.js';

/** The name of each state, in the workspace language. */
export const STATE_LABELS: Record<GalleryState, () => string> = {
	draft: m.state_draft,
	shared: m.state_shared,
	delivered: m.state_delivered,
	expired: m.state_expired,
	archived: m.state_archived
};

/** Every state, in the order the filter lists them. */
export const ALL_STATES = Object.keys(STATE_LABELS) as GalleryState[];

/** The progress state comes first in `states`; the rest are flags. */
export function splitStates(states: GalleryState[]) {
	const [progress, ...flags] = states;

	return { progress, flags };
}

/** What the dashboard shows until the photographer chooses otherwise: everything but Archived. */
export const DEFAULT_STATES: GalleryState[] = ALL_STATES.filter((state) => state !== 'archived');

/** Whether two choices pick the same states, whatever their order. */
export function sameStates(a: readonly GalleryState[], b: readonly GalleryState[]): boolean {
	return a.length === b.length && a.every((state) => b.includes(state));
}
