import { render } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
import Layout from './+layout.svelte';
import { UploadQueue } from '#lib/gallery/uploads.svelte.js';

it('keeps one upload queue for the dashboard, guards leaving and clears it on leave', () => {
	const guard = vi.spyOn(UploadQueue.prototype, 'guard');
	const clear = vi.spyOn(UploadQueue.prototype, 'clear');
	const children = createRawSnippet(() => ({ render: () => '<p>dashboard</p>' }));

	const { unmount } = render(Layout, { props: { children } as never });

	window.dispatchEvent(new Event('beforeunload', { cancelable: true }));

	expect(guard).toHaveBeenCalledOnce();
	expect(clear).not.toHaveBeenCalled();

	unmount();

	expect(clear).toHaveBeenCalledOnce();
});
