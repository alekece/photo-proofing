import { Heart } from '@lucide/svelte';
import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import EmptyState from './EmptyState.svelte';

const children = createRawSnippet(() => ({ render: () => '<p>Nothing here</p>' }));

describe('EmptyState', () => {
	it('renders children in a striped, centred card', async () => {
		const { container } = render(EmptyState, { children });

		const box = screen.getByText('Nothing here').parentElement as HTMLElement;

		expect(box.className).toContain('rounded-xl border border-border');
		expect(box.className).toContain('repeating-linear-gradient');
		expect(box.className).toContain('p-12 text-center');
		expect(box.className).toContain('justify-items-center');
		expect((await axe(container)).violations).toEqual([]);
	});

	it('merges a caller class', () => {
		render(EmptyState, { class: 'rounded-dialog text-muted-foreground', children });

		const { className } = screen.getByText('Nothing here').parentElement as HTMLElement;

		expect(className).toContain('rounded-dialog');
		expect(className).not.toContain('rounded-card');
		expect(className).toContain('text-muted-foreground');
	});

	it('renders an icon, title and description in the shared order', () => {
		render(EmptyState, { icon: Heart, title: 'No favorites', description: 'They appear here.' });

		const box = screen.getByRole('heading', { name: 'No favorites' }).parentElement as HTMLElement;

		expect(Array.from(box.children).map((node) => node.tagName)).toEqual(['svg', 'H3', 'P']);
		expect(screen.getByText('They appear here.')).toBeTruthy();
	});

	it('has no stripes in the plain variant, reserved for read-only empty states', () => {
		render(EmptyState, { variant: 'plain', children });

		const box = screen.getByText('Nothing here').parentElement as HTMLElement;

		expect(box.className).not.toContain('repeating-linear-gradient');
		expect(box.className).toContain('justify-items-center');
		expect(box.className).toContain('text-center');
	});
});
