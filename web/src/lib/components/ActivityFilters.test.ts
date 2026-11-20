import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
import { useLocale } from '#lib/i18n/testing.js';
import ActivityFilters from './ActivityFilters.svelte';

const galleries = [
	{ id: 'g1', title: 'Wedding' },
	{ id: 'g2', title: null }
];
const contacts = [{ id: 'c1', name: 'Alice' }];

function setup(props: { galleries?: typeof galleries } = { galleries }) {
	const feed = new ActivityFeed({ fetcher: vi.fn() as unknown as typeof fetch });
	const load = vi.spyOn(feed, 'load').mockResolvedValue();

	render(ActivityFilters, { feed, contacts, ...props });

	return { feed, load };
}

describe('ActivityFilters', () => {
	useLocale('en');

	it('applies a gallery, visitor and date choice and reloads', async () => {
		const { feed, load } = setup();
		const user = userEvent.setup();

		await user.click(screen.getByRole('button', { name: 'Filters' }));
		await user.selectOptions(screen.getByRole('combobox', { name: 'Gallery' }), 'g1');
		await user.selectOptions(screen.getByRole('combobox', { name: 'Visitor' }), 'anonymous');
		await user.selectOptions(screen.getByRole('combobox', { name: 'Date' }), '7d');

		expect(feed.filters).toEqual({ gallery: 'g1', contact: 'anonymous', range: '7d' });
		expect(load).toHaveBeenCalledTimes(3);
		expect(screen.getByRole('option', { name: 'Untitled gallery' })).toBeTruthy();
		expect(screen.getByRole('option', { name: 'Anonymous client' })).toBeTruthy();
	});

	it('picks a kind and resets filters independently of sort', async () => {
		const { feed } = setup();
		const user = userEvent.setup();

		await user.click(screen.getByRole('button', { name: 'Filters' }));
		await user.selectOptions(screen.getByRole('combobox', { name: 'Type' }), 'open');
		expect(feed.filters.kind).toBe('open');
		expect(screen.getByRole('button', { name: 'Filters: Views' }).textContent).toContain('1');

		await user.click(screen.getByRole('button', { name: 'Sort by: Newest first' }));
		await user.click(screen.getByRole('menuitemradio', { name: 'Oldest first' }));
		await user.click(screen.getByRole('button', { name: 'Reset' }));
		expect(feed.filters.kind).toBeUndefined();
		expect(feed.sort).toBe('oldest');
	});

	it('leaves the gallery choice out of a gallery-scoped feed', async () => {
		setup({});

		await userEvent.click(screen.getByRole('button', { name: 'Filters' }));
		expect(screen.queryByRole('combobox', { name: 'Gallery' })).toBeNull();
		expect(screen.getByRole('combobox', { name: 'Visitor' })).toBeTruthy();
	});

	it('has no axe violations', async () => {
		const feed = new ActivityFeed({ fetcher: vi.fn() as unknown as typeof fetch });
		const { container } = render(ActivityFilters, { feed, galleries, contacts });

		expect((await axe(container)).violations).toEqual([]);
	});
});
