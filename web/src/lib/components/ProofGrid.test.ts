import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import ProofGrid from './ProofGrid.svelte';

const proofs = [1, 2].map((n) => ({
	id: `p${n}`,
	alt: `Proof ${n}`,
	previewUrl: `data:,${n}`,
	thumbnailUrl: `data:,t${n}`,
	width: 300,
	height: 200
}));

describe('ProofGrid', () => {
	it('lists each proof as a keyboard-operable button', async () => {
		const onopen = vi.fn();
		const user = userEvent.setup();
		render(ProofGrid, { proofs, layout: 'grid', onopen });
		expect(screen.getAllByRole('listitem')).toHaveLength(2);
		await user.tab();
		await user.tab();
		await user.keyboard('{Enter}');
		expect(onopen).toHaveBeenCalledWith(1);
	});

	it('renders an empty message when there are no proofs', () => {
		render(ProofGrid, { proofs: [], layout: 'grid', onopen: () => {} });
		expect(screen.getByText(/no proofs/i)).toBeTruthy();
	});

	it.each(['grid', 'masonry'] as const)('renders every proof in order for %s', (layout) => {
		render(ProofGrid, { proofs, layout, onopen: () => {} });
		const list = screen.getByRole('list', { name: 'Proofs' });
		expect(list.getAttribute('data-layout')).toBe(layout);
		expect(list.getAttribute('data-density')).toBe('compact');
		expect(list.getAttribute('data-spacing')).toBe('compact');
		expect(screen.getAllByRole('img').map((img) => img.getAttribute('alt'))).toEqual([
			'Proof 1',
			'Proof 2'
		]);
	});

	it('applies container-query density and spacing classes for grid and masonry', () => {
		const gridView = render(ProofGrid, {
			proofs,
			layout: 'grid',
			density: 'large',
			spacing: 'generous',
			onopen: () => {}
		});
		const gridList = screen.getByRole('list', { name: 'Proofs' });
		expect(gridList.getAttribute('data-density')).toBe('large');
		expect(gridList.getAttribute('data-spacing')).toBe('generous');
		expect(gridList.className).toContain(
			'grid-cols-[repeat(auto-fill,minmax(min(100%,22rem),1fr))]'
		);
		expect(gridList.className).toContain('@max-[700px]:grid-cols-1');
		expect(gridList.className).toContain('gap-5');
		expect(gridList.className).toContain('@max-[700px]:gap-3.5');
		gridView.unmount();

		const masonryView = render(ProofGrid, {
			proofs,
			layout: 'masonry',
			density: 'large',
			spacing: 'generous',
			onopen: () => {}
		});
		const masonryList = screen.getByRole('list', { name: 'Proofs' });
		expect(masonryList.getAttribute('data-density')).toBe('large');
		expect(masonryList.getAttribute('data-spacing')).toBe('generous');
		expect(masonryList.className).toContain('[columns:2_26rem]');
		expect(masonryList.className).toContain('@max-[700px]:columns-1');
		expect(masonryList.className).not.toContain('@min-[1400px]:[columns:3_19rem]');
		expect(masonryList.className).toContain('gap-x-5');
		expect(masonryList.className).toContain('@max-[700px]:gap-x-3.5');
		for (const item of screen.getAllByRole('listitem')) {
			expect(item.className).toContain('mb-5');
			expect(item.className).toContain('@max-[700px]:mb-3.5');
		}
		masonryView.unmount();

		const compactMasonry = render(ProofGrid, {
			proofs,
			layout: 'masonry',
			density: 'compact',
			spacing: 'compact',
			onopen: () => {}
		});
		const compactList = screen.getByRole('list', { name: 'Proofs' });
		expect(compactList.className).toContain('[columns:2_19rem]');
		expect(compactList.className).toContain('@max-[700px]:columns-2');
		expect(compactList.className).toContain('@min-[1400px]:[columns:3_19rem]');
		expect(compactList.className).toContain('gap-x-2.5');
		expect(compactList.className).toContain('@max-[700px]:gap-x-1.5');
		for (const item of screen.getAllByRole('listitem')) {
			expect(item.className).toContain('mb-2.5');
			expect(item.className).toContain('@max-[700px]:mb-1.5');
		}
		compactMasonry.unmount();
	});
});
