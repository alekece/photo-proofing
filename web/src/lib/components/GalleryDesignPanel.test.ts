import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';

import GalleryDesignPanel from './GalleryDesignPanel.svelte';

describe('GalleryDesignPanel', () => {
	it('renders the Design heading and saves the chosen layout', async () => {
		const onsave = vi.fn().mockResolvedValue(true);
		const user = userEvent.setup();
		const { container } = render(GalleryDesignPanel, { layout: 'grid', onsave });

		expect(screen.getByRole('heading', { name: 'Design' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Staggered columns' }));

		expect(onsave).toHaveBeenCalledWith({ layout: 'masonry' });
		expect((await axe(container)).violations).toEqual([]);
	});

	it('reverts a refused layout change', async () => {
		const onsave = vi.fn().mockResolvedValue(false);
		const user = userEvent.setup();

		render(GalleryDesignPanel, { layout: 'grid', onsave });

		const staggered = screen.getByRole('button', { name: 'Staggered columns' });
		const rows = screen.getByRole('button', { name: 'Even rows' });

		await user.click(staggered);

		await vi.waitFor(() => expect(rows.getAttribute('aria-pressed')).toBe('true'));
		expect(staggered.getAttribute('aria-pressed')).toBe('false');
	});
});
