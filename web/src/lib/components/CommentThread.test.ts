import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { CommentsClient } from '#lib/gallery/comments.js';
import CommentThread from './CommentThread.svelte';

const comment = {
	id: 'c1',
	photo_id: 'p1',
	author: 'client',
	body: 'Nice',
	created_at: '2026-11-10T09:00:00Z',
	edited_at: null,
	deleted_at: null
};

describe('CommentThread', () => {
	it('loads plain text, only offers own controls and sends a new comment', async () => {
		const user = userEvent.setup();
		const fetcher = vi
			.fn<typeof fetch>()
			.mockResolvedValueOnce(
				Response.json([comment, { ...comment, id: 'c2', author: 'photographer', body: 'Thanks' }])
			)
			.mockResolvedValueOnce(Response.json({ ...comment, id: 'c3', body: 'Beautiful' }));
		const onchange = vi.fn();
		render(CommentThread, {
			client: new CommentsClient('/base', fetcher),
			photoId: 'p1',
			viewer: 'client',
			onchange
		});
		expect(await screen.findByText('Nice')).toBeTruthy();
		expect(screen.queryByText(/^\d+\/500$/)).toBeNull();
		expect(screen.getAllByRole('button', { name: 'Edit' })).toHaveLength(1);
		expect(screen.getByRole('button', { name: 'Send' }).hasAttribute('disabled')).toBe(true);
		await user.type(screen.getByRole('textbox'), 'Beautiful');
		await user.click(screen.getByRole('button', { name: 'Send' }));
		expect(await screen.findByText('Beautiful')).toBeTruthy();
		expect(onchange).toHaveBeenCalledOnce();
	});

	it.each(['client', 'photographer'] as const)(
		'shows deleted content appropriately for %s',
		async (viewer) => {
			const fetcher = vi.fn<typeof fetch>().mockResolvedValue(
				Response.json([
					{
						...comment,
						body: viewer === 'client' ? null : 'Nice',
						deleted_at: comment.created_at
					}
				])
			);
			render(CommentThread, {
				client: new CommentsClient('/base', fetcher),
				photoId: 'p1',
				viewer
			});
			const text = await screen.findByText(viewer === 'client' ? 'Comment deleted' : 'Nice');
			if (viewer === 'photographer') expect(text.classList.contains('line-through')).toBe(true);
			expect(screen.queryByRole('button', { name: 'Edit' })).toBeNull();
		}
	);

	it('edits and confirms deletion without duplicating a message', async () => {
		const user = userEvent.setup();
		const fetcher = vi
			.fn<typeof fetch>()
			.mockResolvedValueOnce(Response.json([comment]))
			.mockResolvedValueOnce(
				Response.json({ ...comment, body: 'Changed', edited_at: comment.created_at })
			)
			.mockResolvedValueOnce(new Response(null, { status: 204 }))
			.mockResolvedValueOnce(
				Response.json([{ ...comment, body: null, deleted_at: comment.created_at }])
			);
		render(CommentThread, {
			client: new CommentsClient('/base', fetcher),
			photoId: 'p1',
			viewer: 'client'
		});
		await screen.findByText('Nice');
		await user.click(screen.getByRole('button', { name: 'Edit' }));
		const input = screen.getByRole('textbox');
		await user.clear(input);
		await user.type(input, 'Changed');
		await user.click(screen.getByRole('button', { name: 'Save' }));
		expect(await screen.findByText('Changed')).toBeTruthy();
		await user.click(screen.getByRole('button', { name: 'Delete' }));
		await user.click(screen.getByRole('button', { name: 'Delete comment' }));
		expect(await screen.findByText('Comment deleted')).toBeTruthy();
	});

	it('shows load and write errors without discarding the draft', async () => {
		const user = userEvent.setup();
		const fetcher = vi
			.fn<typeof fetch>()
			.mockResolvedValueOnce(Response.json([]))
			.mockRejectedValueOnce(new TypeError('network'));
		render(CommentThread, {
			client: new CommentsClient('/base', fetcher),
			photoId: 'p1',
			viewer: 'client'
		});
		await waitFor(() => expect(screen.queryByRole('status')).toBeNull());
		await user.type(screen.getByRole('textbox'), 'Keep me');
		await user.click(screen.getByRole('button', { name: 'Send' }));
		expect(await screen.findByRole('alert')).toBeTruthy();
		expect((screen.getByRole('textbox') as HTMLTextAreaElement).value).toBe('Keep me');
	});
});
