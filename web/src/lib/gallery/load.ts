import { MOCK_GALLERIES } from '../mock/galleries';
import { toClientView } from './access';
import type { ClientGalleryView } from './types';

/**
 * API seam: replace the mock lookup with a fetch of the gallery by share token.
 * Unknown and unavailable galleries must stay indistinguishable.
 */
export async function loadClientGallery(
	token: string,
	now: Date = new Date()
): Promise<ClientGalleryView> {
	const record = Object.hasOwn(MOCK_GALLERIES, token) ? MOCK_GALLERIES[token] : null;
	return toClientView(record, now);
}
