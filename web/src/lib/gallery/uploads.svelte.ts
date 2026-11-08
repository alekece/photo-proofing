import { getContext, setContext } from 'svelte';
import { goto } from '$app/navigation';
import {
	deletePhoto,
	getUploadLimits,
	uploadPhoto,
	type Photo,
	type PhotoSet,
	type UploadLimits,
	type UploadOutcome
} from './photos';

export type QueueState = 'waiting' | 'uploading' | 'failed' | 'rejected';

/** A file the browser still holds; once the server accepts it, the photo list takes over. */
export interface UploadItem {
	key: string;
	galleryId: string;
	set: PhotoSet;
	file: File;
	state: QueueState;
	error?: string;
}

interface UploadQueueOptions {
	upload?: (
		galleryId: string,
		file: File,
		signal: AbortSignal,
		set: PhotoSet
	) => Promise<UploadOutcome>;
	limits?: () => Promise<UploadLimits>;
	remove?: (galleryId: string, photoId: string, set: PhotoSet) => Promise<void>;
	onSignedOut?: () => void;
}

/** What the queue tracks about a file besides what tiles show. */
interface Sending {
	controller?: AbortController;
	notBefore: number;
}

function isJpeg(file: File) {
	return file.type === 'image/jpeg' || /\.jpe?g$/i.test(file.name);
}

/**
 * The signed-in area's uploads, across galleries and page changes. It sends the oldest waiting
 * files, never more at once than the server's upload concurrency, and puts a file back to wait for the server's `Retry-After` on a busy answer.
 */
export class UploadQueue {
	items = $state<UploadItem[]>([]);

	#upload;
	#limits;
	#remove;
	#onSignedOut;
	// Bookkeeping that nothing renders, so plain collections on purpose.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#uploadedHandlers = new Set<(galleryId: string, photo: Photo, set: PhotoSet) => void>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#sending = new Map<string, Sending>();
	#serverLimits: Pick<UploadLimits, 'concurrency' | 'max_size'> | null = null;
	#readingLimits = false;
	#wake: ReturnType<typeof setTimeout> | undefined;
	#nextKey = 0;

	constructor({
		upload = (galleryId, file, signal, set) => uploadPhoto(galleryId, file, signal, fetch, set),
		limits = getUploadLimits,
		remove = (galleryId, photoId, set) => deletePhoto(galleryId, photoId, fetch, set),
		onSignedOut = () => void goto('/login?expired=1', { refreshAll: true })
	}: UploadQueueOptions = {}) {
		this.#upload = upload;
		this.#limits = limits;
		this.#remove = remove;
		this.#onSignedOut = onSignedOut;
	}

	/** Whether any file is still to be sent or being sent. */
	get hasPending() {
		return this.items.some(({ state }) => state === 'waiting' || state === 'uploading');
	}

	add(galleryId: string, files: Iterable<File>, set: PhotoSet = 'photos') {
		for (const file of files) {
			const key = `upload-${++this.#nextKey}`;

			this.items.push(
				isJpeg(file)
					? { key, galleryId, set, file, state: 'waiting' }
					: { key, galleryId, set, file, state: 'rejected', error: 'Not a JPEG' }
			);
		}

		this.#pump();
	}

	retry(key: string) {
		const item = this.#find(key);

		if (item?.state !== 'failed') return;

		item.state = 'waiting';
		item.error = undefined;
		this.#sending.delete(key);
		this.#pump();
	}

	/** Forgets a file at once; an upload in flight is aborted. */
	cancel(key: string) {
		this.#forget((item) => item.key === key);
		this.#pump();
	}

	forGallery(galleryId: string, set: PhotoSet = 'photos') {
		return this.items.filter((item) => item.galleryId === galleryId && item.set === set);
	}

	/** Files of this gallery that may still become photos. */
	inFlight(galleryId: string, set: PhotoSet = 'photos') {
		return this.items.filter(
			(item) =>
				item.galleryId === galleryId &&
				item.set === set &&
				(item.state === 'waiting' || item.state === 'uploading')
		).length;
	}

	onUploaded(handler: (galleryId: string, photo: Photo, set: PhotoSet) => void) {
		this.#uploadedHandlers.add(handler);

		return () => {
			this.#uploadedHandlers.delete(handler);
		};
	}

	/** `beforeunload` handler: the browser asks before leaving while files are unsent. */
	guard(event: BeforeUnloadEvent) {
		if (this.hasPending) event.preventDefault();
	}

	clear() {
		clearTimeout(this.#wake);
		this.#forget(() => true);
	}

	#find(key: string) {
		return this.items.find((item) => item.key === key);
	}

	#forget(matches: (item: UploadItem) => boolean) {
		for (const item of this.items.filter(matches)) {
			this.#sending.get(item.key)?.controller?.abort();
			this.#sending.delete(item.key);
		}

		this.items = this.items.filter((item) => !matches(item));
	}

	#pump() {
		const waiting = this.items.filter((item) => item.state === 'waiting');

		if (waiting.length === 0) return;

		const limits = this.#serverLimits;

		if (limits === null) {
			this.#readLimits();

			return;
		}

		const now = Date.now();
		let uploading = this.items.filter((item) => item.state === 'uploading').length;
		let wakeAt = Infinity;

		for (const item of waiting) {
			if (uploading >= limits.concurrency) break;

			const notBefore = this.#sending.get(item.key)?.notBefore ?? 0;

			if (notBefore > now) {
				wakeAt = Math.min(wakeAt, notBefore);
				continue;
			}

			if (item.file.size > limits.max_size) {
				item.state = 'rejected';
				item.error = 'Too large';
				continue;
			}

			uploading++;
			void this.#send(item);
		}

		clearTimeout(this.#wake);

		if (wakeAt !== Infinity) this.#wake = setTimeout(() => this.#pump(), wakeAt - now);
	}

	#readLimits() {
		if (this.#readingLimits) return;

		this.#readingLimits = true;
		this.#limits()
			.then(
				(limits) => {
					this.#serverLimits = limits;
				},
				() => {
					// Without limits nothing can be sent; Retry reads them again.
					for (const item of this.items) {
						if (item.state === 'waiting') this.#fail(item, 'Upload failed');
					}
				}
			)
			.finally(() => {
				this.#readingLimits = false;
				this.#pump();
			});
	}

	async #send(item: UploadItem) {
		const { key, galleryId, set, file } = item;
		const controller = new AbortController();
		let outcome: UploadOutcome | null;

		item.state = 'uploading';
		this.#sending.set(key, { controller, notBefore: 0 });

		try {
			outcome = await this.#upload(galleryId, file, controller.signal, set);
		} catch {
			outcome = controller.signal.aborted ? null : { kind: 'failed' };
		}

		const sent = this.#sending.get(key);

		if (sent !== undefined) sent.controller = undefined;

		const current = this.#find(key);

		if (current === undefined) {
			// Cancelled; the server may have saved it anyway.
			if (outcome?.kind === 'created')
				void this.#remove(galleryId, outcome.photo.id, set).catch(() => {});
		} else if (outcome !== null) {
			this.#settle(current, outcome);
		}

		this.#pump();
	}

	#settle(item: UploadItem, outcome: UploadOutcome) {
		switch (outcome.kind) {
			case 'created':
				this.#forget((other) => other.key === item.key);

				for (const handler of this.#uploadedHandlers)
					handler(item.galleryId, outcome.photo, item.set);

				break;
			case 'busy':
				// The file goes back in the queue until the server says it may try again.
				this.#sending.set(item.key, { notBefore: Date.now() + outcome.retryAfterMs });
				item.state = 'waiting';

				break;
			case 'storage-full':
				this.#fail(item, 'Storage full');

				for (const other of this.items) {
					if (other.state === 'waiting') this.#fail(other, 'Storage full');
				}

				break;
			case 'rejected':
				item.state = 'rejected';
				item.error = outcome.reason;

				break;
			case 'failed':
				this.#fail(item, 'Upload failed');

				break;
			case 'gone':
				this.#forget((other) => other.galleryId === item.galleryId);

				break;
			case 'signed-out':
				this.clear();
				this.#onSignedOut();

				break;
		}
	}

	#fail(item: UploadItem, error: string) {
		this.#sending.delete(item.key);
		item.state = 'failed';
		item.error = error;
	}
}

const key = Symbol('upload-queue');

export function setUploadQueue(queue: UploadQueue) {
	setContext(key, queue);
}

/** The upload queue of the enclosing dashboard layout. */
export function getUploadQueue(): UploadQueue {
	return getContext(key);
}
