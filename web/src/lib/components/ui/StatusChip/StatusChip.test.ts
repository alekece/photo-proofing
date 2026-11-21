import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import StatusChip, { type GalleryStatus } from './StatusChip.svelte';

describe('StatusChip', () => {
	describe.each([false, true])('workspace cover=%s', (cover) => {
		it.each([
			['draft', 'border-border-strong', 'bg-border', 'text-muted-foreground'],
			['shared', 'border-workspace-green', 'bg-inverse', 'text-workspace-green'],
			['delivered', 'border-workspace-green', 'bg-workspace-green', 'text-inverse'],
			['archived', 'border-foreground', 'bg-foreground', 'text-inverse']
		] as const)('uses the workspace %s treatment', (status, ...classes) => {
			const { container } = render(StatusChip, { status, cover, workspace: true });
			const chip = container.querySelector('[data-slot="status-chip"]')!;
			for (const name of classes) expect(chip.classList.contains(name)).toBe(true);
		});
	});

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
