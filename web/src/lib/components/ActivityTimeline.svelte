<script module lang="ts">
	/** The tab, and for a comment the photo whose thread, that an Activity line leads to. */
	export type Destination = { section?: string; thread?: string };
</script>

<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { m } from '#lib/paraglide/messages.js';
	import {
		Activity,
		Download,
		Eye,
		Heart,
		HeartOff,
		LoaderCircle,
		Mail,
		MessageSquare,
		RotateCw
	} from '@lucide/svelte';
	import type { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
	import type { ActivityEvent, ActivityKind } from '#lib/activityLog/api.js';
	import { dayHeading } from '#lib/activityLog/format.js';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { thumbnailUrl } from '#lib/gallery/photos.js';

	let {
		feed,
		filtered = false,
		showGallery = true,
		resultsHeight = $bindable(0),
		onclear,
		onopen
	}: {
		feed: ActivityFeed;
		/** Whether filters are narrowing the list, which changes what an empty list means. */
		filtered?: boolean;
		/** Gallery name and link on each line; a gallery's own feed leaves them out. */
		showGallery?: boolean;
		resultsHeight?: number;
		onclear?: () => void;
		/** Takes a line's destination over from its link, for a page that is already showing the gallery. */
		onopen?: (place: Destination) => void;
	} = $props();

	const icons: Record<ActivityKind, typeof Eye> = {
		open: Eye,
		favorite: Heart,
		unfavorite: HeartOff,
		comment: MessageSquare,
		download: Download,
		email: Mail,
		email_failed: Mail
	};
	// Translation chooses where the escaped actor goes, including after the verb.
	const actorMarker = '\uE000';
	let photoLimit = $state(10);
	let openPhotoEvent = $state<string>();
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
		if (openPhotoEvent && !feed.events.some(({ id }) => id === openPhotoEvent)) {
			openPhotoEvent = undefined;
		}
	});

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
			case 'email_failed':
				return event.contact
					? m.activity_email_failed({ actor })
					: m.activity_email_failed_private();
		}
	}

	/** Where a line leads: the tab of the gallery that holds what it describes. */
	function destination(event: ActivityEvent): Destination | null {
		switch (event.kind) {
			case 'favorite':
			case 'unfavorite':
				return { section: 'favorites' };
			case 'comment':
				return { section: 'comments', thread: event.photo_ids[0] };
			case 'download':
				return { section: 'final' };
			case 'email':
				return { section: 'contacts' };
			case 'email_failed':
				return null;
			case 'open':
				return showGallery ? {} : null;
		}
	}

	function hrefOf(event: ActivityEvent, place: Destination): string {
		const query = [
			place.section && `section=${encodeURIComponent(place.section)}`,
			place.thread && `thread=${encodeURIComponent(place.thread)}`
		].filter(Boolean);
		const search = query.length > 0 ? `?${query.join('&')}` : '';

		return `/dashboard/${encodeURIComponent(event.gallery_id)}${search}`;
	}

	/** Opens the destination in place when the page can, as the same address would not navigate. */
	function follow(click: MouseEvent, place: Destination) {
		if (!onopen || click.defaultPrevented || click.button !== 0) return;
		if (click.metaKey || click.ctrlKey || click.shiftKey || click.altKey) return;

		click.preventDefault();
		onopen(place);
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

<p aria-live="polite" aria-atomic="true" class="sr-only">
	{#if feed.loaded && feed.filters.q && !feed.loading && !feed.failed}
		{feed.events.length === 0 ? m.activity_search_no_matches() : m.activity_search_updated()}
	{/if}
</p>

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
	<EmptyState
		variant="plain"
		icon={Activity}
		title={filtered ? m.activity_empty_filtered() : m.activity_empty_title()}
		description={filtered ? undefined : m.activity_empty_body()}
	>
		{#if filtered && onclear}
			<Button variant="outline" onclick={onclear}>{m.filter_reset()}</Button>
		{/if}
	</EmptyState>
{:else}
	{#snippet thumbnail(event: ActivityEvent, photoId: string)}
		<img
			src={thumbnailUrl(event.gallery_id, photoId)}
			alt={m.activity_photo_alt()}
			loading="lazy"
			class="size-14 shrink-0 rounded-md border border-border object-cover"
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
						{@const photos =
							isSelection || event.kind === 'comment'
								? [...new Set(event.photo_ids)]
								: event.photo_ids}
						{@const place = destination(event)}
						{@const href = place ? hrefOf(event, place) : null}
						<li
							class="relative grid gap-2 rounded-xl border border-border bg-panel px-4 py-3 transition-colors motion-reduce:transition-none {href
								? 'hover:border-border-strong'
								: ''}"
						>
							<div
								class="grid items-baseline gap-x-2 sm:gap-x-3 {showGallery
									? 'grid-cols-[auto_minmax(0,1fr)_fit-content(40%)]'
									: 'grid-cols-[auto_minmax(0,1fr)]'}"
							>
								<Icon
									size={16}
									class="relative top-0.5 self-start {event.kind === 'favorite'
										? 'text-danger'
										: 'text-foreground'}"
									fill={event.kind === 'favorite' ? 'currentColor' : 'none'}
									aria-hidden="true"
								/>
								<p class="text-[0.9375rem] wrap-anywhere">
									{#if href}
										<a
											{href}
											onclick={(click) => place && follow(click, place)}
											class="after:absolute after:inset-0 after:rounded-xl after:content-[''] focus-visible:outline-none focus-visible:after:outline-2 focus-visible:after:outline-focus"
										>
											{before}{#if after !== undefined}<strong>{actor}</strong>{after}{/if}
										</a>
									{:else}
										{before}{#if after !== undefined}<strong>{actor}</strong>{after}{/if}
									{/if}
									{#if event.kind === 'email_failed' && event.can_retry}
										<Button
											size="sm"
											variant="ghost"
											class="relative z-10 ml-1 size-7 min-h-7! min-w-7! px-0 align-middle text-muted-foreground hover:text-foreground"
											aria-label={m.action_retry()}
											title={m.action_retry()}
											disabled={feed.retrying.has(event.id)}
											onclick={() =>
												feed.retry(event).catch(() => toasts.error(m.activity_retry_failed()))}
										>
											{#if feed.retrying.has(event.id)}
												<LoaderCircle
													size={15}
													class="animate-spin motion-reduce:animate-none"
													aria-hidden="true"
												/>
											{:else}
												<RotateCw size={15} aria-hidden="true" />
											{/if}
										</Button>
									{/if}
								</p>
								{#if showGallery}
									<a
										href="/dashboard/{encodeURIComponent(event.gallery_id)}"
										class="relative z-10 min-w-0 truncate pill border border-border bg-background text-xs font-medium text-foreground hover:bg-border/60"
									>
										{event.gallery_title ?? m.gallery_untitled()}
									</a>
								{/if}
							</div>
							{#if event.kind === 'comment' && photos.length > 0}
								<div class="flex items-center gap-3">
									{@render thumbnail(event, photos[0])}
									{#if event.detail}
										<p class="min-w-0 text-sm wrap-anywhere text-muted-foreground italic">
											«&nbsp;{event.detail}&nbsp;»
										</p>
									{/if}
								</div>
							{:else if event.kind === 'comment' && event.detail}
								<p class="text-sm wrap-anywhere text-muted-foreground italic">
									«&nbsp;{event.detail}&nbsp;»
								</p>
							{/if}
							{#if isSelection && photos.length > 0}
								<div class="relative flex w-fit max-w-full flex-wrap items-center gap-1.5">
									{#each photos.slice(0, photoLimit) as photoId (photoId)}
										{@render thumbnail(event, photoId)}
									{/each}
									{#if photos.length > photoLimit}
										{@const remaining = photos.slice(photoLimit)}
										{@const label = m.activity_more_photos({ count: remaining.length })}
										<Popover
											bind:open={
												() => openPhotoEvent === event.id,
												(open) => {
													if (open) openPhotoEvent = event.id;
													else if (openPhotoEvent === event.id) openPhotoEvent = undefined;
												}
											}
											align="end"
											placement="auto"
											{label}
											aria-haspopup={false}
											class="overflow-count z-10 max-sm:size-11 max-sm:p-0 sm:icon-action"
											rootClass="static shrink-0"
											panelClass="w-72 min-w-0 max-w-full p-2 motion-reduce:animate-none"
										>
											{#snippet trigger()}+{remaining.length}{/snippet}
											<!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users must be able to scroll the photo grid.) -->
											<div
												role="region"
												aria-label={label}
												tabindex="0"
												class="flex max-h-[min(16rem,var(--popover-space,50dvh))] flex-wrap gap-1.5 overflow-auto rounded-sm focus-visible:outline-2 focus-visible:outline-focus"
											>
												{#each remaining as photoId (photoId)}
													{@render thumbnail(event, photoId)}
												{/each}
											</div>
										</Popover>
									{/if}
								</div>
							{:else if event.kind !== 'comment' && photos.length > 0}
								<div class="flex gap-1.5">
									{#each photos as photoId, index (`${photoId}:${index}`)}
										{@render thumbnail(event, photoId)}
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
