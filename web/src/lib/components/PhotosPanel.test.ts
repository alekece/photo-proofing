import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
import { goto } from '$app/navigation';
import type { LiveEvents, PhotoEvent } from '#lib/events.js';
import type { Photo, PhotoStatus, UploadOutcome } from '#lib/gallery/photos.js';
import { UploadQueue } from '#lib/gallery/uploads.svelte.js';
import PhotosPanelHarness from './PhotosPanelHarness.test.svelte';

function photo(id: string, status: PhotoStatus = 'ready'): Photo {
	return {
		id,
		original_name: `${id}.jpg`,
		size: 3,
		width: 300,
		height: 200,
		status,
		created_at: '2026-11-04T10:00:00Z'
	};
}

function jpeg(name: string) {
	return new File([new Uint8Array(3)], name, { type: 'image/jpeg' });
}

function fakeEvents() {
	const photoHandlers = new Set<(event: PhotoEvent) => void>();
	const resyncHandlers = new Set<() => void>();
	const events: LiveEvents = {
		onPhoto: (handler) => (photoHandlers.add(handler), () => photoHandlers.delete(handler)),
		onResync: (handler) => (resyncHandlers.add(handler), () => resyncHandlers.delete(handler)),
		start() {},
		close() {}
	};

	return {
		events,
		emit: (event: Omit<PhotoEvent, 'gallery_id'> & { gallery_id?: string }) => {
			for (const handler of photoHandlers) handler({ gallery_id: 'g1', ...event });
		},
		resync: () => {
			for (const handler of resyncHandlers) handler();
		}
	};
}

/** Answers the photo routes like the API; `server` is what the list returns. */
function stubApi(server: Photo[], answers: { delete?: number; retry?: number } = {}) {
	const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
		if (init?.method === 'DELETE') return new Response(null, { status: answers.delete ?? 204 });

		if (init?.method === 'POST') {
			return new Response(JSON.stringify({ requeued: 1 }), { status: answers.retry ?? 200 });
		}

		return new Response(JSON.stringify(server));
	});

	vi.stubGlobal('fetch', fetcher);

	return {
		lists: () => fetcher.mock.calls.filter(([, init]) => init?.method === undefined).length,
		calls: (method: string) =>
			fetcher.mock.calls.filter(([, init]) => init?.method === method).map(([url]) => url)
	};
}

function setup(upload?: (file: File) => Promise<UploadOutcome>) {
	const pending: ((outcome: UploadOutcome) => void)[] = [];
	const queue = new UploadQueue({
		upload: (_, file) =>
			upload?.(file) ?? new Promise<UploadOutcome>((resolve) => pending.push(resolve)),
		limits: async () => ({ concurrency: 1, max_size: 100, max_pixels: 1000 }),
		remove: vi.fn().mockResolvedValue(undefined)
	});
	const live = fakeEvents();
	const view = render(PhotosPanelHarness, {
		props: { galleryId: 'g1', queue, events: live.events }
	});

	return { ...live, queue, pending, view };
}

const tile = (name: string) =>
	screen.getAllByRole('listitem').find((item) => item.textContent?.includes(name)) ??
	screen.getByRole('button', { name: `Open ${name}` }).closest('li')!;

beforeEach(() => vi.mocked(goto).mockClear());

afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe('PhotosPanel', () => {
	it('invites a first drop when the gallery is empty', async () => {
		stubApi([]);
		setup();

		expect(await screen.findByText('No photos yet. Drop JPEGs here to start.')).toBeTruthy();
	});

	it('shows a tile for each dropped or picked file at once', async () => {
		stubApi([]);
		const { queue } = setup();
		const user = userEvent.setup();

		await screen.findByText(/No photos yet/);
		await fireEvent.drop(screen.getByRole('region', { name: 'Photos' }), {
			dataTransfer: { files: [jpeg('a.jpg'), jpeg('b.jpg')] }
		});
		await user.upload(screen.getByLabelText('Choose photos to upload'), [jpeg('c.jpg')]);

		expect(queue.items.map((item) => item.file.name)).toEqual(['a.jpg', 'b.jpg', 'c.jpg']);
		expect(screen.getAllByRole('listitem')).toHaveLength(3);
		expect(screen.queryByText(/No photos yet/)).toBeNull();
	});

	it('shows each state with its label and actions', async () => {
		stubApi([
			photo('waiting', 'waiting'),
			photo('processing', 'processing'),
			photo('broken', { failed: 'The image could not be read.' }),
			photo('ready')
		]);
		const { queue } = setup(async (file) =>
			file.name === 'lost.jpg' ? { kind: 'failed' } : new Promise(() => {})
		);

		await screen.findByText('processing.jpg');
		queue.add('g1', [jpeg('lost.jpg'), jpeg('sending.jpg'), jpeg('next.jpg')]);
		queue.add('g1', [new File([], 'notes.txt', { type: 'text/plain' })]);

		await waitFor(() => expect(within(tile('sending.jpg')).getByText('Uploading')).toBeTruthy());

		for (const [name, label, retry] of [
			['waiting.jpg', 'Waiting to process', false],
			['processing.jpg', 'Processing', false],
			['broken.jpg', 'Processing failed', true],
			['lost.jpg', 'Upload failed', true],
			['sending.jpg', 'Uploading', false],
			['next.jpg', 'Waiting to upload', false],
			['notes.txt', 'Not a JPEG', false]
		] as const) {
			const item = within(tile(name));

			expect(item.getByText(label), name).toBeTruthy();
			expect(item.getByRole('button', { name: `Remove ${name}` })).toBeTruthy();
			expect(item.queryByRole('button', { name: `Retry ${name}` }) !== null, name).toBe(retry);
		}

		const ready = within(tile('ready.jpg'));

		expect(ready.getByRole('img').getAttribute('src')).toBe(
			'/api/v1/galleries/g1/photos/ready/thumbnail'
		);
		expect(ready.getByRole('img').getAttribute('loading')).toBe('lazy');
		expect(screen.getByRole('status').textContent).toMatch(/2 uploading, 2 processing, 3 failed/);
	});

	it('follows live events: status changes and deletions', async () => {
		stubApi([photo('p1', 'waiting'), photo('p2', 'processing')]);
		const { emit } = setup();

		await screen.findByText('p1.jpg');
		emit({ photo_id: 'p1', status: 'processing' });
		emit({ photo_id: 'p2', status: 'ready' });
		emit({ photo_id: 'p1', status: 'failed', gallery_id: 'other' });

		await waitFor(() => expect(within(tile('p1.jpg')).getByText('Processing')).toBeTruthy());
		expect(screen.getByRole('button', { name: 'Open p2.jpg' })).toBeTruthy();

		emit({ photo_id: 'p1', status: 'failed' });

		await waitFor(() => expect(within(tile('p1.jpg')).getByText('Processing failed')).toBeTruthy());

		emit({ photo_id: 'p1', status: 'deleted' });

		await waitFor(() => expect(screen.queryByText('p1.jpg')).toBeNull());
	});

	it('brings in an uploaded photo through its answer, not a refetch', async () => {
		const api = stubApi([]);
		const { emit, pending } = setup();

		await screen.findByText(/No photos yet/);
		await fireEvent.drop(screen.getByRole('region', { name: 'Photos' }), {
			dataTransfer: { files: [jpeg('new.jpg'), jpeg('later.jpg')] }
		});
		await waitFor(() => expect(pending).toHaveLength(1));

		emit({ photo_id: 'new', status: 'processing' });
		pending[0]({ kind: 'created', photo: photo('new', 'waiting') });

		await waitFor(() => expect(within(tile('new.jpg')).getByText('Processing')).toBeTruthy());
		expect(api.lists()).toBe(1);

		await waitFor(() => expect(pending).toHaveLength(2));
		pending[1]({ kind: 'created', photo: photo('later', 'waiting') });

		// The gallery's uploads are done: one refetch catches a photo whose answer was lost.
		await waitFor(() => expect(api.lists()).toBe(2));
	});

	it('refetches on an unknown photo or a resync, at most once a second', async () => {
		vi.useFakeTimers();
		const api = stubApi([]);
		const { emit, resync } = setup();

		await vi.advanceTimersByTimeAsync(0);

		expect(api.lists()).toBe(1);

		emit({ photo_id: 'elsewhere', status: 'ready' });
		await vi.advanceTimersByTimeAsync(0);

		expect(api.lists()).toBe(1);

		await vi.advanceTimersByTimeAsync(1000);
		resync();
		resync();
		emit({ photo_id: 'elsewhere', status: 'ready' });
		await vi.advanceTimersByTimeAsync(0);

		expect(api.lists()).toBe(2);

		await vi.advanceTimersByTimeAsync(1000);

		expect(api.lists()).toBe(3);
	});

	it('keeps changes that arrive while a refetch is pending', async () => {
		vi.useFakeTimers();
		let answerLate: (photos: Photo[]) => void = () => {};
		const lists = [
			Promise.resolve([photo('p1', 'processing'), photo('p2'), photo('p3')]),
			new Promise<Photo[]>((resolve) => (answerLate = resolve))
		];

		vi.stubGlobal(
			'fetch',
			vi.fn(async (_url: string, init?: RequestInit) =>
				init?.method === 'DELETE'
					? new Response(null, { status: 204 })
					: new Response(JSON.stringify(await lists.shift()))
			)
		);

		const { emit, resync } = setup();

		await vi.advanceTimersByTimeAsync(0);
		resync();
		await vi.advanceTimersByTimeAsync(1000);

		emit({ photo_id: 'p1', status: 'ready' });
		emit({ photo_id: 'p3', status: 'deleted' });
		await fireEvent.click(screen.getByRole('button', { name: 'Remove p2.jpg' }));
		await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
		await vi.advanceTimersByTimeAsync(0);

		answerLate([photo('p1', 'processing'), photo('p2'), photo('p3')]);
		await vi.advanceTimersByTimeAsync(0);

		expect(screen.getAllByRole('button', { name: /^Open/ }).map((open) => open.ariaLabel)).toEqual([
			'Open p1.jpg'
		]);
	});

	it('removes a file still in the browser without asking', async () => {
		stubApi([]);
		const { queue } = setup();
		const user = userEvent.setup();

		await screen.findByText(/No photos yet/);
		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg')]);
		await user.click(await screen.findByRole('button', { name: 'Remove b.jpg' }));

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(queue.items.map((item) => item.file.name)).toEqual(['a.jpg']);
	});

	it('asks before deleting a server photo, then deletes it', async () => {
		const api = stubApi([photo('p1'), photo('p2')]);
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Remove p1.jpg' }));

		const dialog = screen.getByRole('dialog', { name: 'Delete p1.jpg?' });

		await user.click(within(dialog).getByRole('button', { name: 'Cancel' }));

		expect(api.calls('DELETE')).toEqual([]);

		await user.click(screen.getByRole('button', { name: 'Remove p1.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));

		expect(screen.queryByRole('button', { name: 'Open p1.jpg' })).toBeNull();
		expect(api.calls('DELETE')).toEqual(['/api/v1/galleries/g1/photos/p1']);
	});

	it('puts a photo back when its deletion fails', async () => {
		stubApi([photo('p1'), photo('p2')], { delete: 500 });
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Remove p2.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));

		expect((await screen.findByRole('alert')).textContent).toMatch(/Couldn’t delete p2\.jpg/);
		expect(screen.getAllByRole('button', { name: /^Open/ }).map((open) => open.ariaLabel)).toEqual([
			'Open p1.jpg',
			'Open p2.jpg'
		]);
	});

	it('goes to sign-in when a deletion finds the session ended', async () => {
		stubApi([photo('p1')], { delete: 401 });
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Remove p1.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));

		await waitFor(() =>
			expect(goto).toHaveBeenCalledWith('/login?expired=1', { refreshAll: true })
		);
	});

	it('retries a photo whose processing failed', async () => {
		const api = stubApi([photo('p1', { failed: 'The image could not be read.' })]);
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Retry p1.jpg' }));

		expect(api.calls('POST')).toEqual(['/api/v1/galleries/g1/photos/p1/retry']);
		expect(within(tile('p1.jpg')).getByText('Waiting to process')).toBeTruthy();
	});

	it('opens a ready photo in the viewer', async () => {
		stubApi([photo('p1'), photo('p2', 'processing'), photo('p3')]);
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Open p3.jpg' }));

		const viewer = screen.getByRole('dialog', { name: /proof viewer/i });

		expect(within(viewer).getByText('2 of 2')).toBeTruthy();
		expect(within(viewer).getByRole('img').getAttribute('src')).toBe(
			'/api/v1/galleries/g1/photos/p3/preview'
		);
	});

	it('has no detectable accessibility violations', async () => {
		stubApi([photo('p1'), photo('p2', { failed: 'x' })]);
		const { queue, view } = setup();

		await screen.findByText('p2.jpg');
		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg')]);
		await screen.findByText('Waiting to upload');

		expect((await axe(view.container)).violations).toEqual([]);
	});
});
