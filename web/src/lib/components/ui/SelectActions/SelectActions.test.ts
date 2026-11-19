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

	it('is a fixed bottom bar shifted clear of the dashboard workspace rail (sm:w-60)', () => {
		const { container } = setup(1, 3, 'dashboard');
		const bar = container.querySelector('[data-slot="select-actions"]') as HTMLElement;

		expect(bar.className).toMatch(/\bfixed\b/);
		expect(bar.className).toMatch(/\bbottom-/);
		expect(bar.className).not.toMatch(/\bsticky\b/);
		// Centres within the content area to the right of the dashboard's sm:w-60 fixed rail.
		expect(bar.className).toMatch(/sm:left-\[calc\(50%\+7\.5rem\)\]/);
		expect(bar.className).toMatch(/sm:max-w-\[calc\(100vw-16\.5rem\)\]/);
		// Must not also carry the gallery rail's md-breakpoint offset.
		expect(bar.className).not.toMatch(/md:left-/);
	});

	it('is a fixed bottom bar shifted clear of the gallery workspace rail (md:w-[16.5rem])', () => {
		const { container } = setup(1, 3, 'gallery');
		const bar = container.querySelector('[data-slot="select-actions"]') as HTMLElement;

		expect(bar.className).toMatch(/\bfixed\b/);
		expect(bar.className).toMatch(/\bbottom-/);
		// Centres within the content area to the right of the gallery studio's md:w-[16.5rem] rail,
		// which only takes effect from the md breakpoint, not sm.
		expect(bar.className).toMatch(/md:left-\[calc\(50%\+8\.25rem\)\]/);
		expect(bar.className).toMatch(/md:max-w-\[calc\(100vw-18rem\)\]/);
		// Must not also carry the dashboard rail's sm-breakpoint offset.
		expect(bar.className).not.toMatch(/sm:left-/);
	});
});
