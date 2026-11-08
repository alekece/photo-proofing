import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import Button, { buttonVariants } from './Button.svelte';

describe('buttonVariants', () => {
	it('gives sm a distinct, real size class set that keeps the touch target', () => {
		const small = buttonVariants({ size: 'sm' });
		const regular = buttonVariants();

		expect(small).not.toBe(regular);
		expect(small).toContain('text-sm');
		expect(small).toContain('min-h-11');
		expect(regular).toContain('min-h-11');
		expect(small).not.toContain('cn-button-size-');
		expect(regular).not.toContain('cn-button-size-');
	});
});

describe('Button', () => {
	it('renders a disabled button that ignores pointer events', () => {
		render(Button, {
			disabled: true,
			children: createRawSnippet(() => ({ render: () => '<span>Save</span>' }))
		});

		const button = screen.getByRole('button');

		expect(button.hasAttribute('disabled')).toBe(true);
		expect(button.className).toContain('disabled:pointer-events-none');
	});
});
