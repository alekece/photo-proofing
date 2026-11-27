<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { ArrowUpDown, ListFilter } from '@lucide/svelte';
	import type { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
	import type { ActivityKind, ActivityRange } from '#lib/activityLog/api.js';
	import ToolbarMenu from '#lib/components/ui/ToolbarMenu/ToolbarMenu.svelte';
	import WorkspaceToolbar from '#lib/components/ui/WorkspaceToolbar/WorkspaceToolbar.svelte';

	let {
		feed,
		searchLabel,
		resultsHeight = 0
	}: {
		feed: ActivityFeed;
		searchLabel: string;
		resultsHeight?: number;
	} = $props();

	const kinds = $derived<{ value: ActivityKind | ''; label: string }[]>([
		{ value: '', label: m.activity_type_all() },
		{ value: 'favorite', label: m.activity_type_favorites() },
		{ value: 'comment', label: m.activity_type_comments() },
		{ value: 'download', label: m.activity_type_downloads() },
		{ value: 'open', label: m.activity_type_views() },
		{ value: 'email', label: m.activity_type_emails() },
		{ value: 'email_failed', label: m.activity_type_failed_emails() }
	]);
	const sorts = $derived([
		{ value: 'newest', label: m.activity_sort_newest() },
		{ value: 'oldest', label: m.activity_sort_oldest() }
	] as const);
	const ranges = $derived<{ value: ActivityRange | ''; label: string }[]>([
		{ value: '', label: m.activity_range_any() },
		{ value: 'today', label: m.activity_range_today() },
		{ value: '7d', label: m.activity_range_7d() },
		{ value: '30d', label: m.activity_range_30d() }
	]);

	const choices = $derived([
		{ key: 'range', label: m.activity_filter_date(), options: ranges },
		{ key: 'kind', label: m.activity_filter_type(), options: kinds }
	] as const);

	function choose(key: 'range' | 'kind', value: string) {
		const next = { ...feed.filters };

		if (value) Object.assign(next, { [key]: value });
		else delete next[key];

		feed.filters = next;
		void feed.load();
	}
</script>

<WorkspaceToolbar
	class="mb-5"
	resultsHeight={feed.loading ? 0 : resultsHeight}
	search={{ label: searchLabel, value: feed.search, oninput: (value) => feed.setSearch(value) }}
>
	{#snippet filters()}
		{#each choices as { key, label, options } (key)}
			<ToolbarMenu
				{label}
				summary={(feed.filters[key] &&
					options.find((option) => option.value === feed.filters[key])?.label) ||
					label}
				icon={ListFilter}
				value={feed.filters[key] ?? ''}
				{options}
				onchange={(value) => choose(key, value)}
			/>
		{/each}
	{/snippet}
	{#snippet sort()}
		<ToolbarMenu
			label={m.sort_by()}
			summary={sorts[feed.sort === 'newest' ? 0 : 1].label}
			icon={ArrowUpDown}
			value={feed.sort}
			options={sorts}
			onchange={(value) => {
				feed.sort = value;
				void feed.load();
			}}
		/>
	{/snippet}
</WorkspaceToolbar>
