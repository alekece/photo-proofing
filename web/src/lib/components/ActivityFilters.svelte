<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
	import type { ActivityKind, ActivityRange } from '#lib/activityLog/api.js';
	import Select from '#lib/components/ui/Select/Select.svelte';
	import { cn } from '#lib/utils.js';

	let {
		feed,
		galleries,
		contacts
	}: {
		feed: ActivityFeed;
		/** The gallery choice; left out for a gallery's own feed. */
		galleries?: { id: string; title: string | null }[];
		contacts: { id: string; name: string }[];
	} = $props();

	const kinds = $derived<{ kind: ActivityKind | undefined; label: string }[]>([
		{ kind: undefined, label: m.activity_type_all() },
		{ kind: 'favorite', label: m.activity_type_favorites() },
		{ kind: 'comment', label: m.activity_type_comments() },
		{ kind: 'download', label: m.activity_type_downloads() },
		{ kind: 'open', label: m.activity_type_views() },
		{ kind: 'email', label: m.activity_type_emails() }
	]);
	const ranges = $derived<{ range: ActivityRange | ''; label: string }[]>([
		{ range: '', label: m.activity_range_any() },
		{ range: 'today', label: m.activity_range_today() },
		{ range: '7d', label: m.activity_range_7d() },
		{ range: '30d', label: m.activity_range_30d() }
	]);

	function apply(
		changes: Partial<Record<'gallery' | 'contact' | 'range', string>> & {
			kind?: ActivityKind | null;
		}
	) {
		const next = { ...feed.filters, ...changes } as Record<string, string | null | undefined>;

		feed.filters = Object.fromEntries(
			Object.entries(next).filter(([, value]) => value)
		) as typeof feed.filters;
		void feed.load();
	}

	function choose(
		event: Event & { currentTarget: HTMLSelectElement },
		key: 'gallery' | 'contact' | 'range'
	) {
		apply({ [key]: event.currentTarget.value });
	}
</script>

<div class="mb-5 flex flex-wrap items-center justify-between gap-3">
	<div class="flex flex-wrap gap-2">
		{#if galleries}
			<Select
				aria-label={m.activity_filter_gallery()}
				value={feed.filters.gallery ?? ''}
				onchange={(event) => choose(event, 'gallery')}
			>
				<option value="">{m.activity_all_galleries()}</option>
				{#each galleries as gallery (gallery.id)}
					<option value={gallery.id}>{gallery.title ?? m.gallery_untitled()}</option>
				{/each}
			</Select>
		{/if}
		<Select
			aria-label={m.activity_filter_visitor()}
			value={feed.filters.contact ?? ''}
			onchange={(event) => choose(event, 'contact')}
		>
			<option value="">{m.activity_all_visitors()}</option>
			<option value="anonymous">{m.activity_anonymous()}</option>
			{#each contacts as contact (contact.id)}
				<option value={contact.id}>{contact.name}</option>
			{/each}
		</Select>
		<Select
			aria-label={m.activity_filter_date()}
			value={feed.filters.range ?? ''}
			onchange={(event) => choose(event, 'range')}
		>
			{#each ranges as { range, label } (range)}
				<option value={range}>{label}</option>
			{/each}
		</Select>
	</div>
	<div
		role="group"
		aria-label={m.activity_filter_type()}
		class="flex flex-wrap gap-1 rounded-full border border-border-strong bg-panel p-1"
	>
		{#each kinds as { kind, label } (kind ?? 'all')}
			<button
				type="button"
				aria-pressed={feed.filters.kind === kind}
				class={cn(
					'rounded-full px-3 py-1 text-sm',
					feed.filters.kind === kind
						? 'bg-foreground font-semibold text-background'
						: 'font-medium text-muted-foreground hover:text-foreground'
				)}
				onclick={() => apply({ kind: kind ?? null })}>{label}</button
			>
		{/each}
	</div>
</div>
