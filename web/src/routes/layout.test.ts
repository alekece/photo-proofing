import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it, vi } from 'vitest';

const state = vi.hoisted(() => ({ page: { route: { id: '/dashboard' as string | null } } }));
vi.mock('$app/state', () => state);
import Layout from './+layout.svelte';

const children = createRawSnippet(() => ({ render: () => '<p>content</p>' }));

describe('root layout', () => {
	it('adds the shared footer after the page with a copy action', () => {
		state.page.route.id = '/settings';
		render(Layout, { props: { children } as never });

		expect(screen.getByText('content')).toBeTruthy();
		expect(screen.getByRole('contentinfo')).toBeTruthy();
		expect(screen.queryAllByRole('contentinfo')).toHaveLength(1);
	});

	it('shows the client gallery the version without a copy action', () => {
		state.page.route.id = '/g/[token]';
		render(Layout, { props: { children } as never });

		expect(screen.getByRole('contentinfo')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Copy build version' })).toBeNull();
	});
});
