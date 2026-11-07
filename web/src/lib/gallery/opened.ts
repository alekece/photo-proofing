/**
 * Tells the API that a client opened the shared gallery. Fire and forget: a missed ping must
 * never affect viewing, and the API ignores repeats.
 */
export async function recordOpened(token: string, fetcher: typeof fetch = fetch): Promise<void> {
	try {
		await fetcher(`/api/v1/galleries/shared/${encodeURIComponent(token)}/open`, {
			method: 'POST',
			keepalive: true
		});
	} catch {
		// Nothing to recover: the next visit reports it.
	}
}
