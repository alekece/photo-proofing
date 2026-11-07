import { fireEvent, render, screen } from '@testing-library/svelte';
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
	it('animates the pending confirmation and prevents duplicate deletion', async () => {
		let finish!: () => void;
		const onconfirm = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
		const user = userEvent.setup();
		render(DeleteGalleryDialog, {
			galleryId: 'g1',
			load: async () => [],
			onconfirm,
			onclose: vi.fn()
		});

		await user.click(await screen.findByRole('button', { name: 'Delete' }));
		const pending = screen.getByRole('button', { name: /Deleting/ });
		expect(pending.hasAttribute('disabled')).toBe(true);
		expect(pending.querySelector('svg')).toBeTruthy();
		await user.click(pending);
		expect(onconfirm).toHaveBeenCalledTimes(1);
		finish();
	});

	it('offers Keep it and Delete, and keeps the dialog open when deleting fails', async () => {
		const onconfirm = vi.fn().mockRejectedValue(new Error('Couldn’t delete.'));
		const user = userEvent.setup();
		render(DeleteGalleryDialog, {
			galleryId: 'g1',
			load: async () => [],
			onconfirm,
			onclose: vi.fn()
		});

		expect(screen.getByRole('dialog', { name: 'Delete this gallery?' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Keep it' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Delete' }));
		expect((await screen.findByRole('alert')).textContent).toBe('Couldn’t delete.');
		expect(screen.getByRole('dialog', { name: 'Delete this gallery?' })).toBeTruthy();
	});

	it('does not dismiss an in-progress deletion through Cancel, Escape or native cancel', async () => {
		let reject!: (error: Error) => void;
		const onclose = vi.fn();
		render(DeleteGalleryDialog, {
			galleryId: 'g1',
			load: async () => [],
			onconfirm: () => new Promise<void>((_, fail) => (reject = fail)),
			onclose
		});
		const user = userEvent.setup();
		await user.click(await screen.findByRole('button', { name: 'Delete' }));
		expect(screen.getByRole('button', { name: 'Keep it' })).toHaveProperty('disabled', true);
		await user.keyboard('{Escape}');
		await fireEvent(screen.getByRole('dialog'), new Event('cancel', { cancelable: true }));
		expect(onclose).not.toHaveBeenCalled();
		reject(new Error('Deletion failed.'));
		expect((await screen.findByRole('alert')).textContent).toBe('Deletion failed.');
		expect(screen.getByRole('button', { name: 'Keep it' })).toHaveProperty('disabled', false);
	});
});
