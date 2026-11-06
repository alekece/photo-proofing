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
});
