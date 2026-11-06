import { Grid2x2, Rows3 } from '@lucide/svelte';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import IconButtonGroup from './icon-button-group.svelte';

const items = [
	{ id: 'grid', label: 'Grid', icon: Grid2x2 },
	{ id: 'rows', label: 'Rows', icon: Rows3 }
];

describe('IconButtonGroup', () => {
	it('marks the selected button and changes selection on click', async () => {
		const { container } = render(IconButtonGroup, { items, value: 'grid', label: 'Layout' });

		expect(screen.getByRole('button', { name: 'Grid' }).getAttribute('aria-pressed')).toBe('true');
		expect((await axe(container)).violations).toEqual([]);

		await userEvent.click(screen.getByRole('button', { name: 'Rows' }));

		expect(screen.getByRole('button', { name: 'Rows' }).getAttribute('aria-pressed')).toBe('true');
		expect(screen.getByRole('button', { name: 'Grid' }).getAttribute('aria-pressed')).toBe('false');
	});
});
