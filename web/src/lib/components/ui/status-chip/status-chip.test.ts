import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import StatusChip, { type GalleryStatus } from './status-chip.svelte';

describe('StatusChip', () => {
	it.each<[GalleryStatus, string]>([
		['draft', 'Draft'],
		['shared', 'Shared'],
		['delivered', 'Delivered'],
		['expired', 'Expired'],
		['archived', 'Archived']
	])('labels %s', async (status, label) => {
		const { container } = render(StatusChip, { status });

		expect(screen.getByText(label)).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('shows its colour dot only when asked', () => {
		const { container, rerender } = render(StatusChip, { status: 'shared' });

		expect(container.querySelector('[data-slot="status-chip-dot"]')).toBeNull();
		rerender({ status: 'shared', dot: true });
		expect(container.querySelector('[data-slot="status-chip-dot"]')).toBeTruthy();
	});

	it('stays legible over a photo in its cover form', async () => {
		const { container } = render(StatusChip, { status: 'delivered', dot: true, cover: true });

		expect(screen.getByText('Delivered').className).toMatch(/bg-panel\/90/);
		expect((await axe(container)).violations).toEqual([]);
	});
});
