import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import ProofGrid from './ProofGrid.svelte';

const proofs = [1, 2].map((n) => ({ id: `p${n}`, alt: `Proof ${n}`, previewUrl: `data:,${n}` }));

describe('ProofGrid', () => {
	it('lists each proof as a keyboard-operable button', async () => {
		const onopen = vi.fn();
		const user = userEvent.setup();
		render(ProofGrid, { proofs, onopen });
		expect(screen.getAllByRole('listitem')).toHaveLength(2);
		await user.tab();
		await user.tab();
		await user.keyboard('{Enter}');
		expect(onopen).toHaveBeenCalledWith(1);
	});

	it('renders an empty message when there are no proofs', () => {
		render(ProofGrid, { proofs: [], onopen: () => {} });
		expect(screen.getByText(/no proofs/i)).toBeTruthy();
	});
});
