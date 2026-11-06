import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import FinalPanel from './FinalPanel.svelte';

const photos = [
	{ id: 'f1', name: 'f1.jpg', thumbnailUrl: '/f1', downloaded: true },
	{ id: 'f2', name: 'f2.jpg', thumbnailUrl: '/f2' }
];

describe('FinalPanel', () => {
	it('invites a drop of the finished photos, with Upload unavailable for now', () => {
		render(FinalPanel);

		expect(screen.getByText('Drop the finished photos here')).toBeTruthy();
		expect(screen.getAllByRole('button', { name: 'Upload' })[0]).toHaveProperty('disabled', true);
		expect(screen.getByRole('button', { name: 'Select' })).toHaveProperty('disabled', true);
	});

	it('marks downloaded photos and deletes a selection', async () => {
		const ondelete = vi.fn();
		const user = userEvent.setup();

		render(FinalPanel, { photos, ondelete });

		expect(screen.getAllByRole('img', { name: 'Downloaded by the client' })).toHaveLength(1);

		await user.click(screen.getByRole('button', { name: 'Select' }));
		await user.click(screen.getByRole('button', { name: 'Select all' }));
		await user.click(screen.getByRole('button', { name: 'Delete' }));

		expect(ondelete).toHaveBeenCalledWith(photos);
	});
});
