import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import PageShell from './PageShell.svelte';

const children = createRawSnippet(() => ({ render: () => '<h1>Hi</h1>' }));

describe('PageShell', () => {
	it('renders a main landmark with children and default width', async () => {
		const { container } = render(PageShell, { children });

		const main = screen.getByRole('main');

		expect(main.className).toContain('mx-auto grid');
		expect(main.className).toContain('gap-6 p-6');
		expect(main.className).toContain('max-w-5xl');
		expect(screen.getByRole('heading', { name: 'Hi' })).toBeTruthy();
		expect((await axe(container)).violations).toEqual([]);
	});

	it.each([
		['narrow', 'max-w-sm'],
		['wide', 'max-w-6xl']
	] as const)('maps %s to %s', (width, cls) => {
		render(PageShell, { width, children });

		expect(screen.getByRole('main').className).toContain(cls);
	});

	it('merges a caller class over the defaults', () => {
		render(PageShell, { class: 'flex px-6 py-12', children });

		const { className } = screen.getByRole('main');

		expect(className).toContain('flex');
		expect(className).not.toContain('grid');
		expect(className).toContain('px-6 py-12');
	});
});
