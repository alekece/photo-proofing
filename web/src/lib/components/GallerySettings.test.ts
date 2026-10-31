import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type { Gallery } from '#lib/gallery/api.js';

import GallerySettings from './GallerySettings.svelte';

const gallery = {
	layout: 'grid',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false
} as Gallery;

function setup() {
	const onsave = vi.fn();
	const schedule = vi.fn();
	const cancel = vi.fn();

	render(GallerySettings, { gallery, failed: {}, onsave, schedule, cancel });

	return { onsave, schedule, cancel, user: userEvent.setup() };
}

describe('GallerySettings', () => {
	it('saves the layout and watermark as patches', async () => {
		const { onsave, user } = setup();

		await user.click(screen.getByLabelText('Masonry'));
		await user.click(screen.getByLabelText('Watermark'));

		expect(onsave).toHaveBeenNthCalledWith(1, { layout: 'masonry' });
		expect(onsave).toHaveBeenNthCalledWith(2, { watermark_enabled: true });
	});

	it('debounces included photos through the page and saves on leaving', async () => {
		const { onsave, schedule, cancel, user } = setup();
		const included = screen.getByLabelText('Included photos');

		await user.type(included, '5');
		expect(schedule).toHaveBeenCalledWith('included_photos', expect.any(Function));
		expect(onsave).not.toHaveBeenCalled();

		await user.tab();
		expect(cancel).toHaveBeenCalledWith('included_photos');
		expect(onsave).toHaveBeenCalledWith({ included_photos: 5 });
	});

	it('does not save an invalid included count', async () => {
		const { onsave, user } = setup();

		await user.type(screen.getByLabelText('Included photos'), '0');
		await user.tab();

		expect(onsave).not.toHaveBeenCalled();
		expect(screen.getByText('Enter a whole number of at least 1.')).toBeTruthy();
	});
});
