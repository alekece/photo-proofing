<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { ArrowUpDown, ListFilter } from '@lucide/svelte';
	import type { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
	import type { ActivityFilters, ActivityKind, ActivityRange } from '#lib/activityLog/api.js';
	import Select from '#lib/components/ui/Select/Select.svelte';
	import ToolbarMenu from '#lib/components/ui/ToolbarMenu/ToolbarMenu.svelte';
	import WorkspaceToolbar from '#lib/components/ui/WorkspaceToolbar/WorkspaceToolbar.svelte';

	let {
		feed,
		galleries,
		contacts,
		resultsHeight = 0
	}: {
		feed: ActivityFeed;
		/** The gallery choice; left out for a gallery's own feed. */
		galleries?: { id: string; title: string | null }[];
		contacts: { id: string; name: string }[];
		resultsHeight?: number;
	} = $props();

	const kinds = $derived<{ kind: ActivityKind | ''; label: string }[]>([
		{ kind: '', label: m.activity_type_all() },
		{ kind: 'favorite', label: m.activity_type_favorites() },
		{ kind: 'comment', label: m.activity_type_comments() },
		{ kind: 'download', label: m.activity_type_downloads() },
		{ kind: 'open', label: m.activity_type_views() },
		{ kind: 'email', label: m.activity_type_emails() }
	]);
	const sorts = $derived([
		{ value: 'newest', label: m.activity_sort_newest() },
		{ value: 'oldest', label: m.activity_sort_oldest() }
	] as const);
	const ranges = $derived<{ range: ActivityRange | ''; label: string }[]>([
		{ range: '', label: m.activity_range_any() },
		{ range: 'today', label: m.activity_range_today() },
		{ range: '7d', label: m.activity_range_7d() },
		{ range: '30d', label: m.activity_range_30d() }
	]);

	const activeCount = $derived(Object.keys(feed.filters).length);
	const summary = $derived(
		[
			feed.filters.gallery
				? (galleries?.find(({ id }) => id === feed.filters.gallery)?.title ?? m.gallery_untitled())
				: undefined,
			feed.filters.contact === 'anonymous'
				? m.activity_anonymous()
				: contacts.find(({ id }) => id === feed.filters.contact)?.name,
			ranges.find(({ range }) => range === feed.filters.range)?.label,
			kinds.find(({ kind }) => kind === feed.filters.kind)?.label
		]
			.filter(Boolean)
			.join(', ') || m.activity_filters()
	);

	function choose(key: keyof ActivityFilters, value: string) {
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
	onclear={activeCount ? () => feed.clearFilters() : undefined}
>
	{#snippet filters()}
		<ToolbarMenu label={m.activity_filters()} {summary} {activeCount} icon={ListFilter}>
			<div class="grid gap-3 p-2 text-sm">
				{#if galleries}
					<label class="grid gap-1">
						{m.activity_filter_gallery()}
						<Select
							class="w-full min-w-0"
							value={feed.filters.gallery ?? ''}
							onchange={(event) => choose('gallery', event.currentTarget.value)}
						>
							<option value="">{m.activity_all_galleries()}</option>
							{#each galleries as gallery (gallery.id)}
								<option value={gallery.id}>{gallery.title ?? m.gallery_untitled()}</option>
							{/each}
						</Select>
					</label>
				{/if}
				<label class="grid gap-1">
					{m.activity_filter_visitor()}
					<Select
						class="w-full min-w-0"
						value={feed.filters.contact ?? ''}
						onchange={(event) => choose('contact', event.currentTarget.value)}
					>
						<option value="">{m.activity_all_visitors()}</option>
						<option value="anonymous">{m.activity_anonymous()}</option>
						{#each contacts as contact (contact.id)}
							<option value={contact.id}>{contact.name}</option>
						{/each}
					</Select>
				</label>
				<label class="grid gap-1">
					{m.activity_filter_date()}
					<Select
						class="w-full min-w-0"
						value={feed.filters.range ?? ''}
						onchange={(event) => choose('range', event.currentTarget.value)}
					>
						{#each ranges as { range, label } (range)}
							<option value={range}>{label}</option>
						{/each}
					</Select>
				</label>
				<label class="grid gap-1">
					{m.activity_filter_type()}
					<Select
						class="w-full min-w-0"
						value={feed.filters.kind ?? ''}
						onchange={(event) => choose('kind', event.currentTarget.value)}
					>
						{#each kinds as { kind, label } (kind)}
							<option value={kind}>{label}</option>
						{/each}
					</Select>
				</label>
			</div>
		</ToolbarMenu>
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
