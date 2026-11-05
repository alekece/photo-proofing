import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
import { createLiveEvents, type PhotoEvent } from './events';

class FakeSource extends EventTarget {
	static readonly CLOSED = 2;
	readyState = 1;
	closed = false;
	close() {
		this.closed = true;
		this.readyState = FakeSource.CLOSED;
	}
	emit(type: string, data = '{}') {
		this.dispatchEvent(new MessageEvent(type, { data }));
	}
	fail(closed: boolean) {
		this.readyState = closed ? FakeSource.CLOSED : 0;
		this.dispatchEvent(new Event('error'));
	}
}

function setup(signedIn: () => Promise<boolean> = async () => true) {
	const sources: FakeSource[] = [];
	const onSignedOut = vi.fn();
	const events = createLiveEvents({
		open: () => {
			const source = new FakeSource();
			sources.push(source);
			return source as unknown as EventSource;
		},
		isSignedIn: signedIn,
		onSignedOut
	});
	return { events, sources, onSignedOut };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('live events', () => {
	it('opens only once started and dispatches photo and resync events to subscribers', () => {
		const { events, sources } = setup();
		const photos: PhotoEvent[] = [];
		const resyncs = vi.fn();
		const stop = events.onPhoto((event) => photos.push(event));
		events.onResync(resyncs);
		expect(sources).toHaveLength(0);

		events.start();
		const event = { gallery_id: 'g', photo_id: 'p', status: 'ready' };
		sources[0].emit('resync');
		sources[0].emit('photo', JSON.stringify(event));
		sources[0].emit('photo', 'not json');
		stop();
		sources[0].emit('photo', JSON.stringify(event));

		expect(resyncs).toHaveBeenCalledOnce();
		expect(photos).toEqual([event]);
	});

	it('leaves reconnecting after a network drop to the browser', async () => {
		const isSignedIn = vi.fn(async () => true);
		const { events, sources } = setup(isSignedIn);
		events.start();

		sources[0].fail(false);
		await vi.runAllTimersAsync();

		expect(isSignedIn).not.toHaveBeenCalled();
		expect(sources).toHaveLength(1);
	});

	it('reopens a closed stream with a growing delay while still signed in', async () => {
		const { events, sources, onSignedOut } = setup();
		events.start();

		sources[0].fail(true);
		await vi.advanceTimersByTimeAsync(999);
		expect(sources).toHaveLength(1);
		await vi.advanceTimersByTimeAsync(1);
		expect(sources).toHaveLength(2);

		sources[1].fail(true);
		await vi.advanceTimersByTimeAsync(1999);
		expect(sources).toHaveLength(2);
		await vi.advanceTimersByTimeAsync(1);
		expect(sources).toHaveLength(3);

		sources[2].emit('open');
		sources[2].fail(true);
		await vi.advanceTimersByTimeAsync(1000);
		expect(sources).toHaveLength(4);
		expect(onSignedOut).not.toHaveBeenCalled();
	});

	it('retries when the session cannot be checked', async () => {
		const { events, sources, onSignedOut } = setup(async () => {
			throw new Error('offline');
		});
		events.start();

		sources[0].fail(true);
		await vi.advanceTimersByTimeAsync(1000);

		expect(sources).toHaveLength(2);
		expect(onSignedOut).not.toHaveBeenCalled();
	});

	it('sends a signed-out user to sign-in without reopening', async () => {
		const { events, sources, onSignedOut } = setup(async () => false);
		events.start();

		sources[0].fail(true);
		await vi.runAllTimersAsync();

		expect(onSignedOut).toHaveBeenCalledOnce();
		expect(sources).toHaveLength(1);
	});

	it('closes the stream and cancels a pending reopen', async () => {
		const { events, sources } = setup();
		events.start();
		sources[0].fail(true);
		await Promise.resolve();

		events.close();
		await vi.runAllTimersAsync();

		expect(sources).toHaveLength(1);
		expect(sources[0].closed).toBe(true);
	});
});
