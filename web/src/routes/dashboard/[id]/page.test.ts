import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({
	afterNavigate: vi.fn(),
	beforeNavigate: vi.fn(),
	goto: vi.fn()
}));
const state = vi.hoisted(() => ({ page: { url: new URL('https://pp.test/dashboard/g1') } }));
vi.mock('$app/state', () => state);
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
vi.mock('#lib/components/GalleryActivityPanel.svelte', async () => ({
	default: (await import('#lib/components/GalleryActivityPanelStub.test.svelte')).default
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
import { DEFAULT_SETTINGS } from '#lib/settings/settings.js';
import { tick } from 'svelte';
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
	state.page.url = new URL('https://pp.test/dashboard/g1');
	vi.mocked(goto).mockClear();
	toasts.items = [];
});
afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe('gallery page', () => {
	it('keeps same-gallery drafts but replaces the dialog and mutation target on gallery navigation', async () => {
		const user = userEvent.setup();
		const requests: string[] = [];
		let resolvePatch!: (response: Response) => void;
		vi.stubGlobal(
			'fetch',
			vi.fn(async (url: string, init?: RequestInit) => {
				if (url.includes('/auth/')) return new Response(null, { status: 204 });
				if (init?.method === 'PATCH')
					return new Promise<Response>((resolve) => (resolvePatch = resolve));
				if (init?.method === 'POST' && url.endsWith('/share')) {
					requests.push(url);
					return Response.json({
						...base,
						id: 'g2',
						title: 'Second gallery',
						share_token: 'T'.repeat(22),
						states: ['shared']
					});
				}
				return Response.json([]);
			})
		);
		const view = open();
		const replace = (gallery: Gallery) =>
			view.rerender({ data: { gallery, idleTimeoutMs: 2592000000 } as never });
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await user.click(screen.getByRole('switch', { name: 'Require access code' }));
		const input = screen.getByLabelText('6-digit access code') as HTMLInputElement;
		const edited = input.value;
		await replace({ ...base, title: 'Server refresh' });
		expect(input.value).toBe(edited);
		await user.click(screen.getByRole('button', { name: 'Share link' }));
		await user.click(await screen.findByRole('button', { name: 'Share' }));

		await replace({ ...base, id: 'g2', title: 'Second gallery' });
		expect(screen.queryByRole('dialog')).toBeNull();
		expect((screen.getByLabelText('Gallery title') as HTMLInputElement).value).toBe(
			'Second gallery'
		);
		resolvePatch(Response.json({ ...base, access_code: edited }));
		await tick();
		expect(requests).toEqual([]);

		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await user.click(screen.getByRole('button', { name: 'Share link' }));
		await user.click(await screen.findByRole('button', { name: 'Share' }));
		await waitFor(() => expect(requests).toEqual(['/api/v1/galleries/g2/share']));
	});

	it.each(['proof', 'final'] as const)('opens the source %s collection from the URL', (kind) => {
		state.page.url = new URL(`https://pp.test/dashboard/g1?photo=p2&kind=${kind}`);
		open();
		expect(screen.getByRole('tabpanel').id).toBe(kind === 'final' ? 'panel-final' : 'panel-photos');
	});

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

	it('previews in Design only, flushes pending edits and returns focus on close', async () => {
		const patches = stubApi();
		const user = userEvent.setup();
		open({ share_token: null, hero_id: 'p1' });

		await user.click(screen.getByRole('tab', { name: 'Contacts' }));
		expect(screen.queryByRole('button', { name: /preview/i })).toBeNull();
		await user.click(screen.getByRole('tab', { name: 'Design' }));
		await fireEvent.input(screen.getByLabelText('Gallery title'), {
			target: { value: 'Studio edits' }
		});
		expect(patches()).toHaveLength(0);

		const button = screen.getByRole('button', { name: 'Open full preview' });
		await user.click(button);
		expect(patches()).toHaveLength(1);
		expect(JSON.parse(patches()[0][1]?.body as string)).toEqual({
			title: 'Studio edits'
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
				if ((init?.method ?? 'GET') === 'GET') return Response.json([]);
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

	it('sends the chosen calendar day directly', async () => {
		const patches = stubApi();
		const user = userEvent.setup();
		open();
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await pickDay('Open until', 1, '4');
		await waitFor(() => expect(patches()).toHaveLength(1));
		const sent = JSON.parse(patches()[0][1]?.body as string).expires_at as string;
		const next = new Date();
		next.setDate(1);
		next.setMonth(next.getMonth() + 1);
		const pad = (value: number) => String(value).padStart(2, '0');
		expect(sent).toBe(`${next.getFullYear()}-${pad(next.getMonth() + 1)}-04`);
	});

	it('allows a past expiry, marks the gallery expired and clears it with null', async () => {
		const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
			if (url.includes('/auth/')) return new Response(null, { status: 204 });
			if ((init?.method ?? 'GET') === 'GET') return Response.json([]);
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
			.filter(([url, init]) => url.includes('/galleries/') && init?.method === 'PATCH')
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

	it('shows the stored expiry calendar date', async () => {
		const user = userEvent.setup();
		open({ expires_at: '2030-06-15' });
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

	it('has no axe violations across Photos, Design, Share, and Settings views', async () => {
		stubApi();
		const user = userEvent.setup();
		const { container } = open();
		expect((await axe(container)).violations).toEqual([]);
		await user.click(screen.getByRole('tab', { name: 'Design' }));
		expect((await axe(container)).violations).toEqual([]);
		await user.click(screen.getByRole('button', { name: 'Go to Share' }));
		expect((await axe(container)).violations).toEqual([]);
		await user.click(screen.getByRole('button', { name: 'Cancel' }));
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		expect((await axe(container)).violations).toEqual([]);
	});

	it.each(['before', 'after'])(
		'requires a new Share action when the access-code save fails %s confirmation',
		async (when) => {
			let resolvePatch!: (response: Response) => void;
			const shareCalls: string[] = [];

			vi.stubGlobal(
				'fetch',
				vi.fn(async (url: string, init?: RequestInit) => {
					if (url.includes('/auth/')) return new Response(null, { status: 204 });
					if (!init?.method || init.method === 'GET') return Response.json([]);
					if (url.endsWith('/share')) {
						shareCalls.push(url);
						return Response.json({
							...base,
							share_token: 'A'.repeat(22),
							access_code: '042819',
							states: ['shared']
						});
					}
					return new Promise<Response>((r) => (resolvePatch = r));
				})
			);

			const user = userEvent.setup();
			open();

			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			await user.click(screen.getByRole('switch', { name: 'Require access code' }));
			await waitFor(() => expect(resolvePatch).toBeDefined());

			await user.click(screen.getByRole('button', { name: 'Share link' }));
			const confirm = await screen.findByRole('button', { name: 'Share' });
			if (when === 'after') await user.click(confirm);

			expect(shareCalls).toHaveLength(0);
			resolvePatch(Response.json({ code: 'unavailable' }, { status: 503 }));

			await waitFor(() =>
				expect(
					screen.getByRole('switch', { name: 'Require access code' }).getAttribute('aria-checked')
				).toBe('false')
			);
			if (when === 'before') await user.click(confirm);
			await screen.findByText('Couldn’t share the gallery. Please try again.');
			expect(shareCalls).toHaveLength(0);
			await user.click(screen.getByRole('button', { name: 'Share' }));
			expect(shareCalls).toHaveLength(0);
			await user.click(screen.getByRole('button', { name: 'Cancel' }));
			await user.click(screen.getByRole('button', { name: 'Share link' }));
			await user.click(await screen.findByRole('button', { name: 'Share' }));
			await waitFor(() => expect(shareCalls).toHaveLength(1));
		}
	);

	it('waits for the access code selected when Share was opened to save before sharing', async () => {
		const events: string[] = [];
		let finishOkPatch!: () => void;
		const user = userEvent.setup();
		vi.stubGlobal(
			'fetch',
			vi.fn(async (url: string, init?: RequestInit) => {
				if (url.includes('/auth/')) return new Response(null, { status: 204 });
				if (!init?.method || init.method === 'GET') return Response.json([]);
				if (url.endsWith('/share')) {
					events.push('share');
					return Response.json({
						...base,
						share_token: 'B'.repeat(22),
						access_code: '042819',
						states: ['shared']
					});
				}
				await new Promise<void>((r) => (finishOkPatch = r));
				const patch = JSON.parse(init?.body as string);
				events.push('patch-done');
				return Response.json({ ...base, ...patch });
			})
		);

		open();
		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await user.click(screen.getByRole('switch', { name: 'Require access code' }));
		await waitFor(() => expect(finishOkPatch).toBeDefined());

		await user.click(screen.getByRole('button', { name: 'Share link' }));
		await user.click(await screen.findByRole('button', { name: 'Share' }));
		expect(events).toEqual([]);

		finishOkPatch();
		await waitFor(() => expect(events).toEqual(['patch-done', 'share']));
	});

	it('refuses to open Share or lock a stale code when the access-code input has a half-typed invalid draft', async () => {
		const shareCalls: string[] = [];
		vi.stubGlobal(
			'fetch',
			vi.fn(async (url: string, init?: RequestInit) => {
				if (url.includes('/auth/')) return new Response(null, { status: 204 });
				if (!init?.method || init.method === 'GET') return Response.json([]);
				if (url.endsWith('/share')) {
					shareCalls.push(url);
					return Response.json({ ...base, share_token: 'C'.repeat(22), states: ['shared'] });
				}
				const patch = JSON.parse(init?.body as string);
				return Response.json({
					...base,
					...patch
				});
			})
		);

		const user = userEvent.setup();
		open();

		await user.click(screen.getByRole('tab', { name: 'Settings' }));
		await user.click(screen.getByRole('switch', { name: 'Require access code' }));
		const codeInput = await screen.findByLabelText('6-digit access code');

		await user.clear(codeInput);
		await user.type(codeInput, '12');

		// Clicking Share link or top-bar Share while '12' is in the field must block and never revert+share the old code.
		await user.click(screen.getByRole('button', { name: 'Share link' }));
		expect(screen.queryByRole('button', { name: 'Share' })).toBeNull();
		expect((codeInput as HTMLInputElement).value).toBe('12');
		await user.click(screen.getByRole('button', { name: 'Go to Share' }));
		expect(screen.getByRole('tab', { name: 'Settings' }).getAttribute('aria-selected')).toBe(
			'true'
		);
		expect(shareCalls).toHaveLength(0);
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
				if (url.includes('/photos')) return new Response(JSON.stringify(photos));

				return new Response(null, { status: 204 });
			});
			vi.stubGlobal('fetch', fetcher);

			return () => fetcher.mock.calls.filter(([url]) => url.endsWith('/share'));
		};

		it('does not send if the access-code save fails while an invitation is being prepared', async () => {
			const user = userEvent.setup();
			let resolvePatch!: (response: Response) => void;
			const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
				if (url.includes('/auth/')) return new Response(null, { status: 204 });
				if (init?.method === 'PATCH')
					return new Promise<Response>((resolve) => (resolvePatch = resolve));
				if (url.endsWith('/invitations')) return new Response(null, { status: 503 });
				if (url.endsWith('/photos')) {
					return Response.json([photo('p1')]);
				}
				return Response.json([]);
			});
			vi.stubGlobal('fetch', fetcher);
			render(GalleryPage, {
				props: {
					data: {
						gallery: base,
						settings: { ...DEFAULT_SETTINGS, emailEnabled: true },
						idleTimeoutMs: 2592000000
					} as never
				}
			});
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			await user.click(screen.getByRole('switch', { name: 'Require access code' }));
			await waitFor(() => expect(resolvePatch).toBeDefined());
			await user.click(screen.getByRole('button', { name: 'Go to Share' }));
			await user.click(await screen.findByRole('button', { name: 'Add a recipient' }));
			await user.type(screen.getByLabelText('To'), 'camille@example.test{Enter}');
			const send = screen.getByRole('button', { name: 'Share' });
			await user.click(send);
			expect((send as HTMLButtonElement).disabled).toBe(true);
			resolvePatch(Response.json({ code: 'unavailable' }, { status: 503 }));
			await waitFor(() => expect(toasts.items).not.toEqual([]));
			await screen.findByRole('alert');
			expect(fetcher.mock.calls.some(([url]) => url.endsWith('/invitations'))).toBe(false);
		});

		it.each([false, true])(
			'does not revive an invitation after leaving Share during readiness (warning: %s)',
			async (warn) => {
				const user = userEvent.setup();
				const readiness = Promise.withResolvers<Response>();
				let deferNextRead = false;
				const invitations: string[] = [];
				const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
					if (url.endsWith('/invitations')) {
						invitations.push(url);
						return new Response(null, { status: 503 });
					}
					if (init?.method === 'PATCH')
						return Response.json({ code: 'unavailable' }, { status: 503 });
					if (url.endsWith('/photos')) {
						if (deferNextRead) {
							deferNextRead = false;
							return readiness.promise;
						}
						return Response.json([photo('p1')]);
					}
					return Response.json([]);
				});
				vi.stubGlobal('fetch', fetcher);
				render(GalleryPage, {
					props: {
						data: {
							gallery: base,
							settings: { ...DEFAULT_SETTINGS, emailEnabled: true },
							idleTimeoutMs: 2592000000
						} as never
					}
				});

				deferNextRead = true;
				await user.click(screen.getByRole('button', { name: 'Go to Share' }));
				await waitFor(() => expect(deferNextRead).toBe(false));

				await user.click(screen.getByRole('button', { name: 'Cancel' }));
				await user.click(screen.getByRole('tab', { name: 'Settings' }));
				if (warn) {
					const protection = screen.getByRole('switch', { name: 'Require access code' });
					await user.click(protection);
					await waitFor(() => expect(protection.getAttribute('aria-checked')).toBe('false'));
					expect(toasts.items).not.toEqual([]);
				}
				readiness.resolve(Response.json(warn ? [] : [photo('p1')]));
				await tick();
				expect(screen.queryByRole('dialog')).toBeNull();
				expect(invitations).toHaveLength(0);
			}
		);

		it.each(['stay', 'leave', 'return'])(
			'keeps released focal writes in order (%s)',
			async (navigation) => {
				const gallery = { ...base, id: `focal-${navigation}`, hero_id: 'p1' };
				const path = `/api/v1/galleries/${gallery.id}`;
				const first = Promise.withResolvers<Response>();
				const writes: { url: string; focal_point: Gallery['focal_point'] }[] = [];
				vi.stubGlobal(
					'fetch',
					vi.fn(async (url: string, init?: RequestInit) => {
						if (init?.method !== 'PATCH') {
							return Response.json(
								url === path ? { ...gallery, focal_point: { x: 70, y: 35 } } : []
							);
						}
						const { focal_point } = JSON.parse(init.body as string);
						writes.push({ url, focal_point });
						if (writes.length === 1) return first.promise;
						if (navigation === 'return' && writes.length === 2) {
							return Response.json({ ...gallery, focal_point });
						}
						return Response.json(
							{ code: 'unavailable' },
							{ status: navigation === 'leave' ? 401 : 503 }
						);
					})
				);
				const user = userEvent.setup();
				const view = open(gallery);
				await user.click(screen.getByRole('tab', { name: 'Design' }));
				const focalPad = () => {
					const pad = screen.getByRole('button', { name: 'Cover focal point' });
					vi.spyOn(pad, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, 0, 100, 100));
					return pad;
				};
				const release = async (pad: HTMLElement, x: number) => {
					await fireEvent.pointerDown(pad, { clientX: x, clientY: 35, pointerId: 1 });
					await fireEvent.pointerUp(pad, { clientX: x, clientY: 35, pointerId: 1 });
				};
				const pad = focalPad();
				await release(pad, 60);
				await release(pad, 70);
				const cover = () => screen.getByRole('img', { name: 'Gallery cover' });
				expect(cover().style.objectPosition).toBe('70% 35%');
				expect(writes).toEqual([{ url: path, focal_point: { x: 60, y: 35 } }]);

				if (navigation !== 'stay') {
					await fireEvent.pointerDown(pad, { clientX: 80, clientY: 45, pointerId: 1 });
					await view.rerender({
						data: {
							gallery: { ...base, id: 'g2', title: 'Second gallery', hero_id: 'p2' },
							idleTimeoutMs: 2592000000
						} as never
					});
					await user.click(screen.getByRole('tab', { name: 'Design' }));
				}
				let returningPad: HTMLButtonElement | undefined;
				let blockedWhileDraining = false;
				if (navigation === 'return') {
					await view.rerender({
						data: {
							gallery,
							idleTimeoutMs: 2592000000
						} as never
					});
					await user.click(screen.getByRole('tab', { name: 'Design' }));
					returningPad = screen.getByRole('button', { name: 'Cover focal point' });
					blockedWhileDraining = returningPad.disabled;
					await fireEvent.keyDown(returningPad, { key: 'ArrowRight' });
					expect(writes).toHaveLength(1);
				}
				first.resolve(Response.json({ ...gallery, focal_point: { x: 60, y: 35 } }));
				if (returningPad) {
					await waitFor(() => {
						expect(returningPad.disabled).toBe(false);
						expect(cover().style.objectPosition).toBe('70% 35%');
					});
					await fireEvent.keyDown(returningPad, { key: 'ArrowRight' });
				}
				await waitFor(() =>
					expect(writes).toEqual([
						{ url: path, focal_point: { x: 60, y: 35 } },
						{ url: path, focal_point: { x: 70, y: 35 } },
						...(navigation === 'return' ? [{ url: path, focal_point: { x: 71, y: 35 } }] : [])
					])
				);
				const expectedX = navigation === 'stay' ? 60 : navigation === 'leave' ? 50 : 70;
				await waitFor(() => expect(cover().style.objectPosition).toBe(`${expectedX}% 35%`));
				if (returningPad) expect(blockedWhileDraining).toBe(true);
				expect(goto).not.toHaveBeenCalled();
				if (navigation === 'leave') expect(toasts.items).toEqual([]);
			}
		);

		it('uses the confirmed focal point for rollback when the old write completes before mount', async () => {
			const first = Promise.withResolvers<Response>();
			const acknowledged = Promise.withResolvers<void>();
			const writes: Gallery['focal_point'][] = [];
			vi.stubGlobal(
				'fetch',
				vi.fn(async (url: string, init?: RequestInit) => {
					if (url !== '/api/v1/galleries/g1') return Response.json([]);
					if (init?.method === 'PATCH') {
						writes.push(JSON.parse(init.body as string).focal_point);
						return writes.length === 1
							? first.promise
							: Response.json({ code: 'unavailable' }, { status: 503 });
					}
					return Response.json({
						...base,
						title: 'Separate server title',
						hero_id: 'p1',
						focal_point: { x: 73, y: 35 }
					});
				})
			);
			const user = userEvent.setup();
			const view = open({ hero_id: 'p1' });
			await user.click(screen.getByRole('tab', { name: 'Design' }));
			await fireEvent.keyDown(screen.getByRole('button', { name: 'Cover focal point' }), {
				key: 'ArrowRight',
				shiftKey: true
			});
			expect(writes).toEqual([{ x: 60, y: 35 }]);
			await view.rerender({
				data: {
					gallery: { ...base, id: 'g2', title: 'Second gallery', hero_id: 'p2' },
					idleTimeoutMs: 2592000000
				} as never
			});

			// The returning route has already read its gallery snapshot; the old PATCH settles before mount.
			const returningData = { gallery: { ...base, hero_id: 'p1' }, idleTimeoutMs: 2592000000 };
			const ack = Response.json({});
			vi.spyOn(ack, 'json').mockImplementation(async () => {
				acknowledged.resolve();
				return { ...base, focal_point: { x: 60, y: 35 } };
			});
			first.resolve(ack);
			await acknowledged.promise;
			await user.click(screen.getByRole('tab', { name: 'Design' }));
			await view.rerender({ data: returningData as never });
			await user.click(screen.getByRole('tab', { name: 'Design' }));
			const pad = screen.getByRole('button', { name: 'Cover focal point' }) as HTMLButtonElement;
			await waitFor(() => expect(pad.disabled).toBe(false));
			await fireEvent.keyDown(pad, { key: 'ArrowRight' });
			await waitFor(() =>
				expect(writes).toEqual([
					{ x: 60, y: 35 },
					{ x: 74, y: 35 }
				])
			);
			await waitFor(() =>
				expect(screen.getByRole('img', { name: 'Gallery cover' }).style.objectPosition).toBe(
					'73% 35%'
				)
			);
			expect((screen.getByLabelText('Gallery title') as HTMLInputElement).value).toBe(base.title);
		});

		it('does not apply an obsolete Publish completion or redirect a different gallery', async () => {
			let finishDelivery!: (response: Response) => void;
			const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
				if (init?.method === 'POST' && url.endsWith('/deliver'))
					return new Promise<Response>((resolve) => (finishDelivery = resolve));
				return Response.json([]);
			});
			vi.stubGlobal('fetch', fetcher);
			const user = userEvent.setup();
			const view = open({ share_token: 'T'.repeat(22), states: ['shared'] });
			await user.click(screen.getByRole('button', { name: 'Go to Deliver final photos' }));
			await user.click(await screen.findByRole('button', { name: 'Deliver' }));
			expect(fetcher.mock.calls.filter(([url]) => url.endsWith('/deliver'))).toEqual([
				['/api/v1/galleries/g1/deliver', expect.anything()]
			]);
			await view.rerender({
				data: {
					gallery: { ...base, id: 'g2', title: 'Second gallery' },
					idleTimeoutMs: 2592000000
				} as never
			});
			expect(screen.queryByRole('dialog')).toBeNull();
			finishDelivery(Response.json({ code: 'unauthorized' }, { status: 401 }));
			await tick();
			expect(screen.queryByRole('button', { name: 'Go to Deliver final photos' })).toBeNull();
			expect(screen.queryByRole('button', { name: 'Deliver' })).toBeNull();
			expect(goto).not.toHaveBeenCalled();
		});

		it('shows the title and the draft status', () => {
			open();
			expect(screen.getByText('Draft')).toBeTruthy();
			expect(screen.queryByText('Created')).toBeNull();
			expect(screen.getByLabelText('Gallery title')).toBeTruthy();
		});

		it('queues the chosen stage only on confirmation and returns to Photos', async () => {
			const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
				if (url.includes('/auth/')) return new Response(null, { status: 204 });
				if (url.endsWith('/invitations') && init?.method === 'POST') {
					return Response.json({
						gallery: { ...base, share_token: 'C'.repeat(22), states: ['shared'] },
						queued_count: 1
					});
				}
				if (url.endsWith('/photos')) return Response.json([photo('p1')]);
				return Response.json([]);
			});
			vi.stubGlobal('fetch', fetcher);
			const user = userEvent.setup();
			render(GalleryPage, {
				props: {
					data: {
						gallery: { ...base },
						settings: {
							studioName: 'Atelier',
							logo: null,
							mark: 'name',
							pattern: 'repeat',
							position: 8,
							size: 4,
							signature: 'name',
							signatureSize: 4,
							proofQuality: 'medium',
							emailEnabled: true
						},
						idleTimeoutMs: 2592000000
					} as never
				}
			});

			await user.click(screen.getByRole('button', { name: 'Go to Share' }));
			const share = within(screen.getByRole('dialog'));
			expect(await share.findByText('No recipient')).toBeTruthy();
			expect((share.getByRole('button', { name: 'Share' }) as HTMLButtonElement).disabled).toBe(
				false
			);
			await user.click(share.getByRole('button', { name: 'Add a recipient' }));
			await user.type(share.getByLabelText('To'), 'camille@example.com{Enter}');
			await user.click(share.getByRole('button', { name: 'Share' }));

			await waitFor(() => expect(screen.getByText('Shared')).toBeTruthy());
			expect(toasts.items.some((item) => item.message.includes('Invitation queued'))).toBe(true);
			expect(screen.getByRole('tab', { name: /^Photos/ }).getAttribute('aria-selected')).toBe(
				'true'
			);
			expect(fetcher.mock.calls.filter(([url]) => url.endsWith('/share'))).toHaveLength(0);
		});

		it('uses the saved title when confirmation waits for a pending title change', async () => {
			const save = Promise.withResolvers<Response>();
			let subject: string | undefined;
			vi.stubGlobal('fetch', async (url: string, init?: RequestInit) => {
				if (init?.method === 'PATCH') return save.promise;
				if (url.endsWith('/invitations')) {
					subject = JSON.parse(String(init?.body)).subject;
					return Response.json({
						gallery: { ...base, share_token: 'T'.repeat(22), states: ['shared'] },
						queued_count: 1
					});
				}
				return Response.json([]);
			});
			const user = userEvent.setup();
			render(GalleryPage, {
				props: {
					data: {
						gallery: base,
						settings: { ...DEFAULT_SETTINGS, emailEnabled: true },
						idleTimeoutMs: 2592000000
					} as never
				}
			});
			const title = screen.getByLabelText('Gallery title');
			await user.clear(title);
			await user.type(title, 'Portraits');
			await user.click(screen.getByRole('button', { name: 'Go to Share' }));
			await user.click(await screen.findByRole('button', { name: 'Add a recipient' }));
			await user.type(screen.getByLabelText('To'), 'bob@example.test{Enter}');
			await user.click(screen.getByRole('button', { name: 'Share' }));
			expect(subject).toBeUndefined();
			save.resolve(Response.json({ ...base, title: 'Portraits' }));
			await waitFor(() => expect(subject).toContain('Portraits'));
		});

		it('keeps the edited envelope when reconciliation finds the same draft', async () => {
			const submissions: unknown[] = [];
			const contact = {
				id: 'c1',
				link_id: 'link-a',
				name: 'Alice',
				email: 'alice@example.test',
				phone: null,
				notes: null,
				galleries: [],
				created_at: base.created_at,
				updated_at: base.updated_at
			};
			vi.stubGlobal('fetch', async (url: string, init?: RequestInit) => {
				if (url.endsWith('/invitations')) {
					submissions.push(JSON.parse(String(init?.body)));
					if (submissions.length === 1) throw new TypeError('connection closed');
					return Response.json({ code: 'too_many_recipients' }, { status: 400 });
				}
				if (url.endsWith('/contacts')) return Response.json([contact]);
				return Response.json(url.endsWith('/g1') ? base : []);
			});
			const user = userEvent.setup();
			render(GalleryPage, {
				props: {
					data: {
						gallery: base,
						settings: { ...DEFAULT_SETTINGS, emailEnabled: true },
						idleTimeoutMs: 2592000000
					} as never
				}
			});

			await user.click(screen.getByRole('button', { name: 'Go to Share' }));
			await user.click(await screen.findByRole('button', { name: 'Customize' }));
			await user.type(screen.getByLabelText('To'), 'bob@example.test{Enter}');
			await user.click(screen.getByRole('button', { name: 'Remove alice@example.test' }));
			const subject = screen.getByRole('textbox', { name: 'Subject' });
			await user.clear(subject);
			await user.type(subject, 'For Bob');
			const message = screen.getByRole('textbox', { name: 'Personal message' });
			await user.clear(message);
			await user.type(message, 'Hello Bob');
			await user.click(screen.getByRole('button', { name: 'Share' }));
			await screen.findByRole('alert');
			await user.click(screen.getByRole('button', { name: 'Share' }));
			await waitFor(() => expect(submissions).toHaveLength(2));
			expect(submissions[1]).toMatchObject({
				to: [{ email: 'bob@example.test', name: null }],
				subject: 'For Bob',
				message: 'Hello Bob'
			});
			await waitFor(() =>
				expect(screen.getByRole('alert').textContent).toMatch(/Too many recipients/)
			);
		});
		it('reconciles a lost invitation response without sending twice or claiming delivery by email', async () => {
			const shared = { ...base, states: ['shared'], share_token: 'C'.repeat(22) };
			const fetcher = vi.fn(async (url: string) => {
				if (url.endsWith('/invitations')) throw new TypeError('connection closed');
				return Response.json(url.endsWith('/g1') ? shared : []);
			});
			vi.stubGlobal('fetch', fetcher);
			const user = userEvent.setup();
			render(GalleryPage, {
				props: {
					data: {
						gallery: base,
						settings: { ...DEFAULT_SETTINGS, emailEnabled: true },
						idleTimeoutMs: 2592000000
					} as never
				}
			});
			await user.click(screen.getByRole('button', { name: 'Go to Share' }));
			await user.click(await screen.findByRole('button', { name: 'Add a recipient' }));
			await user.type(screen.getByLabelText('To'), 'camille@example.test{Enter}');
			await user.click(screen.getByRole('button', { name: 'Share' }));
			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
			expect(fetcher.mock.calls.filter(([url]) => url.endsWith('/invitations'))).toHaveLength(1);
			expect(toasts.items.some((item) => item.message.includes('queued'))).toBe(false);
			expect(screen.queryByRole('button', { name: 'Go to Share' })).toBeNull();
		});

		it('shares via Settings → Share link only after confirming in the dialog', async () => {
			const shares = stubShareFlow();
			const user = userEvent.setup();
			open();

			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			await user.click(screen.getByRole('button', { name: 'Share link' }));
			expect(await screen.findByRole('dialog', { name: 'Share 1 photo?' })).toBeTruthy();
			expect(shares()).toHaveLength(0);

			await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Share' }));
			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
			expect(shares()).toHaveLength(1);

			expect(screen.getByText('Shared')).toBeTruthy();
			expect(screen.queryByRole('button', { name: 'Share' })).toBeNull();
			expect(screen.getByLabelText('Client link')).toBeTruthy();
		});

		it('does not share when the Share link dialog is dismissed', async () => {
			const shares = stubShareFlow();
			const user = userEvent.setup();
			open();
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			await user.click(screen.getByRole('button', { name: 'Share link' }));
			await user.click(await screen.findByRole('button', { name: 'Cancel' }));
			expect(shares()).toHaveLength(0);
			expect(screen.getByRole('button', { name: 'Go to Share' })).toBeTruthy();
		});

		it('warns instead of blocking when sharing link with no photos', async () => {
			stubShareFlow([]);
			const user = userEvent.setup();
			open();
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			await user.click(screen.getByRole('button', { name: 'Share link' }));
			expect(await screen.findByRole('dialog', { name: 'Share with no photos?' })).toBeTruthy();
			expect(screen.getByText('Clients see an empty gallery until you upload.')).toBeTruthy();
			expect(screen.getByRole('button', { name: 'Share' })).toBeTruthy();
		});

		it('mentions photos still processing or failed when sharing link', async () => {
			stubShareFlow([photo('p1'), photo('p2', 'processing'), photo('p3', { failed: 'bad' })]);
			const user = userEvent.setup();
			open();
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			await user.click(screen.getByRole('button', { name: 'Share link' }));
			expect(await screen.findByText('1 is still uploading and 1 failed.')).toBeTruthy();
			expect(screen.getByRole('button', { name: 'Share' })).toBeTruthy();
		});

		it('names the open-until date only when one is set when sharing link', async () => {
			stubShareFlow();
			const user = userEvent.setup();
			open({ expires_at: '2026-09-12' });
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			await user.click(screen.getByRole('button', { name: 'Share link' }));
			expect(await screen.findByText(/pick favorites until 12 Sep 2026\./)).toBeTruthy();
		});

		it('starts from the link row and an enabled Publish when already shared', async () => {
			stubShareFlow();
			const user = userEvent.setup();
			open({ share_token: 'B'.repeat(22), states: ['shared'] });
			expect(screen.getByText('Shared')).toBeTruthy();
			expect(screen.queryByRole('button', { name: 'Deliver' })).toBeNull();
			await user.click(screen.getByRole('button', { name: 'Go to Deliver final photos' }));
			const share = within(screen.getByRole('dialog'));
			expect((screen.getByRole('button', { name: 'Deliver' }) as HTMLButtonElement).disabled).toBe(
				false
			);
			await user.click(share.getByRole('button', { name: 'Cancel' }));
			expect(document.activeElement).toBe(
				screen.getByRole('button', { name: 'Go to Deliver final photos' })
			);
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			expect(screen.getByLabelText('Client link')).toBeTruthy();
		});

		it('names the gallery in the Publish text, plural when it has an ampersand', async () => {
			stubShareFlow();
			const user = userEvent.setup();
			open({ share_token: 'B'.repeat(22), title: 'Camille & Hugo' });
			await user.click(screen.getByRole('button', { name: 'Go to Deliver final photos' }));
			expect(await screen.findByText(/Camille & Hugo see them at the same link/)).toBeTruthy();
		});

		it('publishes from Share without awaiting SMTP or sending mail', async () => {
			const fetcher = vi.fn(async (url: string) =>
				url.endsWith('/deliver')
					? new Response(
							JSON.stringify({
								...base,
								share_token: 'B'.repeat(22),
								states: ['delivered', 'expired']
							})
						)
					: Response.json(url.endsWith('/photos?finals=true') ? [] : [photo('p1', 'processing')])
			);
			vi.stubGlobal('fetch', fetcher);
			const user = userEvent.setup();
			open({ share_token: 'B'.repeat(22), states: ['shared', 'expired'] });
			await user.click(screen.getByRole('button', { name: 'Go to Deliver final photos' }));
			expect(screen.getByText('This gallery has expired')).toBeTruthy();
			const publish = await screen.findByRole('dialog', { name: 'Deliver with no final photos?' });
			expect(within(publish).queryByText(/still uploading/)).toBeNull();
			expect(fetcher.mock.calls.filter(([url]) => url.endsWith('/deliver'))).toHaveLength(0);
			await user.click(within(publish).getByRole('button', { name: 'Deliver' }));
			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
			expect(screen.getByText('Expired', { selector: '[data-slot="status-chip"]' })).toBeTruthy();
			expect(screen.queryByRole('button', { name: 'Deliver' })).toBeNull();
			expect(
				screen.getByRole('tab', { name: /^Favorites/ }).querySelector('[aria-label="Locked"]')
			).toBeNull();
			expect(toasts.items.some((item) => item.message.includes('queued'))).toBe(false);
			expect(fetcher.mock.calls.filter(([url]) => url.endsWith('/g1/deliver'))).toHaveLength(1);
			expect(fetcher.mock.calls.filter(([url]) => url.endsWith('/invitations'))).toHaveLength(0);
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			expect((screen.getByLabelText('Client link') as HTMLInputElement).value).toBe(
				`https://pp.test/g/${'B'.repeat(22)}`
			);
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
			await user.click(screen.getByRole('button', { name: 'Go to Deliver final photos' }));
			await user.click(
				within(await screen.findByRole('dialog', { name: 'Deliver 1 final photo?' })).getByRole(
					'button',
					{ name: 'Deliver' }
				)
			);
			expect((await screen.findByRole('alert')).textContent).toMatch(/Couldn’t publish/);
		});

		it('reports a failed share and keeps the dialog open to try again', async () => {
			vi.stubGlobal(
				'fetch',
				vi.fn(async (url: string) =>
					url.includes('/photos') ? new Response('[]') : new Response('{}', { status: 500 })
				)
			);
			const user = userEvent.setup();
			open();
			await user.click(screen.getByRole('tab', { name: 'Settings' }));
			await user.click(screen.getByRole('button', { name: 'Share link' }));
			await user.click(await screen.findByRole('button', { name: 'Share' }));
			expect((await screen.findByRole('alert')).textContent).toMatch(/Couldn’t share/);
			expect(screen.getByRole('button', { name: 'Share' })).toBeTruthy();
		});
	});

	describe('restoring', () => {
		const archived: Partial<Gallery> = {
			states: ['shared', 'archived'],
			expires_at: '2026-09-07'
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
				'Retouched',
				'Design',
				'Activity',
				'Comments',
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
			await user.click(screen.getByRole('tab', { name: /^Retouched/ }));
			expect(screen.getByRole('tab', { name: /^Retouched/ }).getAttribute('aria-selected')).toBe(
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
			expect((screen.getByRole('tab', { name: 'Retouched' }) as HTMLButtonElement).disabled).toBe(
				true
			);
			expect((screen.getByRole('tab', { name: 'Design' }) as HTMLButtonElement).disabled).toBe(
				false
			);
			expect((screen.getByRole('tab', { name: 'Settings' }) as HTMLButtonElement).disabled).toBe(
				false
			);
		});
	});
});
