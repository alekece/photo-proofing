import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const state = vi.hoisted(() => ({ page: { route: { id: '/dashboard' as string | null } } }));
vi.mock('$app/state', () => state);
import Layout from './+layout.svelte';

const children = createRawSnippet(() => ({ render: () => '<p>content</p>' }));
const close = vi.fn();
const connect = vi.fn();

beforeEach(() => {
	close.mockClear();
	connect.mockClear();
	vi.stubGlobal(
		'EventSource',
		class extends EventTarget {
			close = close;
			constructor(url: string) {
				super();
				connect(url);
			}
		}
	);
});

afterEach(() => vi.unstubAllGlobals());

describe('root layout', () => {
	it('puts studio metadata under the workspace identity rather than below each page', () => {
		state.page.route.id = '/settings';
		render(Layout, { props: { children } as never });

		expect(screen.getByText('content')).toBeTruthy();
		expect(screen.queryByRole('contentinfo')).toBeNull();
		expect(screen.getByRole('banner').textContent).toContain('©');
	});

	it('owns the workspace navigation outside the signed-in page, not on client routes', () => {
		state.page.route.id = '/settings';
		const { unmount } = render(Layout, { props: { children } as never });

		expect(screen.getByRole('navigation', { name: 'Main sections' })).toBeTruthy();
		expect(screen.getAllByRole('banner')).toHaveLength(1);
		unmount();

		state.page.route.id = '/g/[token]';
		render(Layout, { props: { children } as never });
		expect(screen.queryByRole('navigation', { name: 'Main sections' })).toBeNull();
	});

	it('shows the client gallery the version without a copy action', () => {
		state.page.route.id = '/g/[token]';
		render(Layout, { props: { children } as never });

		expect(screen.getByRole('contentinfo')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Copy build version' })).toBeNull();
	});

	it('shares workspace rail clearance across gallery and signed-in routes', () => {
		for (const routeId of [
			'/dashboard',
			'/dashboard/[id]',
			'/photos',
			'/contacts',
			'/activity',
			'/settings'
		]) {
			state.page.route.id = routeId;
			const { unmount } = render(Layout, { props: { children } as never });
			const content = screen.getByText('content').parentElement;
			expect(content?.className).toContain('sm:ml-(--workspace-rail-width)');
			expect(content?.closest('[data-studio-mode]')).toBeTruthy();

			unmount();
		}
	});

	it('does not offset client or auth routes', () => {
		for (const routeId of ['/g/[token]', '/login']) {
			state.page.route.id = routeId;
			const { unmount } = render(Layout, { props: { children } as never });
			const content = screen.getByText('content').parentElement;
			const footer = screen.getByRole('contentinfo');

			expect(content?.className).not.toContain('sm:ml-');
			expect(footer.className).not.toContain('sm:ml-');

			unmount();
		}
	});

	it('owns the workspace connection and closes it when the layout leaves', () => {
		state.page.route.id = '/activity';
		const { unmount } = render(Layout, { props: { children } as never });

		expect(connect.mock.calls).toEqual([['/api/v1/events']]);
		expect(close).not.toHaveBeenCalled();
		unmount();
		expect(close).toHaveBeenCalledOnce();
	});
});
