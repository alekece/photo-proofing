import { render, screen } from '@testing-library/svelte';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';

import EmptyWorkspace from './EmptyWorkspace.svelte';

describe('EmptyWorkspace', () => {
	it('says no galleries are in focus in the shared empty state, with a decorative scene', async () => {
		const { container } = render(EmptyWorkspace);

		expect(screen.getByRole('heading', { name: 'No galleries in focus.' })).toBeTruthy();
		expect(container.querySelector('[data-slot="empty-state"]')).not.toBeNull();
		expect(container.querySelector('picture img')?.getAttribute('alt')).toBe('');
		expect((await axe(container)).violations).toEqual([]);
	});

	it('exposes only the first pun to assistive tech and leaves the toolbar to create galleries', () => {
		render(EmptyWorkspace);

		expect(screen.getByText('Say cheese… or at least “New gallery”.')).toBeTruthy();
		expect(screen.getByText('Zero shots. Zero stress.').getAttribute('aria-hidden')).toBe('true');
		expect(screen.queryByRole('button')).toBeNull();
	});
});
