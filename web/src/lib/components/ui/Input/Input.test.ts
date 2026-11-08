import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Input from './Input.svelte';

describe('Input', () => {
	it.each(['date', 'number'] as const)('passes type=%s through', (type) => {
		const { container } = render(Input, { type });

		expect(container.querySelector('input')?.getAttribute('type')).toBe(type);
	});
});
