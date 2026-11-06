import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import EmptyState from './empty-state.svelte';

const children = createRawSnippet(() => ({ render: () => '<p>Nothing here</p>' }));

describe('EmptyState', () => {
	it('renders children in a dashed, centred card', async () => {
		const { container } = render(EmptyState, { children });

		const box = screen.getByText('Nothing here').parentElement as HTMLElement;

		expect(box.className).toContain('rounded-xl border-[1.5px] border-dashed');
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
});
