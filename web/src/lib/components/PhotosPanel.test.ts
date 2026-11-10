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
import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
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
		clean_ready: status === 'ready',
		favorited_at: null,
		created_at: '2026-11-04T10:00:00Z'
	};
}

function regenerating(id: string): Photo {
	return { ...photo(id, 'processing'), clean_ready: true };
}

function jpeg(name: string) {
	return new File([new Uint8Array(3)], name, { type: 'image/jpeg' });
}

function fakeEvents() {
	const photoHandlers = new Set<(event: PhotoEvent) => void>();
	const resyncHandlers = new Set<() => void>();
	const events: LiveEvents = {
		onPhoto: (handler) => (photoHandlers.add(handler), () => photoHandlers.delete(handler)),
		onFavorites: () => () => {},
		onExport: () => () => {},
		onNotification: () => () => {},
		onResync: (handler) => (resyncHandlers.add(handler), () => resyncHandlers.delete(handler)),
		start() {},
		close() {}
	};

	return {
		events,
		emit: (event: Omit<PhotoEvent, 'gallery_id' | 'kind'> & Partial<PhotoEvent>) => {
			for (const handler of photoHandlers) handler({ gallery_id: 'g1', kind: 'proof', ...event });
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
		delete?: number | Promise<number>;
		retry?: number;
		bulk?: (ids: string[]) => Response | Promise<Response>;
		list?: () => Response | void | Promise<Response | void>;
		hero?: (heroId: string | null) => Response | Promise<Response>;
	} = {}
) {
	const listOf = (url: string) => (Array.isArray(server) ? server : server[url.split('/')[4]]);
	const drop = (url: string, ids: string[]) => {
		const list = listOf(url);

		for (const id of ids) {
			const at = list.findIndex((listed) => listed.id === id);

			if (at !== -1) list.splice(at, 1);
		}
	};
	const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
		if (init?.method === 'PATCH') {
			const { hero_id } = JSON.parse(String(init.body));

			return answers.hero?.(hero_id) ?? Response.json({ ...gallery, hero_id });
		}

		if (init?.method === 'DELETE') {
			const status = (await answers.delete) ?? 204;

			if (status < 400) drop(url, [url.split('?')[0].split('/')[6]]);

			return new Response(null, { status });
		}

		if (init?.method === 'POST' && url.endsWith('/photos/delete')) {
			const ids: string[] = JSON.parse(String(init.body)).photo_ids;

			if (answers.bulk) return answers.bulk(ids);

			drop(url, ids);

			return Response.json({ deleted: ids.length });
		}

		if (init?.method === 'POST') {
			return new Response(JSON.stringify({ requeued: 1 }), { status: answers.retry ?? 200 });
		}

		return (await answers.list?.()) ?? new Response(JSON.stringify(listOf(url)));
	});

	vi.stubGlobal('fetch', fetcher);

	return {
		lists: () => fetcher.mock.calls.filter(([, init]) => init?.method === undefined).length,
		heroes: () =>
			fetcher.mock.calls
				.filter(([, init]) => init?.method === 'PATCH')
				.map(([, init]) => JSON.parse(String(init?.body)).hero_id as string | null),
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

const gallery = {
	id: 'g1',
	title: null,
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	last_opened_at: null,
	shoot_date: null,
	states: ['draft'],
	created_at: '2026-11-04T10:00:00Z',
	updated_at: '2026-11-04T10:00:00Z'
};

function setup(
	upload?: (file: File) => Promise<UploadOutcome>,
	heroId: string | null = null,
	watermarked = false,
	set: 'photos' | 'finals' = 'photos'
) {
	const pending: ((outcome: UploadOutcome) => void)[] = [];
	const queue = new UploadQueue({
		upload: (_, file) =>
			upload?.(file) ?? new Promise<UploadOutcome>((resolve) => pending.push(resolve)),
		limits: async () => ({ concurrency: 1, max_size: 100, max_pixels: 1000 }),
		remove: vi.fn().mockResolvedValue(undefined)
	});
	const live = fakeEvents();
	const view = render(PhotosPanelHarness, {
		props: { galleryId: 'g1', queue, events: live.events, heroId, watermarked, set }
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
	toasts.items = [];
});

describe('PhotosPanel', () => {
	it('keeps deletion confirmation busy until the real photo request finishes', async () => {
		let finish!: (status: number) => void;
		const pending = new Promise<number>((resolve) => (finish = resolve));
		stubApi([photo('p1')], { delete: pending });
		setup();
		const user = userEvent.setup();
		await user.click(await screen.findByRole('button', { name: 'Remove p1.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));

		expect(screen.getByRole('button', { name: 'Deleting…' })).toHaveProperty('disabled', true);
		finish(204);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
	});

	it('toasts once per batch of uploads that fail', async () => {
		stubApi([photo('p1')]);
		toasts.items = [];
		const { queue } = setup(async () => ({ kind: 'failed' }));

		await screen.findByRole('button', { name: 'Open p1.jpg' });
		queue.add('g1', [jpeg('a.jpg')]);

		await waitFor(() =>
			expect(toasts.items.map(({ message }) => message)).toEqual(['1 photo failed to upload.'])
		);
		expect(within(tile('a.jpg')).getByText('Upload failed')).toBeTruthy();
		expect(within(tile('a.jpg')).getByRole('button', { name: 'Retry a.jpg' })).toBeTruthy();

		toasts.items = [];
	});

	it('says Updating watermarks… while a clean photo stays available during regeneration', async () => {
		const server = [photo('p1')];
		stubApi(server);
		const { emit } = setup(undefined, null, true);

		await screen.findByRole('button', { name: 'Open p1.jpg' });

		expect(screen.queryByText('Updating watermarks…')).toBeNull();

		server[0] = regenerating('p1');
		emit({ photo_id: 'p1', status: 'processing' });

		expect(await screen.findByText('Updating watermarks…')).toBeTruthy();

		server[0] = photo('p1');
		emit({ photo_id: 'p1', status: 'ready' });

		await waitFor(() => expect(screen.queryByText('Updating watermarks…')).toBeNull());
	});

	it('reports regeneration even with watermarking off, but not for a new upload', async () => {
		const server = [photo('p1'), photo('p2', 'processing')];
		stubApi(server);
		const { emit } = setup();

		await screen.findByRole('button', { name: 'Open p1.jpg' });
		expect(screen.queryByText('Updating watermarks…')).toBeNull();
		server[0] = regenerating('p1');
		emit({ photo_id: 'p1', status: 'processing' });

		await waitFor(() => expect(within(tile('p1.jpg')).getByText('Processing')).toBeTruthy());
		expect(screen.getByText('Updating watermarks…')).toBeTruthy();
		expect(within(tile('p1.jpg')).getByRole('img').getAttribute('src')).toBe(
			'/api/v1/galleries/g1/photos/p1/thumbnail'
		);
	});

	it('invites a first drop when the gallery is empty', async () => {
		stubApi([]);
		setup();

		const zone = (await screen.findByText('Drop the shoot here')).parentElement!;

		expect(within(zone).queryByRole('button')).toBeNull();
		expect(screen.getAllByRole('button', { name: 'Upload' })).toHaveLength(1);
	});

	it('shows a tile for each dropped or picked file at once', async () => {
		stubApi([]);
		const { queue } = setup();
		const user = userEvent.setup();

		await screen.findByText(/Drop the shoot here/);
		await fireEvent.drop(screen.getByRole('region', { name: 'Photos' }), {
			dataTransfer: { files: [jpeg('a.jpg'), jpeg('b.jpg')] }
		});
		await user.upload(screen.getByLabelText('Choose photos to upload'), [jpeg('c.jpg')]);

		expect(queue.items.map((item) => item.file.name)).toEqual(['a.jpg', 'b.jpg', 'c.jpg']);
		expect(screen.getAllByRole('listitem')).toHaveLength(3);
		expect(screen.queryByText(/Drop the shoot here/)).toBeNull();
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
			['waiting.jpg', 'Waiting', false],
			['processing.jpg', 'Processing', false],
			['broken.jpg', 'Processing failed', true],
			['lost.jpg', 'Upload failed', true],
			['sending.jpg', 'Uploading', false],
			['next.jpg', 'Waiting', false],
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
		const server = [photo('p1', 'waiting'), photo('p2', 'processing')];
		const api = stubApi(server);
		const { emit } = setup();

		await screen.findByText('p1.jpg');
		server[0] = photo('p1', 'processing');
		server[1] = photo('p2', 'ready');
		emit({ photo_id: 'p1', status: 'processing' });
		emit({ photo_id: 'p1', status: 'failed', gallery_id: 'other' });

		await waitFor(() => expect(within(tile('p1.jpg')).getByText('Processing')).toBeTruthy());
		expect(screen.getByRole('button', { name: 'Open p2.jpg' })).toBeTruthy();

		server[0] = photo('p1', { failed: 'x' });
		emit({ photo_id: 'p1', status: 'failed' });

		await waitFor(() => expect(within(tile('p1.jpg')).getByText('Processing failed')).toBeTruthy());

		server.shift();
		emit({ photo_id: 'p1', status: 'deleted' });

		await waitFor(() => expect(screen.queryByText('p1.jpg')).toBeNull());
		expect(api.lists()).toBe(4);
	});

	it('brings in an uploaded photo by refetching the list', async () => {
		const server: Photo[] = [];
		const api = stubApi(server);
		const { pending } = setup();

		await screen.findByText(/Drop the shoot here/);
		await fireEvent.drop(screen.getByRole('region', { name: 'Photos' }), {
			dataTransfer: { files: [jpeg('new.jpg')] }
		});
		await waitFor(() => expect(pending).toHaveLength(1));

		server.push(photo('new', 'waiting'));
		pending[0]({ kind: 'created', photo: photo('new', 'waiting') });

		expect(await screen.findByText('new.jpg')).toBeTruthy();
		await waitFor(() => expect(api.lists()).toBe(2));
	});

	it('runs one fetch at a time, with one follow-up for everything that changed meanwhile', async () => {
		const gates: PromiseWithResolvers<void>[] = [];
		const server = [photo('p1', 'processing')];
		const api = stubApi(server, {
			list: () => {
				const gate = Promise.withResolvers<void>();

				gates.push(gate);

				return gate.promise;
			}
		});
		const { emit, resync } = setup();

		await waitFor(() => expect(gates).toHaveLength(1));
		emit({ photo_id: 'p1', status: 'ready' });
		emit({ photo_id: 'p2', status: 'waiting' });
		resync();
		resync();

		expect(api.lists()).toBe(1);

		server[0] = photo('p1');
		gates[0].resolve();
		await waitFor(() => expect(gates).toHaveLength(2));

		expect(api.lists()).toBe(2);

		server.push(photo('p2', 'waiting'));
		gates[1].resolve();

		expect(await screen.findByText('p2.jpg')).toBeTruthy();
		expect(api.lists()).toBe(2);

		resync();

		await waitFor(() => expect(gates).toHaveLength(3));
		gates[2].resolve();
	});

	it('shows what the server lists when uploads, deletes, events and resyncs overlap', async () => {
		const server = [photo('p1'), photo('p2', 'processing'), photo('p3')];
		const gates: PromiseWithResolvers<void>[] = [];
		const user = userEvent.setup();
		const api = stubApi(server, {
			list: () => {
				const gate = Promise.withResolvers<void>();

				gates.push(gate);

				return gate.promise;
			}
		});
		const { emit, resync, pending } = setup();

		await waitFor(() => expect(gates).toHaveLength(1));
		gates[0].resolve();
		await screen.findByRole('button', { name: 'Open p3.jpg' });

		// A list is answered late while a photo is uploaded, another deleted and another changes.
		resync();
		await waitFor(() => expect(gates).toHaveLength(2));
		await fireEvent.drop(screen.getByRole('region', { name: 'Photos' }), {
			dataTransfer: { files: [jpeg('new.jpg')] }
		});
		await waitFor(() => expect(pending).toHaveLength(1));
		await user.click(screen.getByRole('button', { name: 'Remove p3.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));

		server.push(photo('new', 'waiting'));
		server[1] = photo('p2');
		pending[0]({ kind: 'created', photo: photo('new', 'waiting') });
		emit({ photo_id: 'p2', status: 'ready' });
		gates[1].resolve();
		await waitFor(() => expect(gates).toHaveLength(3));
		gates[2].resolve();

		await waitFor(() =>
			expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual(
				expect.arrayContaining([expect.stringContaining('new.jpg')])
			)
		);
		expect(screen.queryByRole('button', { name: 'Open p3.jpg' })).toBeNull();
		expect(within(tile('p2.jpg')).getByRole('button', { name: 'Open p2.jpg' })).toBeTruthy();
		expect(api.calls('DELETE')).toHaveLength(1);
	});

	it('ignores a late answer from the gallery it no longer shows', async () => {
		const late = Promise.withResolvers<void>();
		let first = true;
		const api = stubApi(
			{ g1: [photo('p1')], g2: [photo('q1')] },
			{
				list: () => {
					if (!first) return;

					first = false;

					return late.promise;
				}
			}
		);
		const { view } = setup();

		await waitFor(() => expect(api.lists()).toBe(1));
		await view.rerender({ galleryId: 'g2' });

		expect(await screen.findByRole('button', { name: 'Open q1.jpg' })).toBeTruthy();

		late.resolve();
		await new Promise((resolve) => setTimeout(resolve, 10));

		expect(screen.queryByRole('button', { name: 'Open p1.jpg' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Open q1.jpg' })).toBeTruthy();
	});

	it('does not refetch for events about other galleries', async () => {
		const api = stubApi([photo('p1')]);
		const { emit } = setup();

		await screen.findByRole('button', { name: 'Open p1.jpg' });
		emit({ photo_id: 'p1', status: 'ready', gallery_id: 'other' });
		await new Promise((resolve) => setTimeout(resolve, 10));

		expect(api.lists()).toBe(1);
	});

	it('removes a file still in the browser without asking', async () => {
		stubApi([]);
		const { queue } = setup();
		const user = userEvent.setup();

		await screen.findByText(/Drop the shoot here/);
		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg')]);
		await user.click(await screen.findByRole('button', { name: 'Remove b.jpg' }));

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(queue.items.map((item) => item.file.name)).toEqual(['a.jpg']);
	});

	it('moves focus to the next tile, else the previous one, else Upload, when a file is removed', async () => {
		stubApi([]);
		const { queue } = setup();
		const user = userEvent.setup();

		await screen.findByText(/Drop the shoot here/);
		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg'), jpeg('c.jpg')]);
		await user.click(await screen.findByRole('button', { name: 'Remove b.jpg' }));

		await waitFor(() =>
			expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Remove c.jpg' }))
		);

		await user.click(screen.getByRole('button', { name: 'Remove c.jpg' }));

		await waitFor(() =>
			expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Remove a.jpg' }))
		);

		await user.click(screen.getByRole('button', { name: 'Remove a.jpg' }));

		await waitFor(() =>
			expect(document.activeElement).toBe(screen.getAllByRole('button', { name: 'Upload' })[0])
		);
	});

	it('moves focus to the next tile once a deletion is confirmed', async () => {
		stubApi([photo('p1'), photo('p2')]);
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Remove p1.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));

		await waitFor(() =>
			expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Open p2.jpg' }))
		);
	});

	it('says a favorite is the client’s choice before deleting it, and keeps the others', async () => {
		const api = stubApi([
			{ ...photo('p1'), favorited_at: '2026-11-05T10:00:00Z' },
			{ ...photo('p2'), favorited_at: '2026-11-05T10:00:00Z' }
		]);
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Remove p1.jpg' }));

		const dialog = screen.getByRole('dialog', { name: 'Delete p1.jpg?' });

		expect(within(dialog).getByText(/client.s favorites/)).toBeTruthy();

		await user.click(within(dialog).getByRole('button', { name: 'Delete' }));

		expect(api.calls('DELETE')).toHaveLength(1);
		expect(screen.getByRole('button', { name: 'Open p2.jpg' })).toBeTruthy();
	});

	it('does not mention favorites when deleting an ordinary photo', async () => {
		stubApi([photo('p1')]);
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Remove p1.jpg' }));

		expect(within(screen.getByRole('dialog')).queryByText(/favorites/)).toBeNull();
	});

	it('counts the favorites in a bulk deletion', async () => {
		stubApi([
			{ ...photo('p1'), favorited_at: '2026-11-05T10:00:00Z' },
			photo('p2'),
			{ ...photo('p3'), favorited_at: '2026-11-05T10:00:00Z' }
		]);
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Select' }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));

		expect(
			within(screen.getByRole('dialog', { name: 'Delete 3 photos?' })).getByText(
				/2 of them are the client.s favorites/
			)
		).toBeTruthy();
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

	it('puts a photo back as the server lists it when its deletion fails', async () => {
		const answer = Promise.withResolvers<number>();
		const server = [photo('p1'), photo('p2', 'waiting')];
		stubApi(server, { delete: answer.promise });
		const user = userEvent.setup();
		const { emit } = setup();

		await user.click(await screen.findByRole('button', { name: 'Remove p2.jpg' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));
		server[1] = photo('p2', { failed: 'x' });
		emit({ photo_id: 'p2', status: 'failed' });
		answer.resolve(500);

		expect((await screen.findByRole('alert')).textContent).toMatch(/Couldn’t delete p2\.jpg/);
		await waitFor(() => expect(tile('p2.jpg').textContent).toContain('Processing failed'));
		expect(screen.getAllByRole('listitem')).toHaveLength(2);
	});

	it('refetches without an alert when the photo to delete is already gone', async () => {
		const server = [photo('p1'), photo('p2')];
		const api = stubApi(server, { delete: 404 });
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Remove p2.jpg' }));
		server.pop();
		await user.click(screen.getByRole('button', { name: 'Delete' }));

		await waitFor(() => expect(api.lists()).toBe(2));
		expect(screen.queryByRole('alert')).toBeNull();
		expect(screen.queryByText('p2.jpg')).toBeNull();
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
		expect(within(tile('p1.jpg')).getByText('Waiting')).toBeTruthy();
	});

	it('opens a ready photo in the viewer', async () => {
		stubApi([photo('p1'), photo('p2', 'processing'), photo('p3')]);
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Open p3.jpg' }));

		const viewer = screen.getByRole('dialog', { name: /proof viewer/i });

		expect(within(viewer).getByText('3 of 3')).toBeTruthy();
		expect(within(viewer).getByRole('img').getAttribute('src')).toBe(
			'/api/v1/galleries/g1/photos/p3/preview'
		);
	});

	it('sets and removes the hero from the viewer', async () => {
		const api = stubApi([photo('p1'), photo('p2')]);
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Open p2.jpg' }));

		const viewer = screen.getByRole('dialog', { name: /proof viewer/i });

		await user.click(within(viewer).getByRole('button', { name: 'Set as cover' }));
		await waitFor(() =>
			expect(within(viewer).getByRole('button', { name: 'Remove cover' })).toBeTruthy()
		);
		expect(toasts.items).toEqual([]);
		expect(api.heroes()).toEqual(['p2']);
	});

	it('deletes from the viewer after confirming and moves to the next photo', async () => {
		const api = stubApi([photo('p1'), photo('p2')]);
		const user = userEvent.setup();

		setup();
		await user.click(await screen.findByRole('button', { name: 'Open p1.jpg' }));
		await user.click(
			within(screen.getByRole('dialog', { name: /proof viewer/i })).getByRole('button', {
				name: 'Delete'
			})
		);
		await user.click(
			within(screen.getByRole('dialog', { name: /delete p1\.jpg/i })).getByRole('button', {
				name: 'Delete'
			})
		);

		await waitFor(() => expect(api.calls('DELETE')).toEqual(['/api/v1/galleries/g1/photos/p1']));
		expect(
			within(screen.getByRole('dialog', { name: /proof viewer/i })).getByText('1 of 1')
		).toBeTruthy();
	});

	describe('hero photo', () => {
		const star = (name: string, label = 'Set as cover') =>
			within(tile(name)).getByRole('button', { name: label });

		it('moves the hero with one click and clears it by clicking the hero again', async () => {
			const api = stubApi([photo('p1'), photo('p2')]);
			const user = userEvent.setup();

			setup(undefined, 'p1');
			await screen.findByRole('button', { name: 'Open p2.jpg' });

			expect(star('p1.jpg', 'Remove cover').getAttribute('aria-pressed')).toBe('true');
			expect(star('p2.jpg').getAttribute('aria-pressed')).toBe('false');

			await user.click(star('p2.jpg'));

			await waitFor(() =>
				expect(star('p2.jpg', 'Remove cover').getAttribute('aria-pressed')).toBe('true')
			);
			expect(star('p1.jpg').getAttribute('aria-pressed')).toBe('false');

			await user.click(star('p2.jpg', 'Remove cover'));

			await waitFor(() => expect(star('p2.jpg').getAttribute('aria-pressed')).toBe('false'));
			expect(api.heroes()).toEqual(['p2', null]);
		});

		it('keeps the previous hero and says so when saving fails', async () => {
			stubApi([photo('p1'), photo('p2')], {
				hero: () => Response.json({ error: 'nope' }, { status: 500 })
			});
			const user = userEvent.setup();

			setup(undefined, 'p1');
			await user.click(await waitFor(() => star('p2.jpg')));

			expect((await screen.findByRole('alert')).textContent).toContain('cover photo');
			expect(star('p1.jpg', 'Remove cover').getAttribute('aria-pressed')).toBe('true');
			expect(star('p2.jpg').getAttribute('aria-pressed')).toBe('false');
		});

		it('goes to sign-in when saving finds the session ended', async () => {
			stubApi([photo('p1')], { hero: () => new Response(null, { status: 401 }) });
			const user = userEvent.setup();

			setup();
			await user.click(await waitFor(() => star('p1.jpg')));

			await waitFor(() =>
				expect(goto).toHaveBeenCalledWith('/login?expired=1', { refreshAll: true })
			);
		});

		it('offers no star on photos that are not ready, or while selecting', async () => {
			stubApi([photo('p1'), photo('p2', 'processing')]);
			const user = userEvent.setup();

			setup();
			await screen.findByText('p2.jpg');

			expect(within(tile('p2.jpg')).queryByRole('button', { name: /hero/ })).toBeNull();

			await user.click(screen.getByRole('button', { name: 'Select' }));

			expect(screen.queryAllByRole('button', { name: /hero/ })).toEqual([]);
		});

		it('forgets the hero once its photo is gone', async () => {
			const server = [photo('p1'), photo('p2')];
			stubApi(server);
			const { emit } = setup(undefined, 'p1');

			await screen.findByRole('button', { name: 'Open p2.jpg' });
			server.shift();
			emit({ photo_id: 'p1', status: 'deleted' });

			await waitFor(() => expect(screen.queryByRole('button', { name: 'Open p1.jpg' })).toBeNull());
			expect(star('p2.jpg').getAttribute('aria-pressed')).toBe('false');
		});
	});

	it('has no detectable accessibility violations', async () => {
		stubApi([photo('p1'), photo('p2', { failed: 'x' })]);
		const { queue, view } = setup();

		await screen.findByText('p2.jpg');
		queue.add('g1', [jpeg('a.jpg'), jpeg('b.jpg')]);
		await screen.findByText('Waiting');

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

			await user.click(screen.getByRole('button', { name: 'Select all' }));

			expect(counter()).toBe('2 selected');

			await user.click(screen.getByRole('button', { name: 'Unselect all' }));

			expect(counter()).toBe('0 selected');

			await user.click(screen.getByRole('button', { name: 'Exit selection' }));

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

		it('shows the bar with Delete only while selecting, and restores the toolbar on exit', async () => {
			stubApi([photo('p1'), photo('p2')]);
			setup();
			const user = userEvent.setup();

			const select = await screen.findByRole('button', { name: 'Select' });

			expect(document.querySelector('[data-slot="select-actions"]')).toBeNull();
			expect(screen.queryByRole('button', { name: 'Delete' })).toBeNull();

			await user.click(select);

			expect(document.querySelector('[data-slot="select-actions"]')).not.toBeNull();
			expect(screen.getByRole('button', { name: 'Select all' })).toBeTruthy();
			expect(screen.getByRole('button', { name: 'Delete' })).toHaveProperty('disabled', true);
			expect(screen.queryByRole('button', { name: 'Select' })).toBeNull();
			expect(screen.queryByRole('button', { name: 'Upload' })).toBeNull();

			await user.keyboard('{Escape}');

			expect(document.querySelector('[data-slot="select-actions"]')).toBeNull();
			expect(screen.getByRole('button', { name: 'Select' })).toBeTruthy();
			expect(screen.getByRole('button', { name: 'Upload' })).toBeTruthy();
		});

		it('uploads nothing when Delete is pressed', async () => {
			stubApi([photo('p1')]);
			setup();
			const user = userEvent.setup();
			const click = vi.spyOn(HTMLInputElement.prototype, 'click');

			await enter(user);
			await user.click(screen.getByRole('button', { name: 'Select all' }));
			await user.click(screen.getByRole('button', { name: 'Delete' }));

			expect(click).not.toHaveBeenCalled();
			expect(screen.getByRole('dialog')).toBeTruthy();
			click.mockRestore();
		});

		it('selects and unselects every photo from the bar, then exits', async () => {
			stubApi([photo('p1'), photo('p2')]);
			setup();
			const user = userEvent.setup();

			await enter(user);
			await user.click(checkbox('p1.jpg')!);
			await user.click(screen.getByRole('button', { name: 'Select all' }));

			expect(counter()).toBe('2 selected');
			expect(screen.getByRole('button', { name: 'Select all' })).toHaveProperty('disabled', true);

			await user.click(screen.getByRole('button', { name: 'Unselect all' }));

			expect(counter()).toBe('0 selected');
			expect(screen.getByRole('button', { name: 'Delete' })).toHaveProperty('disabled', true);

			await user.click(screen.getByRole('button', { name: 'Exit selection' }));

			expect(checkbox('p1.jpg')).toBeNull();
			expect(screen.getByRole('button', { name: 'Upload' })).toBeTruthy();
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
			// The Delete that opened the dialog left with the bar, so focus lands on Select.
			await waitFor(() =>
				expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Select' }))
			);
			expect(announced()).toContain('2 photos deleted');
		});

		it('keeps the confirmed count visible during optimistic bulk deletion', async () => {
			let finish!: (response: Response) => void;
			stubApi([photo('p1'), photo('p2')], {
				bulk: () => new Promise<Response>((resolve) => (finish = resolve))
			});
			setup();
			const user = userEvent.setup();
			await enter(user);
			await user.click(screen.getByRole('button', { name: 'Select all' }));
			await user.click(screen.getByRole('button', { name: 'Delete' }));
			await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Delete' }));
			expect(screen.getByRole('button', { name: 'Deleting…' })).toHaveProperty('disabled', true);
			expect(screen.getByRole('dialog', { name: 'Delete 2 photos?' })).toBeTruthy();
			finish(Response.json({ deleted: 2 }));
			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		});

		it('announces a deletion once, not again with the next change', async () => {
			const server = [photo('p1'), photo('p2'), photo('p3')];
			stubApi(server);
			const user = userEvent.setup();
			const { emit } = setup();

			await enter(user);
			await user.click(checkbox('p1.jpg')!);
			await confirmDeletion(user);
			await waitFor(() => expect(announced()).toContain('1 photo deleted'));

			server[0] = photo('p2', 'processing');
			emit({ photo_id: 'p2', status: 'processing' });

			await waitFor(() => expect(announced()).toContain('1 processing'));
			expect(announced()).not.toContain('deleted');
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

		it('restores the photos left after switching away and back during a deletion', async () => {
			let answer: (response: Response) => void = () => {};
			const kept = [photo('p1'), photo('p2'), photo('p3')];
			const api = stubApi(
				{ g1: kept, g2: [photo('q1', 'waiting')] },
				{
					bulk: (ids) =>
						ids.length === 2
							? new Promise((resolve) => (answer = resolve))
							: new Response(null, { status: 500 })
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
			await view.rerender({ galleryId: 'g1' });
			await waitFor(() => expect(api.lists()).toBe(3));
			kept.splice(0, 2);
			answer(Response.json({ deleted: 2 }));

			expect((await screen.findByRole('alert')).textContent).toMatch(/Deleted 2 of 3 photos/);
			await waitFor(() => expect(screen.queryByRole('button', { name: 'Open p1.jpg' })).toBeNull());
			expect(screen.getByRole('button', { name: 'Open p3.jpg' })).toBeTruthy();
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
			const server = [photo('p1'), photo('p2')];
			stubApi(server);
			const { emit } = setup();
			const user = userEvent.setup();

			await enter(user);
			await user.click(screen.getByRole('button', { name: 'Select all' }));
			server.pop();
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

	describe('finished photos', () => {
		it('lists, uploads and deletes with the finals query, without proof controls', async () => {
			const api = stubApi([photo('f1')]);
			const user = userEvent.setup();
			const { queue } = setup(undefined, null, false, 'finals');

			await screen.findByRole('button', { name: 'Open f1.jpg' });

			expect(screen.getByRole('region', { name: 'Final' })).toBeTruthy();
			expect(screen.queryByRole('button', { name: 'Set as cover' })).toBeNull();

			queue.add('g1', [jpeg('a.jpg')], 'finals');
			await user.click(await screen.findByRole('button', { name: 'Remove f1.jpg' }));
			await user.click(screen.getByRole('button', { name: 'Delete' }));

			await waitFor(() =>
				expect(api.calls('DELETE')).toEqual(['/api/v1/galleries/g1/photos/f1?finals=true'])
			);
			expect(api.lists()).toBe(2);
		});

		it('dates only the finals the client downloaded', async () => {
			stubApi([{ ...photo('f1'), downloaded_at: '2026-11-09T12:00:00Z' }, photo('f2')]);
			setup(undefined, null, false, 'finals');

			await screen.findByRole('button', { name: 'Open f1.jpg' });

			expect(within(tile('f1.jpg')).getByText(/^Downloaded Nov 9, 2026/)).toBeTruthy();
			expect(within(tile('f2.jpg')).queryByText(/^Downloaded/)).toBeNull();
		});

		it('ignores events about proofs and shows only finals changes', async () => {
			const server = [photo('f1', 'processing')];
			const api = stubApi(server);
			const { emit } = setup(undefined, null, false, 'finals');

			await screen.findByRole('listitem');
			server[0] = photo('f1');
			emit({ photo_id: 'f1', status: 'ready', kind: 'proof' });
			await new Promise((resolve) => setTimeout(resolve, 10));

			expect(api.lists()).toBe(1);
			expect(within(tile('f1.jpg')).queryByRole('button', { name: 'Open f1.jpg' })).toBeNull();

			emit({ photo_id: 'f1', status: 'ready', kind: 'final' });

			expect(
				await within(tile('f1.jpg')).findByRole('button', { name: 'Open f1.jpg' })
			).toBeTruthy();
		});

		it('offers an empty state for finished photos', async () => {
			stubApi([]);
			setup(undefined, null, false, 'finals');

			expect(await screen.findByText('Drop the finished photos here')).toBeTruthy();
		});
	});
});
