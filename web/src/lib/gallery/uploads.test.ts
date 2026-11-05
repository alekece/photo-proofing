import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Photo, UploadLimits, UploadOutcome } from './photos';
import { MAX_BUSY_RETRIES, UploadQueue } from './uploads.svelte';

interface Call {
	galleryId: string;
	file: File;
	signal: AbortSignal;
	resolve(outcome: UploadOutcome): void;
}

function jpeg(name: string, size = 3) {
	return new File([new Uint8Array(size)], name, { type: 'image/jpeg' });
}

function photo(id: string): Photo {
	return {
		id,
		original_name: `${id}.jpg`,
		size: 3,
		width: 2,
		height: 1,
		status: 'waiting',
		created_at: '2026-11-04T10:00:00Z'
	};
}

function setup(limits: Partial<UploadLimits> = {}) {
	const calls: Call[] = [];
	const upload = vi.fn(
		(galleryId: string, file: File, signal: AbortSignal) =>
			new Promise<UploadOutcome>((resolve) => calls.push({ galleryId, file, signal, resolve }))
	);
	const remove = vi.fn().mockResolvedValue(undefined);
	const onSignedOut = vi.fn();
	const getLimits = vi
		.fn()
		.mockResolvedValue({ concurrency: 2, max_size: 100, max_pixels: 1000, ...limits });
	const queue = new UploadQueue({ upload, limits: getLimits, remove, onSignedOut });

	return { queue, calls, upload, remove, onSignedOut, getLimits };
}

/** Lets pending promise callbacks run. */
async function settle() {
	await vi.advanceTimersByTimeAsync(0);
}

function states(queue: UploadQueue) {
	return queue.items.map((item) => [item.file.name, item.state, item.error]);
}

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
});

describe('UploadQueue', () => {
	it('adds one item per file in drop order', async () => {
		const { queue } = setup();

		queue.add('g1', [jpeg('a.jpg'), jpeg('b.JPEG'), jpeg('c.jpg')]);

		expect(queue.forGallery('g1').map((item) => item.file.name)).toEqual([
			'a.jpg',
			'b.JPEG',
			'c.jpg'
		]);
		expect(new Set(queue.items.map((item) => item.key)).size).toBe(3);
	});

	it('never sends more at once than the server allows', async () => {
		const { queue, calls, getLimits } = setup({ concurrency: 2 });

		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg'), jpeg('c.jpg')]);
		await settle();

		expect(calls.map((call) => call.file.name)).toEqual(['a.jpg', 'b.jpg']);
		expect(states(queue)).toEqual([
			['a.jpg', 'uploading', undefined],
			['b.jpg', 'uploading', undefined],
			['c.jpg', 'waiting', undefined]
		]);

		calls[0].resolve({ kind: 'created', photo: photo('p1') });
		await settle();

		expect(calls.map((call) => call.file.name)).toEqual(['a.jpg', 'b.jpg', 'c.jpg']);
		expect(getLimits).toHaveBeenCalledOnce();
	});

	it('sends one at a time when the limits cannot be read, and asks again later', async () => {
		const { queue, calls, getLimits } = setup();

		getLimits.mockRejectedValueOnce(new Error('offline'));
		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg'), jpeg('c.jpg')]);
		await settle();

		expect(calls).toHaveLength(1);

		calls[0].resolve({ kind: 'created', photo: photo('p1') });
		await settle();

		expect(getLimits).toHaveBeenCalledTimes(2);
		expect(calls).toHaveLength(3);
	});

	it('rejects non-JPEG and too-large files without sending them', async () => {
		const { queue, calls } = setup({ max_size: 10 });
		const folder = new File([], 'holiday', { type: '' });
		const png = new File([new Uint8Array(3)], 'a.png', { type: 'image/png' });
		const untyped = new File([new Uint8Array(3)], 'b.JPG', { type: '' });

		queue.add('g1', [folder, png, jpeg('big.jpg', 11), untyped]);
		await settle();

		expect(states(queue)).toEqual([
			['holiday', 'rejected', 'Not a JPEG'],
			['a.png', 'rejected', 'Not a JPEG'],
			['big.jpg', 'rejected', 'Too large'],
			['b.JPG', 'uploading', undefined]
		]);
		expect(calls.map((call) => call.file.name)).toEqual(['b.JPG']);
	});

	it('announces each created photo and forgets its file', async () => {
		const { queue, calls } = setup();
		const uploaded = vi.fn();

		queue.onUploaded(uploaded);
		queue.add('g1', [jpeg('a.jpg')]);
		await settle();
		calls[0].resolve({ kind: 'created', photo: photo('p1') });
		await settle();

		expect(uploaded).toHaveBeenCalledWith('g1', photo('p1'));
		expect(queue.items).toEqual([]);
	});

	it('maps rejections and failures to their tile state', async () => {
		const { queue, calls } = setup({ concurrency: 2 });

		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg')]);
		await settle();
		calls[0].resolve({ kind: 'rejected', reason: 'Unreadable image' });
		calls[1].resolve({ kind: 'failed' });
		await settle();

		expect(states(queue)).toEqual([
			['a.jpg', 'rejected', 'Unreadable image'],
			['b.jpg', 'failed', 'Upload failed']
		]);
	});

	it('waits out a busy server longer each time, up to a minute, then gives up', async () => {
		const { queue, calls } = setup({ concurrency: 1 });

		queue.add('g1', [jpeg('a.jpg')]);
		await settle();

		const waits = [2000, 4000, 8000, 16000, 32000, 60000, 60000];

		for (const [answer, wait] of waits.entries()) {
			calls[answer].resolve({ kind: 'busy', retryAfterMs: 2000 });
			await settle();

			expect(states(queue)).toEqual([['a.jpg', 'waiting', undefined]]);

			await vi.advanceTimersByTimeAsync(wait - 1);

			expect(calls).toHaveLength(answer + 1);

			await vi.advanceTimersByTimeAsync(1);

			expect(calls).toHaveLength(answer + 2);
		}

		expect(calls).toHaveLength(MAX_BUSY_RETRIES);

		calls.at(-1)!.resolve({ kind: 'busy', retryAfterMs: 2000 });
		await settle();

		expect(states(queue)).toEqual([['a.jpg', 'failed', 'Upload failed']]);
	});

	it('honours a Retry-After longer than a minute', async () => {
		const { queue, calls } = setup({ concurrency: 1 });

		queue.add('g1', [jpeg('a.jpg')]);
		await settle();
		calls[0].resolve({ kind: 'busy', retryAfterMs: 90_000 });
		await vi.advanceTimersByTimeAsync(89_999);

		expect(calls).toHaveLength(1);

		await vi.advanceTimersByTimeAsync(1);

		expect(calls).toHaveLength(2);
	});

	it('keeps a file waiting while the server stays busy with one of its own uploads', async () => {
		const { queue, calls } = setup({ concurrency: 2 });

		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg')]);
		await settle();

		for (let answer = 1; answer <= MAX_BUSY_RETRIES * 3; answer++) {
			calls.at(-1)!.resolve({ kind: 'busy', retryAfterMs: 1000 });
			await vi.advanceTimersByTimeAsync(1000);
		}

		expect(states(queue)).toEqual([
			['a.jpg', 'uploading', undefined],
			['b.jpg', 'uploading', undefined]
		]);

		calls[0].resolve({ kind: 'created', photo: photo('a') });
		await settle();
		calls.at(-1)!.resolve({ kind: 'busy', retryAfterMs: 1000 });
		await settle();

		expect(states(queue)).toEqual([['b.jpg', 'waiting', undefined]]);
	});

	it('marks the file and everything still waiting as storage full', async () => {
		const { queue, calls } = setup({ concurrency: 2 });

		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg'), jpeg('c.jpg')]);
		queue.add('g2', [jpeg('d.jpg')]);
		await settle();
		calls[0].resolve({ kind: 'storage-full' });
		await settle();

		expect(states(queue)).toEqual([
			['a.jpg', 'failed', 'Storage full'],
			['b.jpg', 'uploading', undefined],
			['c.jpg', 'failed', 'Storage full'],
			['d.jpg', 'failed', 'Storage full']
		]);
		expect(calls).toHaveLength(2);
	});

	it('drops only the files of a gallery that is gone', async () => {
		const { queue, calls } = setup({ concurrency: 2 });

		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg')]);
		queue.add('g2', [jpeg('c.jpg')]);
		await settle();
		calls[0].resolve({ kind: 'gone' });
		await settle();

		expect(calls[1].signal.aborted).toBe(true);
		expect(queue.forGallery('g1')).toEqual([]);
		expect(states(queue)).toEqual([['c.jpg', 'uploading', undefined]]);
	});

	it('stops everything when the session ends', async () => {
		const { queue, calls, onSignedOut } = setup({ concurrency: 1 });

		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg')]);
		await settle();
		calls[0].resolve({ kind: 'signed-out' });
		await settle();

		expect(queue.items).toEqual([]);
		expect(onSignedOut).toHaveBeenCalledOnce();
		expect(calls).toHaveLength(1);
	});

	it('cancels a waiting or uploading file at once', async () => {
		const { queue, calls } = setup({ concurrency: 1 });

		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg'), jpeg('c.jpg')]);
		await settle();
		const [uploading, waiting] = queue.items.map((item) => item.key);

		queue.cancel(waiting);
		queue.cancel(uploading);
		await settle();

		expect(calls[0].signal.aborted).toBe(true);
		expect(calls.map((call) => call.file.name)).toEqual(['a.jpg', 'c.jpg']);
		expect(states(queue)).toEqual([['c.jpg', 'uploading', undefined]]);
	});

	it('deletes a photo whose upload was cancelled too late', async () => {
		const { queue, calls, remove } = setup();
		const uploaded = vi.fn();

		queue.onUploaded(uploaded);
		queue.add('g1', [jpeg('a.jpg')]);
		await settle();
		queue.cancel(queue.items[0].key);
		calls[0].resolve({ kind: 'created', photo: photo('p1') });
		await settle();

		expect(remove).toHaveBeenCalledWith('g1', 'p1');
		expect(uploaded).not.toHaveBeenCalled();
	});

	it('retries a failed upload', async () => {
		const { queue, calls } = setup();

		queue.add('g1', [jpeg('a.jpg')]);
		await settle();
		calls[0].resolve({ kind: 'failed' });
		await settle();
		queue.retry(queue.items[0].key);
		await settle();

		expect(calls).toHaveLength(2);
		expect(states(queue)).toEqual([['a.jpg', 'uploading', undefined]]);
	});

	it('counts in-flight files per gallery and guards leaving only while pending', async () => {
		const { queue, calls } = setup({ concurrency: 1 });
		const leave = () => {
			const event = new Event('beforeunload', { cancelable: true }) as BeforeUnloadEvent;

			queue.guard(event);

			return event.defaultPrevented;
		};

		expect(leave()).toBe(false);

		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg')]);
		queue.add('g2', [new File([], 'c.png', { type: 'image/png' })]);
		await settle();

		expect(queue.inFlight('g1')).toBe(2);
		expect(queue.inFlight('g2')).toBe(0);
		expect(queue.hasPending).toBe(true);
		expect(leave()).toBe(true);

		calls[0].resolve({ kind: 'failed' });
		await settle();
		calls[1].resolve({ kind: 'created', photo: photo('p2') });
		await settle();

		expect(queue.hasPending).toBe(false);
		expect(leave()).toBe(false);
	});

	it('clears everything, aborting what is in flight', async () => {
		const { queue, calls } = setup();

		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg'), jpeg('c.jpg')]);
		await settle();
		queue.clear();
		await settle();

		expect(queue.items).toEqual([]);
		expect(calls.every((call) => call.signal.aborted)).toBe(true);
		expect(calls).toHaveLength(2);
	});
});
