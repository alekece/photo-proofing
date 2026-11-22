import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';
import { useLocale } from '#lib/i18n/testing.js';
import GalleryStateFilter from './GalleryStateFilter.svelte';

const counts = { draft: 2, shared: 3, delivered: 1, expired: 0, archived: 4 };

describe('GalleryStateFilter', () => {
	it('replaces one status with another or all, showing counts and the current choice', async () => {
		const user = userEvent.setup();
		const onchange = vi.fn();
		const { rerender } = render(GalleryStateFilter, { selected: 'shared', counts, onchange });

		await user.click(screen.getByRole('button', { name: 'Status: Shared' }));
		expect(screen.getByRole('menuitemradio', { name: /Shared.*3/, checked: true })).toBeTruthy();
		await user.click(screen.getByRole('menuitemradio', { name: /Archived.*4/ }));
		expect(onchange).toHaveBeenLastCalledWith('archived');
		await rerender({ selected: 'archived', counts, onchange });
		await user.click(screen.getByRole('button', { name: 'Status: Archived' }));
		await user.click(screen.getByRole('menuitemradio', { name: /All statuses/ }));
		expect(onchange).toHaveBeenLastCalledWith(null);
	});

	it('disables the filter for an empty workspace', async () => {
		const { container } = render(GalleryStateFilter, {
			selected: null,
			counts,
			disabled: true,
			onchange: vi.fn()
		});
		expect(screen.getByRole('button', { name: 'Status' })).toHaveProperty('disabled', true);
		expect((await axe(container)).violations).toEqual([]);
	});
});

describe('GalleryStateFilter in French', () => {
	useLocale('fr');
	it('names the chosen status in French', () => {
		render(GalleryStateFilter, { selected: 'shared', counts, onchange: vi.fn() });
		expect(screen.getByRole('button', { name: 'Statut: Partagée' })).toBeTruthy();
	});
});
