import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet, tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { useLocale } from '#lib/i18n/testing.js';
import type { Proof } from '../gallery/types';
import PhotoViewer from './PhotoViewer.svelte';
import ViewerHarness from './ViewerHarness.test.svelte';

afterEach(() => {
	vi.useRealTimers();
});

const proofs: Proof[] = [1, 2, 3].map((n) => ({
	id: `p${n}`,
	alt: `Proof ${n}`,
	previewUrl: `data:image/svg+xml,${n}`,
	thumbnailUrl: `data:,t${n}`,
	width: 300,
	height: 200
}));

const shown = () => screen.getByRole('img').getAttribute('alt');

describe('PhotoViewer', () => {
	useLocale('en');

	it('opens the photographer thread by default, keeps it across navigation, and closes it on Escape first', async () => {
		const user = userEvent.setup();
		const onclose = vi.fn();
		render(PhotoViewer, {
			proofs,
			onclose,
			commentCount: () => 2,
			thread: createRawSnippet(() => ({ render: () => '<p>Thread content</p>' }))
		});
		const button = screen.getByRole('button', { name: /comments/i });

		expect(screen.getByText('Thread content')).toBeTruthy();
		expect(button.textContent).toContain('Hide comments');
		await user.click(screen.getByRole('button', { name: /next/i }));
		expect(screen.getByText('Thread content')).toBeTruthy();
		await user.keyboard('{Escape}');
		expect(screen.queryByText('Thread content')).toBeNull();
		expect(onclose).not.toHaveBeenCalled();
		await user.click(button);
		expect(screen.getByText('Thread content')).toBeTruthy();
		await user.keyboard('{Escape}');
		await user.keyboard('{Escape}');
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('keeps the client thread closed until asked for', async () => {
		const user = userEvent.setup();
		render(PhotoViewer, {
			proofs,
			title: 'Wedding',
			onclose: () => {},
			commentCount: () => 2,
			thread: createRawSnippet(() => ({ render: () => '<p>Thread content</p>' }))
		});

		expect(screen.queryByText('Thread content')).toBeNull();
		await user.click(screen.getByRole('button', { name: /comments/i }));
		expect(screen.getByText('Thread content')).toBeTruthy();
	});

	it('has no comments control when no thread is provided', () => {
		render(PhotoViewer, { proofs, onclose: () => {} });
		expect(screen.queryByRole('button', { name: /comments/i })).toBeNull();
	});
	it('is a labelled modal dialog with focus on the close button', () => {
		render(PhotoViewer, { proofs, index: 0, onclose: () => {} });
		const dialog = screen.getByRole('dialog', { name: /proof viewer/i });
		expect(dialog.getAttribute('aria-modal')).toBe('true');
		expect(document.activeElement).toBe(screen.getByRole('button', { name: /close/i }));
		expect(screen.getByText('1 of 3')).toBeTruthy();
		expect(screen.getByText('Proof 1', { selector: 'span' })).toBeTruthy();
	});

	it('shows a read-only heart for the photographer', () => {
		render(PhotoViewer, {
			proofs,
			index: 0,
			readonly: true,
			isSelected: () => true,
			onclose: () => {}
		});
		expect(screen.getByRole('img', { name: 'Marked as a favorite by the client' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: /favorite/i })).toBeNull();
	});

	it('shows a Download button once delivered', () => {
		render(PhotoViewer, {
			proofs: [{ ...proofs[0], downloadUrl: '/final/p1' }],
			index: 0,
			title: 'Gallery',
			onclose: () => {}
		});
		expect(screen.getByRole('link', { name: 'Download' }).getAttribute('href')).toBe('/final/p1');
		expect(screen.queryByRole('button', { name: /favorite/i })).toBeNull();
	});

	it('labels the heart by what a click does', async () => {
		const user = userEvent.setup();
		const onselect = vi.fn();
		render(PhotoViewer, { proofs, index: 0, onselect, title: 'Gallery', onclose: () => {} });
		await user.click(screen.getByRole('button', { name: 'Add to favorites' }));
		expect(onselect).toHaveBeenCalledWith('p1');
	});

	it('shows a spinner while a photo is processing', () => {
		render(PhotoViewer, {
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
		render(PhotoViewer, {
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
		render(PhotoViewer, { proofs, index: 0, onclose: () => {} });
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
		render(PhotoViewer, { proofs, index: 0, onclose: () => {} });
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
		render(PhotoViewer, { proofs, index: 0, onclose });
		await user.keyboard('{Escape}');
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('traps Tab focus inside the dialog', async () => {
		const user = userEvent.setup();
		render(PhotoViewer, { proofs, index: 0, onclose: () => {} });
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

	it('omits slideshow controls in photographer mode and on single-photo galleries, and passes axe when playing', async () => {
		const studio = render(PhotoViewer, { proofs, index: 0, onclose: () => {} });
		expect(screen.queryByRole('button', { name: /slideshow/i })).toBeNull();
		studio.unmount();

		const single = render(PhotoViewer, {
			proofs: [proofs[0]],
			index: 0,
			title: 'Gallery',
			onclose: () => {}
		});
		expect(screen.queryByRole('button', { name: /slideshow/i })).toBeNull();
		single.unmount();

		const { container } = render(PhotoViewer, {
			proofs,
			index: 0,
			title: 'Gallery',
			autoplay: true,
			onclose: () => {}
		});
		expect((await axe(container)).violations).toEqual([]);
	});

	it('plays every 4 seconds in client mode (including finals), stops on the last photo, and restarts from photo 1', async () => {
		vi.useFakeTimers();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const finals = proofs.map((p) => ({ ...p, downloadUrl: `/final/${p.id}` }));
		render(PhotoViewer, {
			proofs: finals,
			index: 0,
			title: 'Finals',
			onclose: () => {}
		});

		await user.click(screen.getByRole('button', { name: 'Play slideshow' }));
		expect(screen.getByRole('button', { name: 'Pause slideshow' })).toBeTruthy();

		await vi.advanceTimersByTimeAsync(3999);
		expect(shown()).toBe('Proof 1');

		await vi.advanceTimersByTimeAsync(1);
		await tick();
		expect(shown()).toBe('Proof 2');

		await vi.advanceTimersByTimeAsync(4000);
		await tick();
		expect(shown()).toBe('Proof 3');
		expect(screen.getByRole('button', { name: 'Play slideshow' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Play slideshow' }));
		expect(shown()).toBe('Proof 1');
		expect(screen.getByRole('button', { name: 'Pause slideshow' })).toBeTruthy();

		await vi.advanceTimersByTimeAsync(4000);
		await tick();
		expect(shown()).toBe('Proof 2');
	});

	it('supports autoplay on open and clears the slideshow timer on unmount', async () => {
		vi.useFakeTimers();
		const spy = vi.spyOn(globalThis, 'clearTimeout');
		const { unmount } = render(PhotoViewer, {
			proofs,
			index: 0,
			title: 'Gallery',
			autoplay: true,
			onclose: () => {}
		});

		expect(screen.getByRole('button', { name: 'Pause slideshow' })).toBeTruthy();

		unmount();
		expect(spy).toHaveBeenCalled();
		expect(() => vi.advanceTimersByTime(8000)).not.toThrow();
	});

	it('pauses playback on manual navigation, toggling a favorite, or opening comments', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		render(PhotoViewer, {
			proofs,
			index: 0,
			title: 'Gallery',
			autoplay: true,
			onselect: vi.fn(),
			commentCount: () => 0,
			thread: createRawSnippet(() => ({ render: () => '<p>Thread</p>' })),
			onclose: () => {}
		});

		await user.keyboard('{ArrowRight}');
		expect(shown()).toBe('Proof 2');
		expect(screen.getByRole('button', { name: 'Play slideshow' })).toBeTruthy();
		await vi.advanceTimersByTimeAsync(4000);
		expect(shown()).toBe('Proof 2');

		await user.click(screen.getByRole('button', { name: 'Play slideshow' }));
		await user.click(screen.getByRole('button', { name: 'Add to favorites' }));
		expect(screen.getByRole('button', { name: 'Play slideshow' })).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Play slideshow' }));
		await user.click(screen.getByRole('button', { name: /comments/i }));
		expect(screen.getByRole('button', { name: 'Play slideshow' })).toBeTruthy();
	});
});

describe('PhotoViewer in French', () => {
	useLocale('fr');

	it('localizes slideshow controls with zero axe violations', async () => {
		const user = userEvent.setup();
		const { container } = render(PhotoViewer, {
			proofs,
			index: 0,
			title: 'Mariage',
			onclose: () => {}
		});

		const play = screen.getByRole('button', { name: 'Lire le diaporama' });
		expect(play).toBeTruthy();
		await user.click(play);
		expect(screen.getByRole('button', { name: 'Mettre le diaporama en pause' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});
});
