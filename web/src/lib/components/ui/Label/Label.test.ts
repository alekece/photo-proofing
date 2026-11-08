import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import Label from './Label.svelte';

const children = createRawSnippet(() => ({
	render: () => '<span>Email</span>'
}));

describe('Label', () => {
	it('renders a label associated with its control by for', async () => {
		const { container } = render(Label, { for: 'email', children });
		container.insertAdjacentHTML('beforeend', '<input id="email" />');

		expect(screen.getByLabelText('Email').id).toBe('email');
		expect(container.querySelector('label')?.className).toContain('cn-label');
		expect((await axe(container)).violations).toEqual([]);
	});

	it('adds sr-only when srOnly is set and merges a caller class', () => {
		const { container } = render(Label, {
			srOnly: true,
			class: 'text-sm',
			children
		});

		const label = container.querySelector('label') as HTMLLabelElement;

		expect(label.className).toContain('sr-only');
		expect(label.className).toContain('text-sm');
		expect(label.dataset.slot).toBe('label');
	});

	it('does not add sr-only by default', () => {
		const { container } = render(Label, { children });

		expect(container.querySelector('label')?.className).not.toContain('sr-only');
	});
});
