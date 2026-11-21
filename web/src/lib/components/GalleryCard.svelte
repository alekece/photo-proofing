<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Check, EyeOff, Lock, Star } from '@lucide/svelte';
	import StatusChip from '#lib/components/ui/StatusChip/StatusChip.svelte';
	import type { GalleryOverview } from '#lib/gallery/api.js';
	import {
		galleryExpiryDescription,
		galleryLifecycle,
		gallerySummary,
		highlightParts
	} from '#lib/gallery/format.js';
	import { previewUrl } from '#lib/gallery/photos.js';
	import { DEFAULT_FOCAL_POINT } from '#lib/gallery/schemas.js';

	let {
		gallery,
		query = '',
		unread = false,
		selected = false,
		selecting = false,
		onselect
	}: {
		gallery: GalleryOverview;
		/** The search text to highlight in the title. */
		query?: string;
		/** Whether the gallery has unread activity. */
		unread?: boolean;
		/** Whether this card is currently selected for dashboard bulk actions. */
		selected?: boolean;
		/** Whether dashboard selection mode is active so every card shows its selection circle. */
		selecting?: boolean;
		/** Toggles this card's selection without navigating to the gallery. */
		onselect?: () => void;
	} = $props();

	let status = $derived(galleryLifecycle(gallery.states));
	let title = $derived(gallery.title ?? m.gallery_untitled());
	let expired = $derived(gallery.states.includes('expired'));
	let closed = $derived(expired || status === 'archived');
	let summary = $derived(gallerySummary(gallery));
	let focalPoint = $derived(gallery.focal_point ?? DEFAULT_FOCAL_POINT);
</script>

<li
	class="group relative min-w-0 overflow-hidden rounded-card border border-border bg-panel {selected
		? 'ring-[2.5px] ring-foreground ring-offset-2 ring-offset-background'
		: ''}"
>
	<a
		href={`/dashboard/${gallery.id}`}
		class="block h-full text-foreground no-underline outline-none focus-visible:outline-3 focus-visible:-outline-offset-3 focus-visible:outline-focus"
	>
		<div
			class="relative aspect-[4/3] bg-[repeating-linear-gradient(45deg,var(--border)_0_10px,var(--panel)_10px_20px)]"
		>
			{#if gallery.hero_id}
				<img
					src={previewUrl(gallery.id, gallery.hero_id)}
					alt=""
					loading="lazy"
					style:object-position="{focalPoint.x}% {focalPoint.y}%"
					class="absolute inset-0 size-full object-cover"
				/>
			{/if}
			{#if unread}
				<span
					role="img"
					aria-label={m.card_unread_activity()}
					title={m.card_unread_activity()}
					class="absolute top-2 right-2 size-3 rounded-full border border-panel bg-danger sm:top-3 sm:right-3"
				></span>
			{/if}
			<div
				class="absolute inset-x-2 bottom-2 flex flex-wrap items-end justify-between gap-1 sm:inset-x-3 sm:bottom-3"
			>
				<StatusChip {status} class="border-transparent bg-panel text-foreground" />
				{#if gallery.access_code || closed || gallery.favorite}
					<span class="ml-auto inline-flex items-center gap-1.5 rounded-full bg-panel px-2 py-1">
						{#if gallery.access_code}
							<span
								role="img"
								aria-label={m.card_access_code_protected()}
								title={m.card_access_code_protected()}
								><Lock class="size-3.5" aria-hidden="true" /></span
							>
						{/if}
						{#if closed}
							<span
								role="img"
								aria-label={m.card_client_access_closed()}
								title={expired
									? galleryExpiryDescription(gallery.expires_at)
									: m.card_client_access_closed()}
								><EyeOff class="size-3.5" aria-hidden="true" /></span
							>
						{/if}
						{#if gallery.favorite}
							<span role="img" aria-label={m.card_favorited()} title={m.card_favorited()}>
								<Star class="size-3.5 fill-current text-hero" aria-hidden="true" />
							</span>
						{/if}
					</span>
				{/if}
			</div>
		</div>
		<div class="grid gap-1.5 px-3 py-2.5 sm:px-4 sm:py-4">
			<h2
				class="min-w-0 font-serif text-lg leading-tight wrap-anywhere sm:text-2xl {gallery.title ===
				null
					? 'opacity-70'
					: ''}"
			>
				{#each highlightParts(title, query) as part, index (index)}{#if part.match}<mark
							class="rounded-sm bg-hero/20 text-inherit">{part.text}</mark
						>{:else}{part.text}{/if}{/each}
			</h2>
			<p class="text-xs text-muted-foreground sm:text-sm">
				{#each summary as part, index (index)}{#if part.over}<b
							class="text-base leading-none font-extrabold text-danger">{part.text}</b
						>{:else}{part.text}{/if}{/each}
			</p>
		</div>
	</a>
	{#if onselect}
		<button
			type="button"
			aria-label={m.card_select_gallery({ title })}
			aria-pressed={selected}
			onclick={onselect}
			class="absolute top-2 left-2 z-[2] grid size-[26px] cursor-pointer place-items-center rounded-full border transition-[opacity,background-color,border-color,transform] duration-150 focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus sm:top-3 sm:left-3 {selected
				? 'border-panel bg-foreground text-panel opacity-100 shadow-sm'
				: selecting
					? 'border-white/75 bg-black/35 text-transparent opacity-100 backdrop-blur-[4px] hover:border-white hover:bg-black/50'
					: 'border-white/75 bg-black/35 text-transparent opacity-100 backdrop-blur-[4px] hover:border-white hover:bg-black/50 sm:opacity-0 sm:group-focus-within:opacity-100 sm:group-hover:opacity-100'}"
		>
			<Check size={14} strokeWidth={2.75} aria-hidden="true" />
		</button>
	{/if}
</li>
