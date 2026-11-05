import { render } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
import Layout from './+layout.svelte';

const sources: { url: string; close: ReturnType<typeof vi.fn> }[] = [];

class FakeSource extends EventTarget {
	readyState = 1;
	close = vi.fn();
	constructor(public url: string) {
		super();
		sources.push(this);
	}
}

afterEach(() => {
	vi.unstubAllGlobals();
	sources.length = 0;
});

it('opens one event stream for the dashboard and closes it on leave', () => {
	vi.stubGlobal('EventSource', FakeSource);
	const children = createRawSnippet(() => ({ render: () => '<p>dashboard</p>' }));

	const { getByText, unmount } = render(Layout, { props: { children } as never });

	expect(getByText('dashboard')).toBeTruthy();
	expect(sources.map((source) => source.url)).toEqual(['/api/v1/events']);

	unmount();

	expect(sources[0].close).toHaveBeenCalledOnce();
});
