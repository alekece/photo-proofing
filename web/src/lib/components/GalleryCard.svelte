<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { EyeOff, Heart, Lock, Star } from '@lucide/svelte';
	import SelectionDisc from './SelectionDisc.svelte';
	import CoverBackdrop from './CoverBackdrop.svelte';
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
		liveUpdate = false,
		selected = false,
		selecting = false,
		onselect
	}: {
		gallery: GalleryOverview;
		/** The search text to highlight in the title. */
		query?: string;
		/** Whether the gallery has unread activity. */
		unread?: boolean;
		/** A live update, rather than a fetched baseline, may pulse a newly visible dot. */
		liveUpdate?: boolean;
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
	let previousStar: boolean | undefined;
	let previousUnread: boolean | undefined;
	let animateStar = $state(false);
	let animateUnread = $state(false);
	$effect.pre(() => {
		const star = Boolean(gallery.favorite);
		animateStar = previousStar === false && star;
		previousStar = star;
	});
	$effect.pre(() => {
		if (unread !== previousUnread) {
			animateUnread = previousUnread === false && unread && liveUpdate;
			previousUnread = unread;
		}
	});
</script>

<li class="group relative isolate min-w-0">
	<a
		href={`/dashboard/${gallery.id}`}
		class="relative block overflow-hidden text-foreground no-underline transition-transform focus-visible:outline-2 focus-visible:outline-focus motion-reduce:transition-none {selected
			? 'scale-[0.975]'
			: ''}"
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
			{:else}
				<CoverBackdrop title={gallery.title} compact />
			{/if}
			{#if unread}
				<span
					role="img"
					aria-label={m.card_unread_activity()}
					title={m.card_unread_activity()}
					class="absolute top-2 right-2 size-3 rounded-full border border-panel bg-danger sm:top-3 sm:right-3 {animateUnread
						? 'motion-safe:animate-heart-pop'
						: ''}"
				></span>
			{/if}
		</div>
		<div class="gallery-caption relative grid gap-1.5 bg-panel px-3 py-2.5 sm:px-4 sm:py-4">
			<div class="gallery-status flex items-center gap-2">
				<StatusChip {status} dot class="w-fit border-0 bg-panel px-2 text-foreground" />
				{#if gallery.access_code || closed}
					<span class="inline-flex items-center gap-1.5">
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
					</span>
				{/if}
			</div>
			<h2
				{title}
				class="min-w-0 font-serif text-lg leading-tight wrap-anywhere sm:text-2xl {gallery.title ===
				null
					? 'opacity-70'
					: ''}"
			>
				{#each highlightParts(title, query) as part, index (index)}{#if part.match}<mark
							class="rounded-sm bg-hero/20 text-inherit">{part.text}</mark
						>{:else}{part.text}{/if}{/each}
			</h2>
			<p class="flex flex-wrap items-center gap-x-1 pr-5 text-xs sm:text-sm">
				{#if gallery.favorite_count > 0}<Heart size={12} aria-hidden="true" />{/if}
				{#each summary as part, index (index)}{#if part.over}<b
							class="text-base leading-none font-extrabold text-danger">{part.text}</b
						>{:else}{part.text}{/if}{/each}
			</p>
			{#if gallery.favorite}
				<span
					role="img"
					aria-label={m.card_favorited()}
					title={m.card_favorited()}
					class="absolute right-3 bottom-3"
				>
					<Star
						class="size-4 fill-current text-hero {animateStar
							? 'motion-safe:animate-heart-pop'
							: ''}"
						aria-hidden="true"
					/>
				</span>
			{/if}
		</div>
	</a>
	{#if onselect}
		<SelectionDisc
			label={m.card_select_gallery({ title })}
			{selected}
			visible={selecting}
			touchLabel
			onclick={onselect}
		/>
	{/if}
</li>

<style>
	:global([data-studio-mode='print']) .gallery-caption {
		border: 1px solid var(--border);
		border-top: 0;
	}
	:global([data-studio-mode='print']) .gallery-status {
		position: absolute;
		bottom: calc(100% + 0.5rem);
		left: 0.75rem;
		max-width: calc(100% - 3.25rem);
		flex-wrap: wrap;
		row-gap: 0.125rem;
		background: var(--panel);
		border-radius: var(--radius-control);
	}
	:global([data-studio-mode='print']) .gallery-caption > [role='img'] {
		bottom: calc(100% + 0.5rem);
	}
	:global([data-studio-mode='full_image']) .gallery-caption {
		position: absolute;
		inset: auto 0 0;
		color: white;
		padding-top: 2rem;
		background: linear-gradient(transparent, rgb(0 0 0 / 0.72) 40%, rgb(0 0 0 / 0.85));
	}
	:global([data-studio-mode='full_image']) h2 {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	:global([data-studio-mode='full_image']) .gallery-caption b {
		color: var(--color-warning-on-dark);
	}
</style>
