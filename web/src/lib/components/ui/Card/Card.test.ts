import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import Card from './Card.svelte';

const children = createRawSnippet(() => ({ render: () => '<p>Body</p>' }));

describe('Card', () => {
	it('renders children in a panel with the card classes', async () => {
		const { container } = render(Card, { children });

		const card = screen.getByText('Body').parentElement as HTMLElement;

		expect(card.tagName).toBe('DIV');
		expect(card.className).toBe('rounded-card border border-border bg-panel');
		expect((await axe(container)).violations).toEqual([]);
	});

	it('renders the requested element and merges a caller class', () => {
		render(Card, { as: 'section', class: 'p-4 rounded-dialog', children });

		const card = screen.getByText('Body').parentElement as HTMLElement;

		expect(card.tagName).toBe('SECTION');
		expect(card.className).toContain('p-4');
		expect(card.className).toContain('rounded-dialog');
		expect(card.className).not.toContain('rounded-card');
	});

	it('renders glass surfaces through one shared card mode', () => {
		render(Card, {
			as: 'p',
			surface: 'glass',
			class: 'px-2 py-1.5',
			children
		});

		const card = screen.getByText('Body').parentElement as HTMLElement;

		expect(card.tagName).toBe('P');
		expect(card.dataset.slot).toBe('card');
		expect(card.dataset.surface).toBe('glass');
		expect(card.className).toContain('glass-on-image');
		expect(card.className).not.toContain('bg-panel');
	});
});
