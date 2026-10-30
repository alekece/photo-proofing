import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ afterNavigate: vi.fn(), beforeNavigate: vi.fn(), goto: vi.fn() }));
import { goto } from '$app/navigation';
import type { Gallery } from '#lib/gallery/api.js';
import GalleryPage from './+page.svelte';

const base: Gallery = {
	id: 'g1',
	title: 'Wedding',
	layout: 'grid',
	expires_at: null,
	included_photos: 5,
	watermark_enabled: false,
	created_at: '2026-01-01T10:00:00Z',
	updated_at: '2026-01-02T10:00:00Z'
};
const open = (over: Partial<Gallery> = {}) =>
	render(GalleryPage, {
		props: { data: { gallery: { ...base, ...over }, idleTimeoutMs: 2592000000 } as never }
	});
// Echoes the patch back like the API does; renew calls (auth API) just succeed.
const stubApi = (status = 200) => {
	const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
		if (url.includes('/auth/')) return new Response(null, { status: 204 });
		if (status !== 200) return new Response(JSON.stringify({ error: 'nope' }), { status });
		return new Response(JSON.stringify({ ...base, ...JSON.parse(init?.body as string) }));
	});
	vi.stubGlobal('fetch', fetcher);
	return () => fetcher.mock.calls.filter(([url]) => url.includes('/galleries/'));
};

beforeEach(() => vi.mocked(goto).mockClear());
afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe('gallery page', () => {
	it('sends only the changed field and shows Saved', async () => {
		const patches = stubApi();
		open();
		await userEvent.setup().click(screen.getByLabelText('Masonry'));
		expect(await screen.findByText('Saved')).toBeTruthy();
		expect(patches()).toHaveLength(1);
		const [url, init] = patches()[0];
		expect(url).toBe('/api/v1/galleries/g1');
		expect(init?.method).toBe('PATCH');
		expect(JSON.parse(init?.body as string)).toEqual({ layout: 'masonry' });
		expect(init?.keepalive).toBe(true);
	});

	it('saves the title on blur and reverts an emptied title', async () => {
		const patches = stubApi();
		const user = userEvent.setup();
		open();
		const title = screen.getByLabelText('Gallery title') as HTMLInputElement;
		await user.clear(title);
		await user.tab();
		expect(title.value).toBe('Wedding');
		expect(patches()).toHaveLength(0);
		await user.type(title, ' II');
		await user.tab();
		await waitFor(() => expect(patches()).toHaveLength(1));
		expect(JSON.parse(patches()[0][1]?.body as string)).toEqual({ title: 'Wedding II' });
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

	it('shows an error with Retry that resends the attempted value', async () => {
		const patches = stubApi(500);
		const user = userEvent.setup();
		open();
		await user.click(screen.getByLabelText('Watermark'));
		expect(await screen.findByText('Couldn’t save')).toBeTruthy();
		expect((screen.getByLabelText('Watermark') as HTMLInputElement).checked).toBe(true);
		stubApi();
		await user.click(screen.getByRole('button', { name: 'Retry' }));
		expect(await screen.findByText('Saved')).toBeTruthy();
		expect(patches()).toHaveLength(1);
	});

	it('keeps a failed title visible when a later save succeeds elsewhere', async () => {
		const patches = stubApi(500);
		const user = userEvent.setup();
		open();
		await user.type(screen.getByLabelText('Gallery title'), ' II');
		await user.click(screen.getByLabelText('Watermark'));
		await waitFor(() => expect(patches()).toHaveLength(2));
		stubApi();
		await user.click(screen.getByLabelText('Masonry'));
		await waitFor(() => expect(screen.queryByText('Saving…')).toBeNull());
		expect(screen.queryByText('Saved')).toBeNull();
		expect(screen.getByText('Couldn’t save')).toBeTruthy();
		const retried = stubApi();
		await user.click(screen.getByRole('button', { name: 'Retry' }));
		expect(await screen.findByText('Saved')).toBeTruthy();
		expect(retried().map(([, init]) => JSON.parse(init?.body as string))).toEqual([
			{ title: 'Wedding II' },
			{ watermark_enabled: true }
		]);
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
		await user.tab();
		expect(title.value).toBe('Wedding');
		expect(screen.queryByText('Couldn’t save')).toBeNull();

		await user.type(title, '?');
		await user.tab();
		expect(await screen.findByText('Couldn’t save')).toBeTruthy();
		const retried = stubApi();
		await user.click(title);
		await user.click(screen.getByRole('button', { name: 'Retry' }));
		expect(await screen.findByText('Saved')).toBeTruthy();
		expect(title.value).toBe('Wedding?');
		expect(retried()).toHaveLength(1);
	});

	it('redirects to sign-in when the session expired', async () => {
		stubApi(401);
		open();
		await userEvent.setup().click(screen.getByLabelText('Watermark'));
		await waitFor(() => expect(goto).toHaveBeenCalledWith('/login?expired=1', expect.anything()));
	});

	it('sends the end of the chosen day in local time', async () => {
		const patches = stubApi();
		open();
		await fireEvent.change(screen.getByLabelText('Expiry'), { target: { value: '2099-03-04' } });
		await waitFor(() => expect(patches()).toHaveLength(1));
		const sent = new Date(JSON.parse(patches()[0][1]?.body as string).expires_at);
		expect([sent.getFullYear(), sent.getMonth(), sent.getDate()]).toEqual([2099, 2, 4]);
		expect([sent.getHours(), sent.getMinutes(), sent.getSeconds()]).toEqual([23, 59, 59]);
	});

	it('warns about, but allows, a past expiry and clears it with null', async () => {
		const patches = stubApi();
		const user = userEvent.setup();
		open();
		await fireEvent.change(screen.getByLabelText('Expiry'), { target: { value: '2020-01-01' } });
		expect(await screen.findByText(/in the past/)).toBeTruthy();
		await waitFor(() => expect(patches()).toHaveLength(1));
		await user.click(screen.getByRole('button', { name: 'Clear' }));
		await waitFor(() => expect(patches()).toHaveLength(2));
		expect(JSON.parse(patches()[1][1]?.body as string)).toEqual({ expires_at: null });
	});

	it('shows the stored expiry as a local date', () => {
		const stored = new Date(2030, 5, 15, 23, 59, 59).toISOString();
		open({ expires_at: stored });
		expect((screen.getByLabelText('Expiry') as HTMLInputElement).value).toBe('2030-06-15');
	});

	it('sends null when included photos is cleared and rejects zero', async () => {
		const patches = stubApi();
		const user = userEvent.setup();
		open();
		const input = screen.getByLabelText('Included photos');
		await user.clear(input);
		await user.tab();
		await waitFor(() => expect(patches()).toHaveLength(1));
		expect(JSON.parse(patches()[0][1]?.body as string)).toEqual({ included_photos: null });
		await user.type(input, '0');
		await user.tab();
		expect(screen.getByText(/at least 1/)).toBeTruthy();
		expect(patches()).toHaveLength(1);
	});

	it('has no axe violations', async () => {
		const { container } = open();
		expect((await axe(container)).violations).toEqual([]);
	});
});
