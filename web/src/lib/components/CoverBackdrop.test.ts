import { render, screen } from '@testing-library/svelte';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';

import CoverBackdrop from './CoverBackdrop.svelte';

describe('CoverBackdrop', () => {
	it('prints the film-edge date in full mode and omits it in compact mode', async () => {
		const { container, rerender } = render(CoverBackdrop, {
			title: 'Camille & Lucas',
			edge: '12 Jun 2026'
		});

		expect(container.textContent).toContain('CL');
		expect(container.textContent).toContain('12 Jun 2026');

		await rerender({ title: 'Camille & Lucas', edge: '12 Jun 2026', compact: true });

		expect(container.textContent).toContain('CL');
		expect(container.textContent).not.toContain('12 Jun 2026');
		expect((await axe(container)).violations).toEqual([]);
	});

	it('falls back cleanly for an untitled gallery in compact mode', async () => {
		const { container } = render(CoverBackdrop, { title: null, compact: true });

		expect(screen.getByTestId('no-hero')).toBeTruthy();
		expect(container.textContent?.trim().length).toBeGreaterThan(0);
		expect((await axe(container)).violations).toEqual([]);
	});
});
