import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import ArchiveGalleryDialog from './ArchiveGalleryDialog.svelte';
import DeleteGalleryDialog from './DeleteGalleryDialog.svelte';

describe('Archive gallery dialog', () => {
	it('asks first and confirms only on Archive', async () => {
		const onconfirm = vi.fn();
		const onclose = vi.fn();
		const user = userEvent.setup();
		render(ArchiveGalleryDialog, { onconfirm, onclose });

		expect(screen.getByRole('dialog', { name: 'Archive this gallery?' })).toBeTruthy();
		expect(screen.getByText(/client link stops working/)).toBeTruthy();
		expect(onconfirm).not.toHaveBeenCalled();

		await user.click(screen.getByRole('button', { name: 'Archive' }));
		expect(onconfirm).toHaveBeenCalledOnce();
	});
});

describe('Delete gallery dialog', () => {
	it('offers Keep it and Delete, and keeps the dialog open when deleting fails', async () => {
		const onconfirm = vi.fn().mockRejectedValue(new Error('Couldn’t delete.'));
		const user = userEvent.setup();
		render(DeleteGalleryDialog, { onconfirm, onclose: vi.fn() });

		expect(screen.getByRole('dialog', { name: 'Delete this gallery?' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Keep it' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Delete' }));
		expect((await screen.findByRole('alert')).textContent).toBe('Couldn’t delete.');
		expect(screen.getByRole('dialog', { name: 'Delete this gallery?' })).toBeTruthy();
	});
});
