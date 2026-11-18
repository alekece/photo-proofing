import * as v from 'valibot';
import { parseOrNull, workspacePhotoSchema, type WorkspacePhoto } from '#lib/gallery/schemas.js';
import { readWorkspaceJson } from './http.js';

const UNAVAILABLE = 'Photos are temporarily unavailable. Please try again.';

/** Server-side read of every ready photo in the workspace; 401 redirects, other failures become 503. */
export function listWorkspacePhotos(
	fetcher: typeof fetch,
	cookie: string | null
): Promise<WorkspacePhoto[]> {
	return readWorkspaceJson(
		fetcher,
		cookie,
		'/api/v1/photos',
		(body) => parseOrNull(v.array(workspacePhotoSchema), body),
		{ unavailable: UNAVAILABLE }
	);
}
