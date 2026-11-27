import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { expect, it } from 'vitest';
import Harness from './WorkspaceToolbarHarness.test.svelte';

it('can omit search or keep it named and disabled', async () => {
	const { rerender } = render(Harness, { searchable: false });

	expect(screen.queryByRole('searchbox')).toBeNull();
	expect(screen.getByRole('button', { name: 'Action' })).toBeTruthy();

	await rerender({ searchable: true, disabled: true });

	expect(screen.getByRole('searchbox', { name: 'Search' })).toHaveProperty('disabled', true);
});

it('preserves the focused search and its value when the results stop scrolling', async () => {
	const { rerender } = render(Harness, { height: window.innerHeight + 1 });
	const search = screen.getByRole('searchbox', { name: 'Search' });
	const panel = search.closest('[data-slot="workspace-toolbar"]');

	expect(panel?.classList.contains('sticky')).toBe(true);
	await userEvent.type(search, 'test');
	await rerender({ height: 0 });

	expect(panel?.classList.contains('sticky')).toBe(false);
	expect(document.activeElement).toBe(search);
	expect(search).toHaveProperty('value', 'test');
	expect(screen.getByRole('status').textContent).toBe('test');
});
