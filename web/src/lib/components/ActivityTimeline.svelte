<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Activity, LoaderCircle } from '@lucide/svelte';
	import type { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
	import type { ActivityEvent } from '#lib/activityLog/api.js';
	import { dayHeading } from '#lib/activityLog/format.js';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import { timeAgo } from '#lib/gallery/notifications.svelte.js';
	import { thumbnailUrl } from '#lib/gallery/photos.js';

	let {
		feed,
		filtered = false,
		showGallery = true,
		resultsHeight = $bindable(0),
		onclear
	}: {
		feed: ActivityFeed;
		/** Whether filters are narrowing the list, which changes what an empty list means. */
		filtered?: boolean;
		/** Gallery name and link on each line; a gallery's own feed leaves them out. */
		showGallery?: boolean;
		resultsHeight?: number;
		onclear?: () => void;
	} = $props();

	function wording(event: ActivityEvent): string {
		const actor = event.contact?.name ?? m.activity_anonymous();
		const count = event.count;

		switch (event.kind) {
			case 'open':
				return m.activity_open({ actor, count });
			case 'favorite':
				return m.activity_favorite({ actor, count });
			case 'unfavorite':
				return m.activity_unfavorite({ actor, count });
			case 'comment':
				return m.activity_comment({ actor, count });
			case 'download':
				return count === 0
					? m.activity_download_zip({ actor })
					: m.activity_download({ actor, count });
			case 'email':
				return m.activity_email({ actor });
		}
	}

	const days = $derived.by(() => {
		const groups: { heading: string; events: ActivityEvent[] }[] = [];

		for (const event of feed.events) {
			const heading = dayHeading(event.last_at);
			const last = groups.at(-1);

			if (last?.heading === heading) last.events.push(event);
			else groups.push({ heading, events: [event] });
		}

		return groups;
	});
</script>

{#if feed.failed}
	<p
		role="alert"
		class="mb-3 rounded-control border border-danger/40 px-3 py-2 text-sm text-danger"
	>
		{m.activity_error()}
	</p>
{/if}

{#if !feed.loaded}
	{#if !feed.failed}
		<p role="status" class="flex items-center gap-2 py-8 text-sm text-muted-foreground">
			<LoaderCircle size={16} class="animate-spin" aria-hidden="true" />
			{m.activity_loading()}
		</p>
	{/if}
{:else if feed.events.length === 0}
	<EmptyState variant="plain">
		<Activity size={28} aria-hidden="true" class="text-muted-foreground" />
		<h3>{filtered ? m.activity_empty_filtered() : m.activity_empty_title()}</h3>
		{#if filtered && onclear}
			<Button variant="outline" onclick={onclear}>{m.filter_reset()}</Button>
		{:else if !filtered}
			<p class="max-w-[32ch]">{m.activity_empty_body()}</p>
		{/if}
	</EmptyState>
{:else}
	<div bind:clientHeight={resultsHeight} class="grid gap-6">
		{#each days as day (day.heading)}
			<section class="grid gap-2.5">
				<h3 class="text-sm font-semibold tracking-wide text-muted-foreground">{day.heading}</h3>
				<ol class="grid gap-2.5">
					{#each day.events as event (event.id)}
						<li class="grid gap-2 rounded-xl border border-border bg-panel px-4 py-3">
							<div class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
								<p class="text-[0.9375rem]">{wording(event)}</p>
								<span class="text-xs text-muted-foreground">{timeAgo(event.last_at)}</span>
							</div>
							{#if showGallery}
								<a
									href="/dashboard/{encodeURIComponent(event.gallery_id)}"
									class="w-fit text-sm text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
									>{event.gallery_title ?? m.gallery_untitled()}</a
								>
							{/if}
							{#if event.photo_ids.length > 0}
								<div class="flex gap-1.5">
									{#each event.photo_ids as photoId (photoId)}
										<img
											src={thumbnailUrl(event.gallery_id, photoId)}
											alt={m.activity_photo_alt()}
											loading="lazy"
											class="size-14 rounded-md border border-border object-cover"
										/>
									{/each}
								</div>
							{/if}
						</li>
					{/each}
				</ol>
			</section>
		{/each}
		{#if feed.next !== null}
			<Button
				variant="outline"
				class="justify-self-center"
				disabled={feed.loading || feed.loadingMore}
				onclick={() => void feed.loadMore()}
			>
				{m.activity_load_more()}
			</Button>
		{/if}
	</div>
{/if}
