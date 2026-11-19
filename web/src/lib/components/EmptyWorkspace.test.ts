import { render, screen } from '@testing-library/svelte';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';

import EmptyWorkspace from './EmptyWorkspace.svelte';

describe('EmptyWorkspace', () => {
	it('says no galleries are in focus in a plain icon/title/sentence empty state', async () => {
		const { container } = render(EmptyWorkspace);

		expect(screen.getByRole('heading', { name: 'No galleries in focus.' })).toBeTruthy();
		expect(screen.getByText('Create a gallery and it will show up here.')).toBeTruthy();
		expect(container.querySelector('[data-slot="empty-state"]')).not.toBeNull();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('has no motion or illustration, is not a striped drop zone, and leaves the toolbar to create galleries', () => {
		const { container } = render(EmptyWorkspace);

		expect(container.querySelector('picture')).toBeNull();
		expect(container.querySelector('img')).toBeNull();
		expect(container.querySelector('[data-slot="empty-state"]')?.className).not.toContain(
			'border-dashed'
		);
		expect(screen.queryByRole('button')).toBeNull();
	});
});
