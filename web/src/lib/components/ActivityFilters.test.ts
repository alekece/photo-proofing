import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
import { useLocale } from '#lib/i18n/testing.js';
import ActivityFilters from './ActivityFilters.svelte';

function setup() {
	const feed = new ActivityFeed({ fetcher: vi.fn() as unknown as typeof fetch });
	const load = vi.spyOn(feed, 'load').mockResolvedValue();

	render(ActivityFilters, { feed, searchLabel: 'Search by visitor' });

	return { feed, load };
}

describe('ActivityFilters', () => {
	useLocale('en');

	it('applies a date choice and reloads', async () => {
		const { feed, load } = setup();
		const user = userEvent.setup();

		await user.click(screen.getByRole('button', { name: 'Date' }));
		await user.click(screen.getByRole('menuitemradio', { name: 'Last 7 days' }));

		expect(feed.filters).toEqual({ range: '7d' });
		expect(load).toHaveBeenCalledOnce();
	});

	it('clears a filter from its own menu without changing sort or showing a toolbar reset', async () => {
		const { feed } = setup();
		const user = userEvent.setup();

		await user.click(screen.getByRole('button', { name: 'Type' }));
		await user.click(screen.getByRole('menuitemradio', { name: 'Views' }));
		expect(feed.filters.kind).toBe('open');
		expect(screen.queryByRole('button', { name: 'Reset' })).toBeNull();

		await user.click(screen.getByRole('button', { name: 'Sort by: Newest first' }));
		await user.click(screen.getByRole('menuitemradio', { name: 'Oldest first' }));
		await user.click(screen.getByRole('button', { name: 'Type: Views' }));
		await user.click(screen.getByRole('menuitemradio', { name: 'All' }));
		expect(feed.filters.kind).toBeUndefined();
		expect(feed.sort).toBe('oldest');
	});

	it('has no axe violations', async () => {
		const feed = new ActivityFeed({ fetcher: vi.fn() as unknown as typeof fetch });
		const { container } = render(ActivityFilters, { feed, searchLabel: 'Search by visitor' });

		expect((await axe(container)).violations).toEqual([]);
	});
});
