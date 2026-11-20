import { getLiveEvents } from '#lib/events.js';
import { ApiError } from '#lib/http.js';
import { apiErrorMessage } from '#lib/i18n/errors.js';
import { m } from '#lib/paraglide/messages.js';
import { exportUrl, latestExport, requestExport, startDownload, type Export } from './export';
import type { PhotoSet } from './photos';

type Session = { id: string; load: number; awaited?: string };

/** One preparation slot for a gallery's owner ZIPs, without adopting results from a prior gallery. */
export class OwnerExport {
	#session?: Session;
	#latest = $state.raw<Export | null>(null);
	#requesting = $state(false);
	#refreshing = $state(false);
	#requestError = $state<string | null>(null);
	#statusError = $state<string | null>(null);
	get busy() {
		return this.#requesting || this.#latest?.status.kind === 'preparing';
	}
	get refreshing() {
		return this.#refreshing;
	}
	get statusError() {
		return this.#statusError;
	}
	get error(): string | null {
		const status = this.#latest?.status;
		return (
			this.#requestError ?? (status?.kind === 'failed' ? apiErrorMessage(409, status.code) : null)
		);
	}

	constructor(
		readonly galleryId: () => string,
		readonly set: PhotoSet,
		readonly enabled: () => boolean = () => true
	) {
		const events = getLiveEvents();
		$effect(() => {
			this.#latest = null;
			this.#requesting = false;
			this.#refreshing = false;
			this.#requestError = null;
			this.#statusError = null;
			if (!this.enabled()) return;

			const session = { id: this.galleryId(), load: 0 };
			this.#session = session;
			const refresh = () => void this.#refresh(session);
			const offExport = events.onExport(({ gallery_id }) => {
				if (gallery_id === session.id) refresh();
			});
			const offResync = events.onResync(refresh);
			refresh();
			return () => {
				this.#session = undefined;
				offExport();
				offResync();
			};
		});
	}

	#current(session: Session): boolean {
		return session === this.#session && this.enabled() && this.galleryId() === session.id;
	}

	#adopt(session: Session, latest: Export | null) {
		this.#latest = latest;
		const awaited = session.awaited;
		if (!awaited || (latest?.id === awaited && latest.status.kind === 'preparing')) return;

		session.awaited = undefined;
		if (latest?.id !== awaited) {
			this.#requestError = m.owner_zip_no_longer_available();
		} else if (latest.status.kind === 'ready') {
			startDownload(exportUrl(session.id, latest.id, this.set));
		}
	}

	async #refresh(session: Session) {
		const load = ++session.load;
		this.#refreshing = true;
		try {
			const latest = await latestExport(session.id, fetch, this.set);
			if (!this.#current(session) || load !== session.load) return;
			this.#statusError = null;
			this.#adopt(session, latest);
		} catch {
			if (this.#current(session) && load === session.load)
				this.#statusError = m.favorites_zip_check_failed();
		} finally {
			if (this.#current(session) && load === session.load) this.#refreshing = false;
		}
	}

	retryStatus() {
		const session = this.#session;
		if (!session || !this.#current(session) || !this.#statusError || this.#refreshing) return;
		return this.#refresh(session);
	}

	async request(photoIds?: string[]): Promise<void> {
		const session = this.#session;
		if (!session || !this.#current(session) || this.busy) return;
		this.#requesting = true;
		this.#requestError = null;
		this.#statusError = null;

		try {
			const created = await requestExport(session.id, fetch, photoIds, this.set);
			if (!this.#current(session)) return;
			session.load++;
			this.#refreshing = false;
			session.awaited = created.id;
			this.#adopt(session, created);
			// A completion event can race the POST response, so check the authoritative status.
			if (created.status.kind === 'preparing') await this.#refresh(session);
		} catch (error) {
			if (this.#current(session)) {
				this.#requestError =
					error instanceof ApiError && error.status !== 502
						? error.message
						: m.favorites_zip_failed();
			}
		} finally {
			if (this.#current(session)) this.#requesting = false;
		}
	}
}
