import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import SegmentedControl from './SegmentedControl.svelte';

const items = [
	{ id: 'all', label: 'All' },
	{ id: 'commented', label: 'Commented' }
];

describe('SegmentedControl', () => {
	it('marks the current segment as pressed', () => {
		render(SegmentedControl, { value: 'all', items, label: 'Photos' });

		expect(screen.getByRole('group', { name: 'Photos' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'All' }).getAttribute('aria-pressed')).toBe('true');
		expect(screen.getByRole('button', { name: 'Commented' }).getAttribute('aria-pressed')).toBe(
			'false'
		);
	});

	it('reports the picked segment', async () => {
		const onchange = vi.fn();
		render(SegmentedControl, { value: 'all', items, label: 'Photos', onchange });

		await userEvent.click(screen.getByRole('button', { name: 'Commented' }));

		expect(onchange).toHaveBeenCalledWith('commented');
		expect(screen.getByRole('button', { name: 'Commented' }).getAttribute('aria-pressed')).toBe(
			'true'
		);
	});
});
