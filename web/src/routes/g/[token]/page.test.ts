import { render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import Page from './+page.svelte';

const gallery = {
	title: 'Wedding',
	layout: 'grid' as const,
	expires_at: null,
	included_photos: null,
	delivered: false,
	hero_id: null
};

describe('client gallery page', () => {
	const fetcher = vi.fn();

	beforeEach(() => {
		fetcher.mockReset();
		fetcher.mockResolvedValue(new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetcher);
	});
	afterEach(() => vi.unstubAllGlobals());

	it('reports one open when a gallery is shown', () => {
		render(Page, { data: { gallery, proofs: [], favorites: [] }, params: { token: 'tok' } });

		expect(fetcher).toHaveBeenCalledTimes(1);
		expect(fetcher.mock.calls[0][0]).toBe('/api/v1/galleries/shared/tok/open');
	});

	it('reports nothing for an unavailable link', () => {
		render(Page, { data: { gallery: null, proofs: [], favorites: [] }, params: { token: 'tok' } });

		expect(fetcher).not.toHaveBeenCalled();
	});

	it('still shows the gallery when the report fails', () => {
		fetcher.mockRejectedValue(new TypeError('offline'));

		const { getByRole } = render(Page, {
			data: { gallery, proofs: [], favorites: [] },
			params: { token: 'tok' }
		});

		expect(getByRole('heading', { level: 1 }).textContent).toContain('Wedding');
	});
});
