import { render, screen } from '@testing-library/svelte';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';

import EmptyWorkspace from './EmptyWorkspace.svelte';

describe('EmptyWorkspace', () => {
	it('says nothing is in frame yet over a purely decorative illustration', async () => {
		const { container } = render(EmptyWorkspace);

		expect(screen.getByText('Nothing in frame yet.')).toBeTruthy();
		expect(screen.getByText(/1\/125 · F2\.8 · ISO 400/)).toBeTruthy();
		expect(
			container.querySelector('[data-slot="contact-sheet"]')?.getAttribute('aria-hidden')
		).toBe('true');
		expect((await axe(container)).violations).toEqual([]);
	});
});
