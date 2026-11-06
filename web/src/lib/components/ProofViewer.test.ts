import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { Proof } from '../gallery/types';
import ProofViewer from './ProofViewer.svelte';
import ViewerHarness from './ViewerHarness.test.svelte';

const proofs: Proof[] = [1, 2, 3].map((n) => ({
	id: `p${n}`,
	alt: `Proof ${n}`,
	previewUrl: `data:image/svg+xml,${n}`,
	thumbnailUrl: `data:,t${n}`,
	width: 300,
	height: 200
}));

const shown = () => screen.getByRole('img').getAttribute('alt');

describe('ProofViewer', () => {
	it('is a labelled modal dialog with focus on the close button', () => {
		render(ProofViewer, { proofs, index: 0, onclose: () => {} });
		const dialog = screen.getByRole('dialog', { name: /proof viewer/i });
		expect(dialog.getAttribute('aria-modal')).toBe('true');
		expect(document.activeElement).toBe(screen.getByRole('button', { name: /close/i }));
		expect(screen.getByText('1 of 3')).toBeTruthy();
		expect(screen.getByText('Proof 1', { selector: 'span' })).toBeTruthy();
	});

	it('shows a read-only heart for the photographer', () => {
		render(ProofViewer, {
			proofs,
			index: 0,
			readonly: true,
			isSelected: () => true,
			onclose: () => {}
		});
		const heart = screen.getByRole('button', { name: /favorite proof 1/i }) as HTMLButtonElement;
		expect(heart.disabled).toBe(true);
		expect(heart.getAttribute('aria-pressed')).toBe('true');
	});

	it('shows a spinner while a photo is processing', () => {
		render(ProofViewer, {
			proofs: [{ ...proofs[0], status: 'processing' }],
			index: 0,
			onclose: () => {}
		});
		expect(screen.queryByRole('img')).toBeNull();
		expect(screen.getByRole('status').textContent).toMatch(/processing/i);
	});

	it('offers Try again when processing failed', async () => {
		const onretry = vi.fn();
		const user = userEvent.setup();
		render(ProofViewer, {
			proofs: [{ ...proofs[0], status: 'failed' }],
			index: 0,
			onretry,
			onclose: () => {}
		});
		expect(screen.getByRole('alert').textContent).toBe('This photo could not be processed');
		await user.click(screen.getByRole('button', { name: 'Try again' }));
		expect(onretry).toHaveBeenCalledWith('p1');
	});

	it('navigates with arrows, Home and End, stopping at the ends', async () => {
		const user = userEvent.setup();
		render(ProofViewer, { proofs, index: 0, onclose: () => {} });
		await user.keyboard('{ArrowRight}');
		expect(shown()).toBe('Proof 2');
		await user.keyboard('{End}');
		expect(shown()).toBe('Proof 3');
		await user.keyboard('{ArrowRight}');
		expect(shown()).toBe('Proof 3');
		expect(screen.getByRole('button', { name: /next/i }).getAttribute('aria-disabled')).toBe(
			'true'
		);
		await user.keyboard('{Home}');
		await user.keyboard('{ArrowLeft}');
		expect(shown()).toBe('Proof 1');
		expect(screen.getByRole('button', { name: /previous/i }).getAttribute('aria-disabled')).toBe(
			'true'
		);
	});

	it('operates through keyboard-activated buttons', async () => {
		const user = userEvent.setup();
		render(ProofViewer, { proofs, index: 0, onclose: () => {} });
		screen.getByRole('button', { name: /next/i }).focus();
		await user.keyboard('{Enter}');
		expect(shown()).toBe('Proof 2');
		screen.getByRole('button', { name: /previous/i }).focus();
		await user.keyboard(' ');
		expect(shown()).toBe('Proof 1');
	});

	it('closes on Escape', async () => {
		const onclose = vi.fn();
		const user = userEvent.setup();
		render(ProofViewer, { proofs, index: 0, onclose });
		await user.keyboard('{Escape}');
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('traps Tab focus inside the dialog', async () => {
		const user = userEvent.setup();
		render(ProofViewer, { proofs, index: 0, onclose: () => {} });
		const buttons = screen.getAllByRole('button');
		const dialog = screen.getByRole('dialog');
		for (let i = 0; i < buttons.length * 2 + 1; i++) {
			await user.tab();
			expect(dialog.contains(document.activeElement)).toBe(true);
		}
		await user.tab({ shift: true });
		expect(dialog.contains(document.activeElement)).toBe(true);
	});

	it('restores focus to the opener when closed', async () => {
		const user = userEvent.setup();
		render(ViewerHarness, { proofs });
		const opener = screen.getByRole('button', { name: 'Open viewer' });
		opener.focus();
		await user.keyboard('{Enter}');
		expect(screen.getByRole('dialog')).toBeTruthy();
		await user.keyboard('{Escape}');
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(document.activeElement).toBe(opener);
	});
});
