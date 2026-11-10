import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';

import EmptyWorkspace from './EmptyWorkspace.svelte';

describe('EmptyWorkspace', () => {
	it('says nothing is in frame yet in the shared empty state, without the film animation', async () => {
		const { container } = render(EmptyWorkspace, { oncreate: () => {} });

		expect(screen.getByRole('heading', { name: 'Nothing in frame yet.' })).toBeTruthy();
		expect(container.querySelector('[data-slot="empty-state"]')).not.toBeNull();
		expect(container.querySelector('[data-slot="contact-sheet"]')).toBeNull();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('creates a gallery from its action', async () => {
		const oncreate = vi.fn();
		render(EmptyWorkspace, { oncreate });

		await userEvent.click(screen.getByRole('button', { name: 'New gallery' }));

		expect(oncreate).toHaveBeenCalledOnce();
	});
});
