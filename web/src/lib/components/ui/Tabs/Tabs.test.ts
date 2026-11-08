import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import TabsHarness from './TabsHarness.test.svelte';

const selected = () => screen.getByTestId('value').textContent;

describe('Tabs', () => {
	it('wires tablist, tabs and the panel labelled by the active tab', async () => {
		const { container } = render(TabsHarness);
		const alpha = screen.getByRole('tab', { name: 'Alpha' });
		const panel = screen.getByRole('tabpanel');

		expect(screen.getByRole('tablist', { name: 'Sections' })).toBeTruthy();
		expect(alpha.getAttribute('aria-selected')).toBe('true');
		expect(alpha.getAttribute('aria-controls')).toBe(panel.id);
		expect(panel.getAttribute('aria-labelledby')).toBe(alpha.id);
		expect(panel.textContent).toContain('Content a');
		expect((await axe(container)).violations).toEqual([]);
	});

	it('keeps only the active tab in the tab order', () => {
		render(TabsHarness);

		expect(screen.getByRole('tab', { name: 'Alpha' }).getAttribute('tabindex')).toBe('0');
		expect(screen.getByRole('tab', { name: 'Beta' }).getAttribute('tabindex')).toBe('-1');
		expect(screen.getByRole('tab', { name: 'Gamma' }).getAttribute('tabindex')).toBe('-1');
	});

	it('selects on click', async () => {
		render(TabsHarness);

		await userEvent.click(screen.getByRole('tab', { name: 'Beta' }));

		expect(selected()).toBe('b');
		expect(screen.getByRole('tabpanel').textContent).toContain('Content b');
	});

	it('moves focus and selection with arrows, wrapping at both ends', async () => {
		render(TabsHarness);
		screen.getByRole('tab', { name: 'Alpha' }).focus();

		await userEvent.keyboard('{ArrowLeft}');

		expect(selected()).toBe('c');
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Gamma' }));

		await userEvent.keyboard('{ArrowRight}');

		expect(selected()).toBe('a');
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Alpha' }));

		await userEvent.keyboard('{ArrowRight}');

		expect(selected()).toBe('b');
	});

	it('jumps to the first and last tab with Home and End', async () => {
		render(TabsHarness);
		screen.getByRole('tab', { name: 'Alpha' }).focus();

		await userEvent.keyboard('{End}');

		expect(selected()).toBe('c');
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Gamma' }));

		await userEvent.keyboard('{Home}');

		expect(selected()).toBe('a');
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Alpha' }));
	});

	it('shows counts, an accent on one and a padlock', () => {
		render(TabsHarness, {
			items: [
				{ id: 'a', label: 'Alpha', count: 3 },
				{ id: 'b', label: 'Beta', count: 9, accent: true, locked: true }
			]
		});

		expect(screen.getByRole('tab', { name: /Alpha 3/ })).toBeTruthy();

		const beta = screen.getByRole('tab', { name: /Beta 9/ });

		expect(beta.querySelector('.font-bold')?.textContent).toBe('9');
		expect(beta.querySelector('[aria-label="Locked"]')).toBeTruthy();
	});
});
