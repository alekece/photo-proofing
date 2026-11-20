import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import SelectActions from './SelectActions.svelte';

function setup(count = 1, total = 3, rail: 'dashboard' | 'gallery' = 'dashboard') {
	const handlers = { onselectall: vi.fn(), onunselectall: vi.fn(), onexit: vi.fn() };
	const view = render(SelectActions, { count, total, rail, ...handlers });

	return { ...view, ...handlers };
}

const button = (name: string) => screen.getByRole('button', { name }) as HTMLButtonElement;

describe('SelectActions', () => {
	it('uses the Neutral Warm Paper surface with enter motion', () => {
		const { container } = setup(1);
		const bar = container.querySelector('[data-slot="select-actions"]') as HTMLElement;

		expect(bar.className).toContain('bg-panel');
		expect(bar.className).not.toContain('bg-white');
		expect(bar.className).toContain('animate-selection-in');
	});

	it('shows the count and exits from the icon button', async () => {
		const { container, onexit } = setup(3);

		expect(screen.getByText('3 selected')).toBeTruthy();

		await userEvent.click(button('Exit selection'));

		expect(onexit).toHaveBeenCalledOnce();
		expect((await axe(container)).violations).toEqual([]);
	});

	it('selects everything, or clears once everything is selected', async () => {
		const { onselectall, onunselectall, rerender } = setup(1);

		await userEvent.click(button('Select all'));
		expect(onselectall).toHaveBeenCalledOnce();

		await rerender({ count: 3, total: 3 });
		expect(screen.queryByRole('button', { name: 'Select all' })).toBeNull();

		await userEvent.click(button('Unselect all'));
		expect(onunselectall).toHaveBeenCalledOnce();
	});

	it('cannot select anything when there is nothing to select', () => {
		setup(0, 0);

		expect(button('Select all').disabled).toBe(true);
	});

	it('is shifted clear of the clamp-aware workspace rail', () => {
		const { container } = setup(1, 3, 'dashboard');
		const bar = container.querySelector('[data-slot="select-actions"]') as HTMLElement;

		expect(bar.className).toMatch(/\bfixed\b/);
		expect(bar.className).toMatch(/\bbottom-/);
		expect(bar.className).not.toMatch(/\bsticky\b/);
		expect(bar.className).toMatch(/sm:left-\[calc\(50%\+clamp\(7\.5rem,8vw,8\.75rem\)\)\]/);
		expect(bar.className).toMatch(
			/sm:max-w-\[calc\(100vw-clamp\(15rem,16vw,17\.5rem\)-1\.25rem\)\]/
		);
		// Must not also carry the gallery rail's md-breakpoint offset.
		expect(bar.className).not.toMatch(/md:left-/);
	});

	it('is shifted clear of the workspace and gallery rails', () => {
		const { container } = setup(1, 3, 'gallery');
		const bar = container.querySelector('[data-slot="select-actions"]') as HTMLElement;

		expect(bar.className).toMatch(/\bfixed\b/);
		expect(bar.className).toMatch(/\bbottom-/);
		expect(bar.className).toMatch(/sm:left-\[calc\(50%\+clamp\(7\.5rem,8vw,8\.75rem\)\)\]/);
		expect(bar.className).toMatch(
			/sm:max-w-\[calc\(100vw-clamp\(15rem,16vw,17\.5rem\)-1\.25rem\)\]/
		);
		expect(bar.className).toMatch(
			/md:left-\[calc\(50%\+clamp\(7\.5rem,8vw,8\.75rem\)\+8\.25rem\)\]/
		);
		expect(bar.className).toMatch(/md:max-w-\[calc\(100vw-clamp\(15rem,16vw,17\.5rem\)-18rem\)\]/);
	});
});
