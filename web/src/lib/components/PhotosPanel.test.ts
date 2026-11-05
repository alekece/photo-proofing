import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
// Small chunks keep the bulk-delete tests readable.
vi.mock('#lib/gallery/photos.js', async (importOriginal) => ({
	...(await importOriginal<typeof import('#lib/gallery/photos.js')>()),
	BULK_DELETE_LIMIT: 2
}));
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

/**
 * Answers the photo routes like the API; `server` is what the list returns, per gallery id when
 * it is a record.
 */
function stubApi(
	server: Photo[] | Record<string, Photo[]>,
	answers: {
		delete?: number;
		retry?: number;
		bulk?: (ids: string[]) => Response | Promise<Response>;
		list?: () => Response | undefined;
	} = {}
) {
	const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
		if (init?.method === 'DELETE') return new Response(null, { status: answers.delete ?? 204 });

		if (init?.method === 'POST' && url.endsWith('/photos/delete')) {
			const ids: string[] = JSON.parse(String(init.body)).photo_ids;

			return answers.bulk?.(ids) ?? Response.json({ deleted: ids.length });
		}

		if (init?.method === 'POST') {
			return new Response(JSON.stringify({ requeued: 1 }), { status: answers.retry ?? 200 });
		}

		const listed = Array.isArray(server) ? server : server[url.split('/')[4]];

		return answers.list?.() ?? new Response(JSON.stringify(listed));
	});

	vi.stubGlobal('fetch', fetcher);

	return {
		lists: () => fetcher.mock.calls.filter(([, init]) => init?.method === undefined).length,
		calls: (method: string) =>
			fetcher.mock.calls.filter(([, init]) => init?.method === method).map(([url]) => url),
		bulk: () =>
			fetcher.mock.calls
				.filter(([url, init]) => init?.method === 'POST' && url.endsWith('/photos/delete'))
				.map(([, init]) => JSON.parse(String(init?.body)).photo_ids as string[]),
		bulkUrls: () =>
			fetcher.mock.calls
				.filter(([url, init]) => init?.method === 'POST' && url.endsWith('/photos/delete'))
				.map(([url]) => url)
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

	describe('select mode', () => {
		const checkbox = (name: string) =>
			screen.queryByRole<HTMLInputElement>('checkbox', { name: `Select ${name}` });
		const counter = () =>
			screen.getByText(/\d+ selected/, { selector: ':not(.sr-only)' }).textContent;
		const announced = () => screen.getByRole('status').textContent;
		const selectButton = () => screen.getByRole('button', { name: 'Select' });

		async function confirmDeletion(user: ReturnType<typeof userEvent.setup>) {
			await user.click(screen.getByRole('button', { name: 'Delete' }));
			await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Delete' }));
		}

		async function enter(user: ReturnType<typeof userEvent.setup>) {
			await user.click(await screen.findByRole('button', { name: 'Select' }));
		}

		it('enters and leaves select mode', async () => {
			stubApi([photo('p1'), photo('p2', { failed: 'x' })]);
			const { queue } = setup();
			const user = userEvent.setup();

			await screen.findByText('p2.jpg');
			queue.add('g1', [jpeg('a.jpg')]);
			await enter(user);

			expect(checkbox('p1.jpg')).not.toBeNull();
			expect(checkbox('p2.jpg')).not.toBeNull();
			expect(checkbox('a.jpg')).toBeNull();
			expect(screen.queryByRole('button', { name: 'Remove p1.jpg' })).toBeNull();
			expect(screen.queryByRole('button', { name: 'Retry p2.jpg' })).toBeNull();
			expect(screen.getByRole('button', { name: 'Remove a.jpg' })).toBeTruthy();

			await user.click(within(checkbox('p1.jpg')!.closest('label')!).getByRole('img'));

			expect(screen.queryByRole('dialog')).toBeNull();
			expect(counter()).toBe('1 selected');

			expect(announced()).toContain('1 selected');

			await user.click(screen.getByRole('button', { name: 'Cancel' }));

			expect(checkbox('p1.jpg')).toBeNull();
			await waitFor(() => expect(document.activeElement).toBe(selectButton()));

			await enter(user);

			expect(counter()).toBe('0 selected');

			await user.click(checkbox('p2.jpg')!);
			await user.keyboard('{Escape}');

			expect(checkbox('p2.jpg')).toBeNull();
			expect(screen.getByRole('button', { name: 'Remove p1.jpg' })).toBeTruthy();
			await waitFor(() => expect(document.activeElement).toBe(selectButton()));
		});

		it('select all takes only the photos shown now', async () => {
			const server = [photo('p1'), photo('p2', 'processing')];
			stubApi(server);
			const { queue, pending } = setup();
			const user = userEvent.setup();

			await enter(user);
			await user.click(screen.getByRole('button', { name: 'Select all' }));

			expect(counter()).toBe('2 selected');

			queue.add('g1', [jpeg('p3.jpg')]);
			await waitFor(() => expect(pending).toHaveLength(1));
			server.push(photo('p3', 'waiting'));
			pending[0]({ kind: 'created', photo: photo('p3', 'waiting') });

			expect(
				(await screen.findByRole<HTMLInputElement>('checkbox', { name: 'Select p3.jpg' })).checked
			).toBe(false);
			expect(counter()).toBe('2 selected');

			await user.click(checkbox('p1.jpg')!);
			await user.keyboard('{Control>}a{/Control}');

			expect(counter()).toBe('3 selected');

			await user.click(checkbox('p1.jpg')!);
			await user.keyboard('{Meta>}a{/Meta}');

			expect(counter()).toBe('3 selected');
		});

		it('confirms once, then deletes the selection', async () => {
			const api = stubApi([photo('p1'), photo('p2'), photo('p3')]);
			const user = userEvent.setup();

			setup();
			await enter(user);

			expect((screen.getByRole('button', { name: 'Delete' }) as HTMLButtonElement).disabled).toBe(
				true
			);

			await user.click(checkbox('p1.jpg')!);
			await user.click(checkbox('p3.jpg')!);
			await user.click(screen.getByRole('button', { name: 'Delete' }));

			const dialog = screen.getByRole('dialog', { name: 'Delete 2 photos?' });

			expect(within(dialog).getByText(/can’t be undone/)).toBeTruthy();

			await user.click(within(dialog).getByRole('button', { name: 'Delete' }));

			expect(screen.queryByText('p1.jpg')).toBeNull();
			expect(screen.queryByText('p3.jpg')).toBeNull();
			await waitFor(() => expect(checkbox('p2.jpg')).toBeNull());
			expect(api.bulk()).toEqual([['p1', 'p3']]);
			expect(screen.getByRole('button', { name: 'Open p2.jpg' })).toBeTruthy();
			await waitFor(() => expect(document.activeElement).toBe(selectButton()));
			expect(announced()).toContain('2 photos deleted');
		});

		it('finishes a confirmed deletion in its own gallery after a gallery change', async () => {
			let answer: (response: Response) => void = () => {};
			const api = stubApi(
				{ g1: [photo('p1'), photo('p2'), photo('p3')], g2: [photo('q1', 'waiting')] },
				{
					bulk: (ids) =>
						ids.length === 2
							? new Promise((resolve) => (answer = resolve))
							: Response.json({ deleted: ids.length })
				}
			);
			const { view } = setup();
			const user = userEvent.setup();

			await enter(user);
			await user.click(screen.getByRole('button', { name: 'Select all' }));
			await confirmDeletion(user);
			await waitFor(() => expect(api.bulk()).toHaveLength(1));
			await view.rerender({ galleryId: 'g2' });
			await screen.findByText('q1.jpg');
			answer(Response.json({ deleted: 2 }));

			await waitFor(() => expect(api.bulk()).toEqual([['p1', 'p2'], ['p3']]));
			expect(api.bulkUrls()).toEqual([
				'/api/v1/galleries/g1/photos/delete',
				'/api/v1/galleries/g1/photos/delete'
			]);
			expect(api.lists()).toBe(2);
			expect(screen.queryByRole('alert')).toBeNull();
			expect(screen.getByText('q1.jpg')).toBeTruthy();
			expect(announced()).not.toContain('deleted');
		});

		it('finishes a confirmed deletion after the panel is gone, without touching it', async () => {
			let answer: (response: Response) => void = () => {};
			const api = stubApi([photo('p1'), photo('p2'), photo('p3')], {
				bulk: (ids) =>
					ids.length === 2
						? new Promise((resolve) => (answer = resolve))
						: Promise.reject(new TypeError('network'))
			});
			const { view } = setup();
			const user = userEvent.setup();

			await enter(user);
			await user.click(screen.getByRole('button', { name: 'Select all' }));
			await confirmDeletion(user);
			await waitFor(() => expect(api.bulk()).toHaveLength(1));
			view.unmount();
			answer(Response.json({ deleted: 2 }));

			await waitFor(() => expect(api.bulk()).toEqual([['p1', 'p2'], ['p3']]));
			await new Promise((resolve) => setTimeout(resolve, 0));
			expect(api.lists()).toBe(1);
		});

		it('restores the photos left when both the deletion and the refetch fail', async () => {
			let answer: (response: Response) => void = () => {};
			let lists = 0;
			stubApi([photo('p1'), photo('p2'), photo('p3'), photo('p4')], {
				bulk: () => new Promise((resolve) => (answer = resolve)),
				list: () => (++lists > 1 ? new Response(null, { status: 500 }) : undefined)
			});
			const { emit } = setup();
			const user = userEvent.setup();

			await enter(user);
			await user.click(checkbox('p1.jpg')!);
			await user.click(checkbox('p2.jpg')!);
			await user.click(checkbox('p3.jpg')!);
			await confirmDeletion(user);
			emit({ photo_id: 'p2', status: 'deleted' });
			answer(new Response(null, { status: 500 }));

			await waitFor(() => expect(lists).toBe(2));
			await waitFor(() => expect(checkbox('p3.jpg')).not.toBeNull());
			expect(screen.getAllByRole('checkbox').map((box) => box.getAttribute('aria-label'))).toEqual([
				'Select p1.jpg',
				'Select p3.jpg',
				'Select p4.jpg'
			]);
			expect(screen.queryByText(/No photos yet/)).toBeNull();
			expect(checkbox('p1.jpg')!.checked).toBe(true);
		});

		it('sends a large selection in chunks, one after another', async () => {
			let answer: (response: Response) => void = () => {};
			const api = stubApi([photo('p1'), photo('p2'), photo('p3')], {
				bulk: (ids) =>
					ids.length === 2
						? new Promise((resolve) => (answer = resolve))
						: Response.json({ deleted: ids.length })
			});
			const user = userEvent.setup();

			setup();
			await enter(user);
			await user.keyboard('{Control>}a{/Control}');
			await user.click(screen.getByRole('button', { name: 'Delete' }));
			await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Delete' }));

			await waitFor(() => expect(api.bulk()).toEqual([['p1', 'p2']]));

			answer(Response.json({ deleted: 2 }));

			await waitFor(() => expect(api.bulk()).toEqual([['p1', 'p2'], ['p3']]));
			await waitFor(() => expect(screen.queryByText(/selected/)).toBeNull());
		});

		it('stops and refetches when a chunk fails', async () => {
			const server = ['p1', 'p2', 'p3', 'p4', 'p5'].map((id) => photo(id));
			const api = stubApi(server, {
				bulk: (ids) => {
					if (ids[0] === 'p3') return Promise.reject(new TypeError('network'));

					server.splice(0, ids.length);

					return Response.json({ deleted: ids.length });
				}
			});
			const user = userEvent.setup();

			setup();
			await enter(user);
			await user.click(screen.getByRole('button', { name: 'Select all' }));
			await user.click(screen.getByRole('button', { name: 'Delete' }));
			await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Delete' }));

			expect((await screen.findByRole('alert')).textContent).toBe(
				'Deleted 2 of 5 photos. Please try again.'
			);
			expect(api.bulk()).toEqual([
				['p1', 'p2'],
				['p3', 'p4']
			]);
			await waitFor(() => expect(counter()).toBe('3 selected'));
			expect(api.lists()).toBe(2);
			expect(screen.queryByText('p1.jpg')).toBeNull();
			expect(checkbox('p5.jpg')!.checked).toBe(true);
		});

		it('goes to sign-in when a bulk deletion finds the session ended', async () => {
			stubApi([photo('p1')], { bulk: () => new Response(null, { status: 401 }) });
			const user = userEvent.setup();

			setup();
			await enter(user);
			await user.click(checkbox('p1.jpg')!);
			await user.click(screen.getByRole('button', { name: 'Delete' }));
			await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Delete' }));

			await waitFor(() =>
				expect(goto).toHaveBeenCalledWith('/login?expired=1', { refreshAll: true })
			);
		});

		it('drops a photo deleted elsewhere from the selection', async () => {
			stubApi([photo('p1'), photo('p2')]);
			const { emit } = setup();
			const user = userEvent.setup();

			await enter(user);
			await user.click(screen.getByRole('button', { name: 'Select all' }));
			emit({ photo_id: 'p2', status: 'deleted' });

			await waitFor(() => expect(counter()).toBe('1 selected'));

			await user.click(screen.getByRole('button', { name: 'Delete' }));

			expect(screen.getByRole('dialog', { name: 'Delete 1 photo?' })).toBeTruthy();
		});

		it('leaves keys alone while typing in a text field', async () => {
			stubApi([photo('p1'), photo('p2')]);
			const user = userEvent.setup();
			const field = document.body.appendChild(document.createElement('input'));

			setup();
			await enter(user);
			await user.click(checkbox('p1.jpg')!);
			await user.click(field);
			await user.keyboard('{Control>}a{/Control}');
			await user.keyboard('{Escape}');

			expect(counter()).toBe('1 selected');

			field.remove();
		});

		it('closes only the dialog on Escape', async () => {
			stubApi([photo('p1')]);
			const user = userEvent.setup();

			setup();
			await enter(user);
			await user.click(checkbox('p1.jpg')!);
			await user.click(screen.getByRole('button', { name: 'Delete' }));
			// jsdom lacks showModal, which moves focus into the dialog.
			within(screen.getByRole('dialog')).getByRole('button', { name: 'Cancel' }).focus();
			await user.keyboard('{Escape}');

			expect(screen.queryByRole('dialog')).toBeNull();
			expect(counter()).toBe('1 selected');
		});

		it('has no detectable accessibility violations', async () => {
			stubApi([photo('p1'), photo('p2', 'processing')]);
			const { view } = setup();
			const user = userEvent.setup();

			await enter(user);
			await user.click(checkbox('p1.jpg')!);

			expect((await axe(view.container)).violations).toEqual([]);
		});
	});
});
