<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { slide } from 'svelte/transition';
	import { m } from '#lib/paraglide/messages.js';
	import {
		Activity,
		ChevronDown,
		Download,
		Eye,
		Heart,
		HeartOff,
		LoaderCircle,
		Mail,
		MessageSquare
	} from '@lucide/svelte';
	import type { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
	import type { ActivityEvent, ActivityKind } from '#lib/activityLog/api.js';
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

	const icons: Record<ActivityKind, typeof Eye> = {
		open: Eye,
		favorite: Heart,
		unfavorite: HeartOff,
		comment: MessageSquare,
		download: Download,
		email: Mail
	};
	// Translation chooses where the escaped actor goes, including after the verb.
	const actorMarker = '\uE000';
	const instanceId = $props.id();
	let photoLimit = $state(10);
	let expandedIds = $state(new Set<string>());
	let timeline = $state<HTMLDivElement>();
	let sentinel = $state<HTMLDivElement>();

	onMount(() => {
		if (typeof matchMedia !== 'function') return;

		const phone = matchMedia('(width < 40rem)');
		const measure = () => (photoLimit = phone.matches ? 6 : 10);
		measure();
		phone.addEventListener('change', measure);

		return () => phone.removeEventListener('change', measure);
	});

	$effect(() => {
		const displayed = new Set(feed.events.map(({ id }) => id));
		const kept = [...expandedIds].filter((id) => displayed.has(id));

		if (kept.length !== expandedIds.size) expandedIds = new Set(kept);
	});

	function togglePhotos(id: string) {
		expandedIds = new Set(
			expandedIds.has(id)
				? [...expandedIds].filter((expanded) => expanded !== id)
				: [...expandedIds, id]
		);
	}

	$effect(() => {
		const target = sentinel;
		const current = feed;
		const cursor = current.canPage && current.paging === 'idle' ? current.next : null;

		if (!target || cursor === null || typeof IntersectionObserver !== 'function') return;

		let active = true;
		const observer = new IntersectionObserver(
			(entries) => {
				if (
					active &&
					target === sentinel &&
					current === feed &&
					current.canPage &&
					current.paging === 'idle' &&
					current.next === cursor &&
					entries.some((entry) => entry.target === target && entry.isIntersecting)
				)
					void current.loadMore();
			},
			{ root: null, rootMargin: '0px 0px 400px 0px' }
		);
		observer.observe(target);

		return () => {
			active = false;
			observer.disconnect();
		};
	});

	async function retryPage() {
		const target = sentinel;
		const current = feed;

		if (!target || !current.canPage || current.paging !== 'failed') return;

		if (target.contains(document.activeElement)) target.focus({ preventScroll: true });
		await current.loadMore();
		await tick();

		if (target !== sentinel || current !== feed || document.activeElement !== target) return;

		(target.querySelector('button') ?? timeline)?.focus({ preventScroll: true });
	}

	function wording(event: ActivityEvent, actor: string): string {
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
	<div
		class="mb-3 flex flex-wrap items-center justify-between gap-2 rounded-control border border-danger/40 px-3 py-2 text-sm text-danger"
	>
		<p role="alert">{m.activity_error()}</p>
		{#if feed.needsReload}
			<Button variant="outline" onclick={() => void feed.load()}>{m.action_retry()}</Button>
		{/if}
	</div>
{/if}

{#if !feed.loaded}
	{#if !feed.failed}
		<p role="status" class="flex items-center gap-2 py-8 text-sm text-muted-foreground">
			<LoaderCircle size={16} class="motion-safe:animate-spin" aria-hidden="true" />
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
	{#snippet thumbnail(event: ActivityEvent, photoId: string, size: string)}
		<img
			src={thumbnailUrl(event.gallery_id, photoId)}
			alt={m.activity_photo_alt()}
			loading="lazy"
			class="{size} rounded-md border border-border object-cover"
		/>
	{/snippet}

	<div bind:this={timeline} bind:clientHeight={resultsHeight} tabindex="-1" class="grid gap-6">
		{#each days as day (day.heading)}
			<section class="grid gap-2.5">
				<h3 class="text-sm font-semibold tracking-wide text-muted-foreground">{day.heading}</h3>
				<ol class="grid gap-2.5">
					{#each day.events as event (event.id)}
						{@const Icon = icons[event.kind]}
						{@const actor = event.contact?.name ?? m.activity_anonymous()}
						{@const [before, after] = wording(event, actorMarker).split(actorMarker)}
						{@const isSelection = event.kind === 'favorite' || event.kind === 'unfavorite'}
						{@const photos = isSelection ? [...new Set(event.photo_ids)] : event.photo_ids}
						{@const isExpanded = expandedIds.has(event.id)}
						{@const photoGridId = `${instanceId}-photos-${event.id}`}
						<li class="grid gap-2 rounded-xl border border-border bg-panel px-4 py-3">
							<div class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
								<div class="flex min-w-0 items-center gap-2">
									<Icon size={16} class="shrink-0 text-muted-foreground" aria-hidden="true" />
									<p class="text-[0.9375rem] wrap-anywhere">
										{before}<strong>{actor}</strong>{after}
									</p>
								</div>
								<span class="text-xs text-muted-foreground">{timeAgo(event.last_at)}</span>
							</div>
							{#if showGallery}
								<a
									href="/dashboard/{encodeURIComponent(event.gallery_id)}"
									class="w-fit text-sm text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
									>{event.gallery_title ?? m.gallery_untitled()}</a
								>
							{/if}
							{#if isSelection && photos.length > 0}
								<div
									id={photoGridId}
									class="grid w-full max-w-64 grid-cols-3 gap-1.5 sm:max-w-80 sm:grid-cols-5"
								>
									{#each photos.slice(0, photoLimit) as photoId (photoId)}
										{@render thumbnail(event, photoId, 'aspect-square w-full')}
									{/each}
									{#if isExpanded && photos.length > photoLimit}
										<div
											class="col-span-full grid grid-cols-subgrid gap-1.5"
											transition:slide={{
												duration: globalThis.matchMedia?.('(prefers-reduced-motion: reduce)')
													.matches
													? 0
													: 200
											}}
										>
											{#each photos.slice(photoLimit) as photoId (photoId)}
												{@render thumbnail(event, photoId, 'aspect-square w-full')}
											{/each}
										</div>
									{/if}
								</div>
								{#if photos.length > photoLimit}
									<Button
										variant="ghost"
										class="w-11 justify-self-start p-0"
										aria-label={isExpanded ? m.activity_hide_photos() : m.activity_show_photos()}
										aria-expanded={isExpanded}
										aria-controls={photoGridId}
										onclick={() => togglePhotos(event.id)}
									>
										<ChevronDown
											size={18}
											aria-hidden="true"
											class={isExpanded ? 'rotate-180' : ''}
										/>
									</Button>
								{/if}
							{:else if photos.length > 0}
								<div class="flex gap-1.5">
									{#each photos as photoId, index (`${photoId}:${index}`)}
										{@render thumbnail(event, photoId, 'size-14')}
									{/each}
								</div>
							{/if}
						</li>
					{/each}
				</ol>
			</section>
		{/each}
		<div bind:this={sentinel} tabindex="-1" class="-mt-6 min-h-px">
			{#if feed.canPage && feed.paging === 'loading'}
				<p
					role="status"
					class="flex items-center justify-center gap-2 pt-4 text-sm text-muted-foreground"
				>
					<LoaderCircle size={16} class="motion-safe:animate-spin" aria-hidden="true" />
					{m.activity_loading()}
				</p>
			{:else if feed.canPage && feed.paging === 'failed'}
				<div
					class="mt-4 flex flex-wrap items-center justify-between gap-2 rounded-control border border-danger/40 px-3 py-2 text-sm text-danger"
				>
					<p role="alert">{m.activity_page_error()}</p>
					<Button variant="outline" onclick={() => void retryPage()}>{m.action_retry()}</Button>
				</div>
			{/if}
		</div>
	</div>
{/if}
