import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({
	afterNavigate: vi.fn(),
	beforeNavigate: vi.fn(),
	goto: vi.fn()
}));
vi.mock('$app/state', () => ({ page: { url: new URL('https://pp.test/dashboard/g1') } }));
// The photos tab has its own tests; here it only has to receive the gallery.
vi.mock('#lib/components/PhotosPanel.svelte', async () => ({
	default: (await import('#lib/components/PhotosPanelStub.test.svelte')).default
}));
vi.mock('#lib/components/FavoritesPanel.svelte', async () => ({
	default: (await import('#lib/components/FavoritesPanelStub.test.svelte')).default
}));
vi.mock('#lib/components/CommentsPanel.svelte', async () => ({
	default: (await import('#lib/components/CommentsPanelStub.test.svelte')).default
}));
vi.mock('#lib/components/GalleryContactsPanel.svelte', async () => ({
	default: (await import('#lib/components/GalleryContactsPanelStub.test.svelte')).default
}));
vi.mock('#lib/gallery/notifications.svelte.js', async (importOriginal) => ({
	...(await importOriginal<object>()),
	getNotifications: vi.fn()
}));
import { goto } from '$app/navigation';
import { getNotifications, NotificationCenter } from '#lib/gallery/notifications.svelte.js';
import type { Gallery } from '#lib/gallery/api.js';
import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
import GalleryPage from './+page.svelte';

const base: Gallery = {
	id: 'g1',
	title: 'Wedding',
	layout: 'grid',
	cover_layout: 'full_bleed',
	focal_point: { x: 50, y: 35 },
	heading_font: 'editorial',
	grid_density: 'compact',
	grid_spacing: 'compact',
	expires_at: null,
	included_photos: 5,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	last_opened_at: null,
	shoot_date: null,
	states: ['draft'],
	created_at: '2026-01-01T10:00:00Z',
	updated_at: '2026-01-02T10:00:00Z'
};
/** Opens the date picker named `label` and picks `day` in the month `months` away from today. */
async function pickDay(label: string, months: number, day: string, scope = document.body) {
	const user = userEvent.setup();

	await user.click(within(scope).getByLabelText(label));

	for (let step = 0; step < Math.abs(months); step++) {
		await user.click(
			screen.getByRole('button', { name: months < 0 ? 'Previous month' : 'Next month' })
		);
	}

	await user.click(
		within(screen.getByRole('grid')).getByText(day, {
			selector: '[data-bits-day]:not([data-outside-month])'
		})
	);
}

const open = (over: Partial<Gallery> = {}) =>
	render(GalleryPage, {
		props: { data: { gallery: { ...base, ...over }, idleTimeoutMs: 2592000000 } as never }
	});
// Echoes the patch back like the API does; renew calls (auth API) just succeed.
const stubApi = (status = 200) => {
	const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
		if (url.includes('/auth/')) return new Response(null, { status: 204 });
		if (!init?.method || init.method === 'GET') {
			if (url.endsWith('/settings')) {
				return Response.json({
					studio_name: '',
					mark: 'name',
					pattern: 'repeat',
					position: 8,
					size: 4,
					signature: 'name',
					signature_size: 4,
					proof_quality: 'medium',
					has_logo: false
				});
			}
			return Response.json([]);
		}
		if (status !== 200) return new Response(JSON.stringify({ error: 'nope' }), { status });
		return new Response(JSON.stringify({ ...base, ...JSON.parse(init?.body as string) }));
	});
	vi.stubGlobal('fetch', fetcher);
	return () =>
		fetcher.mock.calls.filter(
			([url, init]) => url.includes('/galleries/') && (init?.method ?? 'GET') !== 'GET'
		);
};

beforeEach(() => {
	vi.mocked(goto).mockClear();
	toasts.items = [];
});
afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe('gallery page', () => {
	it('marks the gallery notifications read on open and when moving to another gallery', async () => {
		vi.stubGlobal('fetch', async () => Response.json({ notifications: [], unread: 0 }));
		const center = new NotificationCenter();
		const markGalleryRead = vi.spyOn(center, 'markGalleryRead');
		vi.mocked(getNotifications).mockReturnValue(center);

		const view = open();

		await waitFor(() => expect(markGalleryRead).toHaveBeenCalledWith('g1'));

		await center.refresh();
		await view.rerender({
			data: { gallery: { ...base, id: 'g2' }, idleTimeoutMs: 2592000000 } as never
		});

		await waitFor(() => expect(markGalleryRead).toHaveBeenCalledWith('g2'));
		expect(markGalleryRead).toHaveBeenCalledTimes(2);
		vi.mocked(getNotifications).mockReset();
	});

	it('links back to the galleries', () => {
		open();

		expect(screen.getByRole('link', { name: 'Back to galleries' }).getAttribute('href')).toBe(
			'/dashboard'
		);
	});

	it('previews an unshared gallery, flushing pending edits first, and returns focus to Preview when closed', async () => {
		const patches = stubApi();
		const user = userEvent.setup();
		open({ share_token: null, hero_id: 'p1' });

		await user.click(screen.getByRole('tab', { name: 'Design' }));
		const pad = screen.getByRole('button', { name: 'Cover focal point (50%, 35%)' });
		pad.focus();
		await user.keyboard('{ArrowRight}');
		expect(patches()).toHaveLength(0);

		const button = screen.getByRole('button', { name: 'Preview' });
		await user.click(button);
		expect(patches()).toHaveLength(1);
		expect(JSON.parse(patches()[0][1]?.body as string)).toEqual({
			focal_point: { x: 51, y: 35 }
		});
		expect(screen.getByRole('dialog', { name: 'Client preview' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Close preview' }));
		expect(screen.queryByRole('dialog', { name: 'Client preview' })).toBeNull();
		expect(document.activeElement).toBe(button);
		expect(screen.queryByLabelText('Client link')).toBeNull();
	});

	it('renders the share control inside Settings with a link built from the page origin', async () => {
		const user = userEvent.setup();
		open({ share_token: 'B'.repeat(22) });
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		expect((screen.getByLabelText('Client link') as HTMLInputElement).value).toBe(
			`https://pp.test/g/${'B'.repeat(22)}`
		);
	});

	it('shows no Saved toast after a save, only the error toast on failure', async () => {
		const user = userEvent.setup();

		stubApi(500);
		open();
		await user.click(screen.getByRole('tab', { name: 'Design' }));
		await user.click(screen.getByRole('button', { name: 'Staggered columns' }));
		await waitFor(() => expect(toasts.items).toHaveLength(1));

		stubApi();
		await user.click(screen.getByRole('button', { name: 'Staggered columns' }));
		await waitFor(() => expect(screen.queryByText('Saving…')).toBeNull());

		expect(toasts.items).toHaveLength(1);
	});

	it('sends only the changed field without an immediate success toast', async () => {
		const patches = stubApi();
		const user = userEvent.setup();
		open();
		await user.click(screen.getByRole('tab', { name: 'Design' }));
		await user.click(screen.getByRole('button', { name: 'Staggered columns' }));
		await waitFor(() => expect(patches()).toHaveLength(1));
		await waitFor(() => expect(screen.queryByText('Saving…')).toBeNull());
		expect(toasts.items).toEqual([]);
		expect(patches()).toHaveLength(1);
		const [url, init] = patches()[0];
		expect(url).toBe('/api/v1/galleries/g1');
		expect(init?.method).toBe('PATCH');
		expect(JSON.parse(init?.body as string)).toEqual({ layout: 'masonry' });
		expect(init?.keepalive).toBe(true);
	});

	it('saves the title on blur and sends null for an emptied title', async () => {
		const patches = stubApi();
		const user = userEvent.setup();
		open();
		const title = screen.getByLabelText('Gallery title') as HTMLInputElement;
		await user.clear(title);
		await user.tab();
		await waitFor(() => expect(patches()).toHaveLength(1));
		expect(JSON.parse(patches()[0][1]?.body as string)).toEqual({ title: null });
		expect(title.placeholder).toBe('Untitled gallery');
		await user.type(title, 'Wedding II');
		await user.tab();
		await waitFor(() => expect(patches()).toHaveLength(2));
		expect(JSON.parse(patches()[1][1]?.body as string)).toEqual({ title: 'Wedding II' });
	});

	it('shows the untitled text for a gallery without a title', () => {
		open({ title: null });
		expect((screen.getByLabelText('Gallery title') as HTMLInputElement).value).toBe('');
		expect(document.title).toMatch(/^Untitled gallery/);
	});

	it('debounces title typing into one save', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const patches = stubApi();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		open();
		await user.type(screen.getByLabelText('Gallery title'), 'abc');
		expect(patches()).toHaveLength(0);
		await vi.advanceTimersByTimeAsync(700);
		expect(patches()).toHaveLength(1);
		expect(JSON.parse(patches()[0][1]?.body as string)).toEqual({ title: 'Weddingabc' });
	});

	it('shows a toast and puts the switch back when a setting fails to save', async () => {
		stubApi(500);
		const user = userEvent.setup();
		open();
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		const toggle = screen.getByRole('switch', { name: 'Enable watermarks' });
		await user.click(toggle);
		await waitFor(() => expect(toasts.items).toHaveLength(1));
		expect(toggle.getAttribute('aria-checked')).toBe('false');
		expect(screen.queryByText('Couldn’t save')).toBeNull();
	});

	it('shows Retry for a failed title that resends the attempted value', async () => {
		const patches = stubApi(500);
		const user = userEvent.setup();
		open();
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await user.type(screen.getByLabelText('Gallery title'), ' II');
		await user.click(screen.getByRole('switch', { name: 'Enable watermarks' }));
		await waitFor(() => expect(patches()).toHaveLength(2));
		expect(await screen.findByText('Couldn’t save')).toBeTruthy();
		stubApi();
		await user.click(screen.getByRole('button', { name: 'Retry' }));
		await waitFor(() => expect(screen.queryByText('Couldn’t save')).toBeNull());
		expect(screen.queryByText('Saved')).toBeNull();
	});

	it('reverts a failed title when leaving the field, but Retry still works while focused', async () => {
		stubApi(500);
		const user = userEvent.setup();
		open();
		const title = screen.getByLabelText('Gallery title') as HTMLInputElement;
		await user.type(title, '!');
		await user.tab();
		expect(await screen.findByText('Couldn’t save')).toBeTruthy();
		expect(title.value).toBe('Wedding!');
		await user.click(title);
		await user.click(document.body);
		expect(title.value).toBe('Wedding');
		expect(screen.queryByText('Couldn’t save')).toBeNull();

		await user.type(title, '?');
		await user.tab();
		expect(await screen.findByText('Couldn’t save')).toBeTruthy();
		const retried = stubApi();
		await user.click(title);
		await user.click(screen.getByRole('button', { name: 'Retry' }));
		await waitFor(() => expect(screen.queryByText('Couldn’t save')).toBeNull());
		expect(screen.queryByText('Saved')).toBeNull();
		expect(title.value).toBe('Wedding?');
		expect(retried()).toHaveLength(1);
	});

	// Gallery PATCHes stay pending until the test settles them, in any order.
	const deferApi = () => {
		const pending: ((status: number) => void)[] = [];
		vi.stubGlobal(
			'fetch',
			vi.fn(async (url: string, init?: RequestInit) => {
				if (url.includes('/auth/')) return new Response(null, { status: 204 });
				const status = await new Promise<number>((resolve) => pending.push(resolve));
				return status === 200
					? new Response(JSON.stringify({ ...base, ...JSON.parse(init?.body as string) }))
					: new Response('{}', { status });
			})
		);
		return pending;
	};

	it('ignores a stale failure after a newer save of the same field succeeded', async () => {
		const pending = deferApi();
		const user = userEvent.setup();
		open();
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		const toggle = screen.getByRole('switch', { name: 'Enable watermarks' });
		await user.click(toggle);
		await user.click(toggle);
		await waitFor(() => expect(pending).toHaveLength(2));
		pending[1](200);
		pending[0](500);
		await waitFor(() => expect(toasts.items).toHaveLength(1));
	});

	it('saves an edited title normally when leaving the field after a failure', async () => {
		const patches = stubApi(500);
		const user = userEvent.setup();
		open();
		const title = screen.getByLabelText('Gallery title') as HTMLInputElement;
		await user.type(title, '!');
		await user.tab();
		expect(await screen.findByText('Couldn’t save')).toBeTruthy();
		const ok = stubApi();
		await user.type(title, '?');
		await user.tab();
		await waitFor(() => expect(screen.queryByText('Couldn’t save')).toBeNull());
		expect(screen.queryByText('Saved')).toBeNull();
		expect(title.value).toBe('Wedding!?');
		expect(JSON.parse(ok()[0][1]?.body as string)).toEqual({ title: 'Wedding!?' });
		expect(patches()).toHaveLength(1);
	});

	it('lets keyboard users Tab from a failed title to Retry', async () => {
		stubApi(500);
		const user = userEvent.setup();
		open();
		const title = screen.getByLabelText('Gallery title') as HTMLInputElement;
		await user.type(title, '!');
		await user.tab();
		await screen.findByText('Couldn’t save');
		const ok = stubApi();
		await user.click(title);
		await user.tab();
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Retry' }));
		expect(title.value).toBe('Wedding!');
		await user.keyboard('{Enter}');
		await waitFor(() => expect(screen.queryByText('Couldn’t save')).toBeNull());
		expect(screen.queryByText('Saved')).toBeNull();
		expect(ok()).toHaveLength(1);
	});

	it('redirects to sign-in when the session expired', async () => {
		stubApi(401);
		const user = userEvent.setup();
		open();
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await user.click(screen.getByRole('switch', { name: 'Enable watermarks' }));
		await waitFor(() => expect(goto).toHaveBeenCalledWith('/login?expired=1', expect.anything()));
	});

	it('sends the end of the chosen day in local time', async () => {
		const patches = stubApi();
		const user = userEvent.setup();
		open();
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await pickDay('Open until', 1, '4');
		await waitFor(() => expect(patches()).toHaveLength(1));
		const sent = new Date(JSON.parse(patches()[0][1]?.body as string).expires_at);
		const next = new Date();
		next.setDate(1);
		next.setMonth(next.getMonth() + 1);
		expect([sent.getFullYear(), sent.getMonth(), sent.getDate()]).toEqual([
			next.getFullYear(),
			next.getMonth(),
			4
		]);
		expect([sent.getHours(), sent.getMinutes(), sent.getSeconds()]).toEqual([23, 59, 59]);
	});

	it('allows a past expiry, marks the gallery expired and clears it with null', async () => {
		const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
			if (url.includes('/auth/')) return new Response(null, { status: 204 });
			const patch = JSON.parse(init?.body as string);

			return new Response(
				JSON.stringify({
					...base,
					...patch,
					states: patch.expires_at ? ['draft', 'expired'] : ['draft']
				})
			);
		});

		vi.stubGlobal('fetch', fetcher);
		const user = userEvent.setup();
		open();
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await pickDay('Open until', -1, '1');
		expect(await screen.findByText('Expired')).toBeTruthy();
		await user.click(screen.getByLabelText('Open until'));
		await user.click(screen.getByRole('button', { name: 'Clear' }));
		expect(await screen.findByText('Draft')).toBeTruthy();
		const bodies = fetcher.mock.calls
			.filter(([url]) => url.includes('/galleries/'))
			.map(([, init]) => JSON.parse(init?.body as string));
		expect(bodies[1]).toEqual({ expires_at: null });
	});

	it('keeps the current expiry status when an older title response arrives last', async () => {
		let finishTitle!: () => void;
		vi.stubGlobal('fetch', async (url: string, init?: RequestInit) => {
			if (url.includes('/auth/')) return new Response(null, { status: 204 });
			const patch = JSON.parse(init?.body as string);

			if (patch.title) await new Promise<void>((resolve) => (finishTitle = resolve));

			return Response.json({
				...base,
				...patch,
				states: patch.expires_at ? ['draft', 'expired'] : ['draft']
			});
		});
		open();
		const user = userEvent.setup();
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await user.type(screen.getByLabelText('Gallery title'), ' II');
		await user.tab();
		await waitFor(() => expect(finishTitle).toBeDefined());
		await pickDay('Open until', -1, '1');
		await screen.findByText('Expired');
		finishTitle();
		await waitFor(() => expect(document.title).toBe('Wedding II — PhotoProofing'));
		expect(screen.getByText('Expired')).toBeTruthy();
	});

	it('shows the stored expiry as a local date', async () => {
		const user = userEvent.setup();
		const stored = new Date(2030, 5, 15, 23, 59, 59).toISOString();
		open({ expires_at: stored });
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		expect(screen.getByLabelText('Open until').textContent).toContain('Jun 15, 2030');
	});

	it('sends null when included photos is cleared, rejects zero, and preserves debounced edits across tab switches', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const patches = stubApi();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		open();
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		const input = screen.getByLabelText('Included photos') as HTMLInputElement;
		await user.clear(input);
		await user.tab();
		await waitFor(() => expect(patches()).toHaveLength(1));
		expect(JSON.parse(patches()[0][1]?.body as string)).toEqual({ included_photos: null });
		await user.type(input, '0');
		await user.tab();
		expect(toasts.items).toHaveLength(1);
		expect(patches()).toHaveLength(1);

		await user.clear(input);
		await user.type(input, '12');
		await user.click(screen.getByRole('tab', { name: /^Photos/ }));
		await vi.advanceTimersByTimeAsync(700);
		expect(patches()).toHaveLength(2);
		expect(JSON.parse(patches()[1][1]?.body as string)).toEqual({ included_photos: 12 });
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		expect((screen.getByLabelText('Included photos') as HTMLInputElement).value).toBe('12');
	});

	it('does not clobber in-progress Included photos typing or optimistic layout when an earlier save resolves', async () => {
		let resolveTitle!: () => void;
		let resolveFirstIncluded!: () => void;
		const patches: unknown[] = [];

		vi.stubGlobal(
			'fetch',
			vi.fn(async (url: string, init?: RequestInit) => {
				if (url.includes('/auth/')) return new Response(null, { status: 204 });
				const patch = JSON.parse(init?.body as string);
				patches.push(patch);

				if ('title' in patch) await new Promise<void>((r) => (resolveTitle = r));
				if (patch.included_photos === 1) await new Promise<void>((r) => (resolveFirstIncluded = r));

				return Response.json({ ...base, ...patch });
			})
		);

		vi.useFakeTimers({ shouldAdvanceTime: true });
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		open();

		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		const titleInput = screen.getByLabelText('Gallery title');
		const includedInput = screen.getByLabelText('Included photos') as HTMLInputElement;

		await user.type(titleInput, ' II');
		await user.click(includedInput);
		await waitFor(() => expect(resolveTitle).toBeDefined());

		await user.clear(includedInput);
		await user.type(includedInput, '1');
		resolveTitle();
		await waitFor(() => expect(document.title).toBe('Wedding II — PhotoProofing'));
		expect(includedInput.value).toBe('1');

		await vi.advanceTimersByTimeAsync(650);
		await waitFor(() => expect(resolveFirstIncluded).toBeDefined());

		await user.type(includedInput, '2');
		resolveFirstIncluded();
		await vi.advanceTimersByTimeAsync(50);
		expect(includedInput.value).toBe('12');

		await user.click(screen.getByRole('tab', { name: 'Design' }));
		const staggered = screen.getByRole('button', { name: 'Staggered columns' });
		await user.click(staggered);
		await vi.advanceTimersByTimeAsync(650);
		expect(staggered.getAttribute('aria-pressed')).toBe('true');
		expect(patches).toContainEqual({ included_photos: 12 });
	});

	it('has no axe violations across Photos, Design, and Settings views', async () => {
		const user = userEvent.setup();
		const { container } = open();
		expect((await axe(container)).violations).toEqual([]);
		await user.click(screen.getByRole('tab', { name: 'Design' }));
		expect((await axe(container)).violations).toEqual([]);
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		expect((await axe(container)).violations).toEqual([]);
	});

	describe('progress header', () => {
		const photo = (id: string, status: unknown = 'ready') => ({
			id,
			original_name: `${id}.jpg`,
			size: 1,
			width: 1,
			height: 1,
			status,
			favorited_at: null,
			created_at: '2026-01-01T10:00:00Z'
		});
		const stubShareFlow = (photos: unknown[] = [photo('p1')]) => {
			const fetcher = vi.fn(async (url: string) => {
				if (url.endsWith('/share')) {
					return new Response(
						JSON.stringify({ ...base, share_token: 'C'.repeat(22), states: ['shared'] })
					);
				}
				if (url.endsWith('/photos') || url.endsWith('/photos?finals=true'))
					return new Response(JSON.stringify(photos));

				return new Response(null, { status: 204 });
			});
			vi.stubGlobal('fetch', fetcher);

			return () => fetcher.mock.calls.filter(([url]) => url.endsWith('/share'));
		};

		it('shows the title and the draft status', () => {
			open();
			expect(screen.getByText('Draft')).toBeTruthy();
			expect(screen.getByText('Created')).toBeTruthy();
			expect(screen.getByLabelText('Gallery title')).toBeTruthy();
		});

		it('shares only after confirming in the dialog', async () => {
			const shares = stubShareFlow();
			const user = userEvent.setup();
			open();

			await user.click(screen.getByRole('button', { name: 'Share' }));
			expect(await screen.findByRole('dialog', { name: 'Share 1 photo?' })).toBeTruthy();
			expect(shares()).toHaveLength(0);

			await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Share' }));
			expect(await screen.findByRole('dialog', { name: 'Your gallery is shared' })).toBeTruthy();
			expect(shares()).toHaveLength(1);

			await user.click(screen.getByRole('button', { name: 'Done' }));
			expect(screen.getByText('Prepublish')).toBeTruthy();
			expect(screen.queryByRole('button', { name: 'Share' })).toBeNull();
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			expect(screen.getByLabelText('Client link')).toBeTruthy();
		});

		it('does not share when the dialog is dismissed', async () => {
			const shares = stubShareFlow();
			const user = userEvent.setup();
			open();
			await user.click(screen.getByRole('button', { name: 'Share' }));
			await user.click(await screen.findByRole('button', { name: 'Cancel' }));
			expect(shares()).toHaveLength(0);
			expect(screen.getByRole('button', { name: 'Share' })).toBeTruthy();
		});

		it('warns instead of blocking when there are no photos', async () => {
			stubShareFlow([]);
			const user = userEvent.setup();
			open();
			await user.click(screen.getByRole('button', { name: 'Share' }));
			expect(await screen.findByRole('dialog', { name: 'Share with no photos?' })).toBeTruthy();
			expect(screen.getByText('Clients see an empty gallery until you upload.')).toBeTruthy();
			expect(screen.getByRole('button', { name: 'Share anyway' })).toBeTruthy();
		});

		it('mentions photos still processing or failed', async () => {
			stubShareFlow([photo('p1'), photo('p2', 'processing'), photo('p3', { failed: 'bad' })]);
			const user = userEvent.setup();
			open();
			await user.click(screen.getByRole('button', { name: 'Share' }));
			expect(await screen.findByText('1 is still uploading and 1 failed.')).toBeTruthy();
			expect(screen.getByRole('button', { name: 'Share anyway' })).toBeTruthy();
		});

		it('names the open-until date only when one is set', async () => {
			stubShareFlow();
			const user = userEvent.setup();
			open({ expires_at: '2026-09-12T12:00:00Z' });
			await user.click(screen.getByRole('button', { name: 'Share' }));
			expect(await screen.findByText(/pick favorites until 12 Sep 2026\./)).toBeTruthy();
		});

		it('starts from the link row and an enabled Publish when already shared', async () => {
			const user = userEvent.setup();
			open({ share_token: 'B'.repeat(22), states: ['shared'] });
			expect(screen.getByText('Prepublish')).toBeTruthy();
			expect((screen.getByRole('button', { name: 'Publish' }) as HTMLButtonElement).disabled).toBe(
				false
			);
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			expect(screen.getByLabelText('Client link')).toBeTruthy();
		});

		it('names the gallery in the Publish text, plural when it has an ampersand', async () => {
			stubShareFlow();
			const user = userEvent.setup();
			open({ share_token: 'B'.repeat(22), title: 'Camille & Hugo' });
			await user.click(screen.getByRole('button', { name: 'Publish' }));
			expect(await screen.findByText(/Camille & Hugo see them at the same link/)).toBeTruthy();
		});

		it('publishes after confirming and shows the gallery as published', async () => {
			const fetcher = vi.fn(async (url: string) =>
				url.endsWith('/deliver')
					? new Response(
							JSON.stringify({
								...base,
								share_token: 'B'.repeat(22),
								states: ['delivered']
							})
						)
					: new Response(JSON.stringify([photo('p1')]))
			);
			vi.stubGlobal('fetch', fetcher);
			const user = userEvent.setup();
			open({ share_token: 'B'.repeat(22) });
			await user.click(screen.getByRole('button', { name: 'Publish' }));
			await user.click(
				within(await screen.findByRole('dialog', { name: 'Publish 1 photo?' })).getByRole(
					'button',
					{ name: 'Publish' }
				)
			);
			expect(
				await screen.findByText('Publish', { selector: '[data-slot="status-chip"]' })
			).toBeTruthy();
			expect(screen.queryByRole('button', { name: 'Publish' })).toBeNull();
			expect(toasts.items).toEqual([]);
			expect(fetcher.mock.calls.some(([url]) => url.endsWith('/g1/deliver'))).toBe(true);
		});

		it('reports a failed publish and keeps the dialog open to try again', async () => {
			vi.stubGlobal(
				'fetch',
				vi.fn(async (url: string) =>
					url.endsWith('/deliver')
						? new Response('{}', { status: 500 })
						: new Response(JSON.stringify([photo('p1')]))
				)
			);
			const user = userEvent.setup();
			open({ share_token: 'B'.repeat(22) });
			await user.click(screen.getByRole('button', { name: 'Publish' }));
			await user.click(
				within(await screen.findByRole('dialog', { name: 'Publish 1 photo?' })).getByRole(
					'button',
					{ name: 'Publish' }
				)
			);
			expect((await screen.findByRole('alert')).textContent).toMatch(/Couldn’t publish/);
		});

		it('reports a failed share and keeps the dialog open to try again', async () => {
			vi.stubGlobal(
				'fetch',
				vi.fn(async (url: string) =>
					url.endsWith('/photos') ? new Response('[]') : new Response('{}', { status: 500 })
				)
			);
			const user = userEvent.setup();
			open();
			await user.click(screen.getByRole('button', { name: 'Share' }));
			await user.click(await screen.findByRole('button', { name: 'Share anyway' }));
			expect((await screen.findByRole('alert')).textContent).toMatch(/Couldn’t share/);
			expect(screen.getByRole('button', { name: 'Share anyway' })).toBeTruthy();
		});
	});

	describe('restoring', () => {
		const archived: Partial<Gallery> = {
			states: ['shared', 'archived'],
			expires_at: '2026-09-07T21:59:59Z'
		};
		// `restored` is the gallery the restore endpoint answers with; PATCH echoes the change.
		const stubRestore = (changes: Partial<Gallery>, patchStatus = 200) => {
			const restored = { expires_at: archived.expires_at, ...changes };
			const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
				if (url.includes('/auth/')) return new Response(null, { status: 204 });
				if (url.endsWith('/restore')) return new Response(JSON.stringify({ ...base, ...restored }));
				if (patchStatus !== 200) {
					return new Response(JSON.stringify({ error: 'nope' }), { status: patchStatus });
				}

				return new Response(
					JSON.stringify({
						...base,
						...restored,
						states: ['shared'],
						...JSON.parse(init?.body as string)
					})
				);
			});
			vi.stubGlobal('fetch', fetcher);

			return () => fetcher.mock.calls.filter(([url]) => url.includes('/galleries/'));
		};
		const restore = () =>
			userEvent.setup().click(screen.getAllByRole('button', { name: 'Restore' })[0]);

		it('restores without prompting when the gallery is no longer expired', async () => {
			stubRestore({ states: ['shared'], expires_at: null });
			open(archived);

			await restore();

			await waitFor(() => expect(screen.queryByRole('button', { name: 'Restore' })).toBeNull());
			expect(toasts.items).toEqual([]);
			expect(screen.queryByRole('dialog')).toBeNull();
			expect(screen.queryByRole('button', { name: 'Restore' })).toBeNull();
		});

		it('prompts when the restored gallery is still expired', async () => {
			stubRestore({ states: ['shared', 'expired'] });
			open(archived);

			await restore();

			expect(
				await screen.findByRole('dialog', { name: 'Clients can’t see this gallery yet' })
			).toBeTruthy();
			expect(screen.getByText(/link expired on 7 Sep/)).toBeTruthy();
		});

		it('keeps the gallery restored and expired when the prompt is dismissed', async () => {
			const calls = stubRestore({ states: ['shared', 'expired'] });
			open(archived);
			await restore();

			await userEvent.setup().click(await screen.findByRole('button', { name: 'Keep expired' }));

			expect(screen.queryByRole('dialog')).toBeNull();
			expect(calls()).toHaveLength(1);
			expect(screen.getByText('Expired')).toBeTruthy();
		});

		it('saves a new date through the gallery update and clears the expired state', async () => {
			const calls = stubRestore({ states: ['shared', 'expired'] });
			open(archived);
			await restore();
			await pickDay(
				'Open until',
				1,
				'2',
				await screen.findByRole('dialog', { name: 'Clients can’t see this gallery yet' })
			);

			await userEvent.setup().click(screen.getByRole('button', { name: 'Set date' }));

			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
			const [url, init] = calls()[1];
			expect(url).toBe('/api/v1/galleries/g1');
			expect(init?.method).toBe('PATCH');
			expect(Object.keys(JSON.parse(init?.body as string))).toEqual(['expires_at']);
			expect(toasts.items).toEqual([]);
			expect(screen.queryByText('Expired')).toBeNull();
		});

		it('clears the expiry with No expiry', async () => {
			const calls = stubRestore({ states: ['shared', 'expired'] });
			open(archived);
			await restore();

			await userEvent.setup().click(await screen.findByRole('button', { name: 'No expiry' }));

			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
			expect(JSON.parse(calls()[1][1]?.body as string)).toEqual({ expires_at: null });
		});

		it('keeps the prompt open when saving the date fails', async () => {
			stubRestore({ states: ['shared', 'expired'] }, 500);
			open(archived);
			await restore();

			await userEvent.setup().click(await screen.findByRole('button', { name: 'No expiry' }));

			await waitFor(() =>
				expect(toasts.items.map(({ message }) => message)).toContain(
					'Something went wrong. Please try again.'
				)
			);
			expect(screen.getByRole('dialog')).toBeTruthy();
		});

		it('reports a failed restore and shows no prompt', async () => {
			vi.stubGlobal('fetch', async () => new Response('{"error":"nope"}', { status: 500 }));
			open(archived);

			await restore();

			await waitFor(() =>
				expect(toasts.items.map(({ message }) => message)).toEqual([
					'Something went wrong. Please try again.'
				])
			);
			expect(screen.queryByRole('dialog')).toBeNull();
			expect(screen.getAllByRole('button', { name: 'Restore' }).length).toBeGreaterThan(0);
		});
	});

	describe('archive and delete', () => {
		const stub = (status = 200) => {
			const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
				if (url.includes('/photos')) return new Response('[]');
				if (status !== 200) return new Response('{"error":"nope"}', { status });
				if (init?.method === 'DELETE') return new Response(null, { status: 204 });
				return new Response(JSON.stringify({ ...base, states: ['draft', 'archived'] }));
			});
			vi.stubGlobal('fetch', fetcher);
			return fetcher;
		};
		const confirm = async (action: 'Archive' | 'Delete') => {
			const user = userEvent.setup();
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			await user.click(screen.getByRole('button', { name: action }));
			const dialog = await screen.findByRole('dialog');
			await user.click(within(dialog).getByRole('button', { name: action }));
		};

		it('archives through the API and shows the archived state', async () => {
			const fetcher = stub();
			open();
			await confirm('Archive');
			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
			expect(
				fetcher.mock.calls.some(
					([url, init]) => url === '/api/v1/galleries/g1/archive' && init?.method === 'POST'
				)
			).toBe(true);
			expect(screen.getAllByRole('button', { name: 'Restore' }).length).toBeGreaterThan(0);
			expect(toasts.items).toEqual([]);
		});

		it('deletes through the API and returns to the dashboard', async () => {
			const fetcher = stub();
			open();
			await confirm('Delete');
			await waitFor(() => expect(goto).toHaveBeenCalledWith('/dashboard', { refreshAll: true }));
			expect(
				fetcher.mock.calls.some(
					([url, init]) => url === '/api/v1/galleries/g1' && init?.method === 'DELETE'
				)
			).toBe(true);
		});

		it.each(['Archive', 'Delete'] as const)('sends %s to sign in on a 401', async (action) => {
			stub(401);
			open();
			await confirm(action);
			await waitFor(() =>
				expect(goto).toHaveBeenCalledWith('/login?expired=1', { refreshAll: true })
			);
		});

		it.each(['Archive', 'Delete'] as const)('shows a %s failure in the dialog', async (action) => {
			stub(500);
			open();
			await confirm(action);
			expect((await screen.findByRole('alert')).textContent).toMatch(/Couldn’t/);
			expect(screen.getByRole('dialog')).toBeTruthy();
			expect(goto).not.toHaveBeenCalled();
		});
	});

	describe('tabs', () => {
		it('offers Collection and Studio tabs with Photos selected by default', async () => {
			vi.stubGlobal('fetch', async () => new Response('[]'));
			open();
			const tabs = screen
				.getAllByRole('tab')
				.map((tab) => tab.textContent?.replace(/\s+/g, ' ').trim());
			expect(tabs).toEqual([
				'Photos',
				'Favorites',
				'Comments',
				'Final',
				'Design',
				'Contacts',
				'Settings'
			]);
			expect(screen.getByRole('tab', { name: /^Photos/ }).getAttribute('aria-selected')).toBe(
				'true'
			);
			expect(screen.getByRole('tabpanel').textContent).toMatch(/Photos of g1/);
		});

		it('switches panels after the gallery is shared', async () => {
			vi.stubGlobal('fetch', async () => new Response('[]'));
			const user = userEvent.setup();
			open({ share_token: 'B'.repeat(22), states: ['shared'] });
			await user.click(screen.getByRole('tab', { name: /^Final/ }));
			expect(screen.getByRole('tab', { name: /^Final/ }).getAttribute('aria-selected')).toBe(
				'true'
			);
			expect(screen.getAllByRole('tabpanel').length).toBeGreaterThan(0);
			await user.click(screen.getByRole('tab', { name: 'Favorites' }));
			expect(screen.getByRole('tabpanel').textContent).toMatch(/No favorites yet/);
		});

		it('disables Favorites, Comments and Final for an unshared gallery while keeping Design and Settings enabled', () => {
			open();
			expect((screen.getByRole('tab', { name: 'Favorites' }) as HTMLButtonElement).disabled).toBe(
				true
			);
			expect((screen.getByRole('tab', { name: 'Comments' }) as HTMLButtonElement).disabled).toBe(
				true
			);
			expect((screen.getByRole('tab', { name: 'Final' }) as HTMLButtonElement).disabled).toBe(true);
			expect((screen.getByRole('tab', { name: 'Design' }) as HTMLButtonElement).disabled).toBe(
				false
			);
			expect((screen.getByRole('tab', { name: 'Settings' }) as HTMLButtonElement).disabled).toBe(
				false
			);
		});
	});
});
