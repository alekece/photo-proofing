import { getContext, setContext } from 'svelte';
import { goto } from '$app/navigation';
import { parseOrNull, photoEventSchema, type PhotoEvent } from './gallery/schemas';

export type { PhotoChange, PhotoEvent } from './gallery/schemas';

/** The signed-in area's live updates. A resync means events may have been missed: refetch. */
export interface LiveEvents {
	onPhoto(handler: (event: PhotoEvent) => void): () => void;
	onResync(handler: () => void): () => void;
	start(): void;
	close(): void;
}

interface LiveEventsOptions {
	open?: () => EventSource;
	isSignedIn?: () => Promise<boolean>;
	onSignedOut?: () => void;
}

const FIRST_REOPEN_DELAY_MS = 1000;
const MAX_REOPEN_DELAY_MS = 30_000;
const CLOSED = 2;

async function isSignedIn(): Promise<boolean> {
	const response = await fetch('/api/v1/auth/status', { credentials: 'same-origin' });
	if (!response.ok) throw new Error(`auth status ${response.status}`);
	const status: unknown = await response.json();
	if (typeof status !== 'object' || status === null || !('identity' in status))
		throw new Error('invalid auth status');
	return status.identity !== null;
}

/**
 * One `EventSource` on `/api/v1/events`. The browser reconnects by itself after a network drop,
 * but a refused or ended stream closes for good without telling why. So on close this checks
 * the session: signed out goes to sign-in, otherwise the stream reopens after a growing delay.
 */
export function createLiveEvents({
	open = () => new EventSource('/api/v1/events'),
	isSignedIn: checkSignedIn = isSignedIn,
	onSignedOut = () => void goto('/login?expired=1', { refreshAll: true })
}: LiveEventsOptions = {}): LiveEvents {
	const photoHandlers = new Set<(event: PhotoEvent) => void>();
	const resyncHandlers = new Set<() => void>();
	let source: EventSource | null = null;
	let reopen: ReturnType<typeof setTimeout> | undefined;
	let delay = FIRST_REOPEN_DELAY_MS;
	let active = false;

	function connect() {
		const current = open();
		source = current;
		current.addEventListener('open', () => {
			delay = FIRST_REOPEN_DELAY_MS;
		});
		current.addEventListener('resync', () => {
			for (const handler of resyncHandlers) handler();
		});
		current.addEventListener('photo', (message) => {
			let data: unknown;
			try {
				data = JSON.parse((message as MessageEvent<string>).data);
			} catch {
				data = null;
			}
			const event = parseOrNull(photoEventSchema, data);
			// A payload the page can't trust is treated as a missed event: refetch instead.
			if (!event) {
				for (const handler of resyncHandlers) handler();
				return;
			}
			for (const handler of photoHandlers) handler(event);
		});
		current.addEventListener('error', () => {
			if (current.readyState === CLOSED) void closed();
		});
	}

	async function closed() {
		const signedIn = await checkSignedIn().catch(() => true);
		if (!active) return;
		if (!signedIn) {
			active = false;
			onSignedOut();
			return;
		}
		reopen = setTimeout(connect, delay);
		delay = Math.min(delay * 2, MAX_REOPEN_DELAY_MS);
	}

	function subscribe<T>(handlers: Set<T>, handler: T) {
		handlers.add(handler);
		return () => {
			handlers.delete(handler);
		};
	}

	return {
		onPhoto: (handler) => subscribe(photoHandlers, handler),
		onResync: (handler) => subscribe(resyncHandlers, handler),
		start() {
			if (active) return;
			active = true;
			connect();
		},
		close() {
			active = false;
			clearTimeout(reopen);
			source?.close();
		}
	};
}

const key = Symbol('live-events');

export function setLiveEvents(events: LiveEvents) {
	setContext(key, events);
}

/** The live events of the enclosing dashboard layout. */
export function getLiveEvents(): LiveEvents {
	return getContext(key);
}
