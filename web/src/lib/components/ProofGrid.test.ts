import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import ProofGrid from './ProofGrid.svelte';

const proofs = [1, 2].map((n) => ({
	id: `p${n}`,
	alt: `Proof ${n}`,
	previewUrl: `data:,${n}`
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
		expect(screen.getByRole('list', { name: 'Proofs' }).getAttribute('data-layout')).toBe(layout);
		expect(screen.getAllByRole('img').map((img) => img.getAttribute('alt'))).toEqual([
			'Proof 1',
			'Proof 2'
		]);
	});
});
