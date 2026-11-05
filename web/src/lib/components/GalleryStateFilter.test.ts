import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';
import GalleryStateFilter from './GalleryStateFilter.svelte';

describe('GalleryStateFilter', () => {
	it('lists the five states in a labelled group and checks the selected ones', async () => {
		const { container } = render(GalleryStateFilter, {
			props: { selected: ['shared'], onchange: vi.fn() }
		});
		const group = screen.getByRole('group', { name: 'State' });

		expect(group).toBeTruthy();
		expect(
			screen.getAllByRole('checkbox').map((box) => box.parentElement?.textContent?.trim())
		).toEqual(['Draft', 'Shared', 'Delivered', 'Archived', 'Expired']);
		expect((screen.getByRole('checkbox', { name: 'Shared' }) as HTMLInputElement).checked).toBe(
			true
		);
		expect((screen.getByRole('checkbox', { name: 'Draft' }) as HTMLInputElement).checked).toBe(
			false
		);
		expect((await axe(container)).violations).toEqual([]);
	});

	it('reports the next selection when a state is checked or unchecked', async () => {
		const onchange = vi.fn();
		const user = userEvent.setup();

		render(GalleryStateFilter, { props: { selected: ['shared'], onchange } });
		await user.click(screen.getByRole('checkbox', { name: 'Expired' }));
		expect(onchange).toHaveBeenLastCalledWith(['shared', 'expired']);
		await user.click(screen.getByRole('checkbox', { name: 'Shared' }));
		expect(onchange).toHaveBeenLastCalledWith([]);
	});
});
