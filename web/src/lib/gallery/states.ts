import type { GalleryState } from './schemas.js';

export const STATE_LABELS: Record<GalleryState, string> = {
	draft: 'Draft',
	shared: 'Shared',
	delivered: 'Delivered',
	archived: 'Archived',
	expired: 'Expired'
};

/** Every state, in the order the filter lists them. */
export const ALL_STATES = Object.keys(STATE_LABELS) as GalleryState[];

/** The progress state comes first in `states`; the rest are flags. */
export function splitStates(states: GalleryState[]) {
	const [progress, ...flags] = states;

	return { progress, flags };
}
