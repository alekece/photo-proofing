import { describe, expect, it, vi } from 'vitest';
import {
	archiveGallery,
	createGallery,
	deleteGallery,
	GalleryApiError,
	restoreGallery,
	sendGalleryInvitations,
	shareGallery,
	updateGallery
} from './api';

describe('gallery API errors', () => {
	it('translates the code of a 422', async () => {
		const fetcher = vi
			.fn()
			.mockResolvedValue(new Response(JSON.stringify({ code: 'invalid_title' }), { status: 422 }));
		await expect(updateGallery('g1', { title: 'x' }, fetcher)).rejects.toMatchObject({
			status: 422,
			message: 'Enter a title of up to 255 characters.',
			code: 'invalid_title'
		});
		await expect(updateGallery('g1', {}, fetcher)).rejects.toBeInstanceOf(GalleryApiError);
	});
});

const validGallery = {
	id: 'g',
	title: null,
	layout: 'grid',
	cover_layout: 'full_bleed',
	focal_point: { x: 50, y: 35 },
	heading_font: 'editorial',
	grid_density: 'compact',
	grid_spacing: 'compact',
	expires_at: null,
	included_photos: null,
	watermark_enabled: false,
	hero_id: null,
	share_token: null,
	last_opened_at: null,
	shoot_date: null,
	states: ['draft'],
	created_at: '2026-11-05T10:00:00Z',
	updated_at: '2026-11-05T10:00:00Z'
};

describe('malformed gallery responses', () => {
	it.each([
		['createGallery', (f: typeof fetch) => createGallery(f)],
		['updateGallery', (f: typeof fetch) => updateGallery('g', {}, f)],
		['shareGallery', (f: typeof fetch) => shareGallery('g', f)],
		['archiveGallery', (f: typeof fetch) => archiveGallery('g', f)],
		['restoreGallery', (f: typeof fetch) => restoreGallery('g', f)]
	])('%s rejects a body that is not a gallery', async (_name, call) => {
		for (const body of ['{"id":"g"}', 'null', 'not json']) {
			const fetcher = vi.fn().mockResolvedValue(new Response(body));

			await expect(call(fetcher)).rejects.toBeInstanceOf(GalleryApiError);
		}
	});
});

describe('shareGallery', () => {
	it('posts to the share path and returns the gallery with its link token', async () => {
		const gallery = { ...validGallery, id: 'g 1', share_token: 'A'.repeat(22) };
		const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify(gallery)));
		expect(await shareGallery('g 1', fetcher)).toEqual(gallery);
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/g%201/share',
			expect.objectContaining({ method: 'POST' })
		);
	});
});

describe('deleteGallery', () => {
	it('sends a DELETE and accepts an empty answer', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));

		await deleteGallery('g 1', fetcher);

		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/g%201',
			expect.objectContaining({ method: 'DELETE' })
		);
	});

	it('reports the failing status', async () => {
		const fetcher = vi.fn().mockResolvedValue(new Response('{}', { status: 404 }));

		await expect(deleteGallery('g', fetcher)).rejects.toMatchObject({ status: 404 });
	});
});

describe('sendGalleryInvitations', () => {
	const payload = {
		to: [{ email: 'camille@example.com', name: 'Camille' }],
		bcc: ['archive@example.com'],
		subject: 'Your gallery is ready',
		message: 'Hello Camille!'
	};

	it('posts recipients and returns the updated gallery plus queued_count', async () => {
		const gallery = { ...validGallery, id: 'g 1', share_token: 'A'.repeat(22), states: ['shared'] };
		const fetcher = vi
			.fn()
			.mockResolvedValue(new Response(JSON.stringify({ gallery, queued_count: 2 })));

		const result = await sendGalleryInvitations('g 1', payload, fetcher);

		expect(result).toEqual({ gallery, queued_count: 2 });
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/galleries/g%201/invitations',
			expect.objectContaining({
				method: 'POST',
				body: JSON.stringify(payload)
			})
		);
	});

	it('rejects malformed responses and maps email_unavailable errors', async () => {
		const badShape = vi
			.fn()
			.mockResolvedValue(new Response(JSON.stringify({ gallery: validGallery, queued_count: 0 })));
		await expect(sendGalleryInvitations('g', payload, badShape)).rejects.toMatchObject({
			status: 502
		});

		const unavailable = vi
			.fn()
			.mockResolvedValue(
				new Response(JSON.stringify({ code: 'email_unavailable' }), { status: 503 })
			);
		await expect(sendGalleryInvitations('g', payload, unavailable)).rejects.toMatchObject({
			status: 503,
			code: 'email_unavailable',
			message: 'Email sending isn’t configured on this server.'
		});
	});
});
