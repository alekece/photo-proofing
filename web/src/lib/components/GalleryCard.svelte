<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Lock } from '@lucide/svelte';
	import SelectionDisc from './SelectionDisc.svelte';
	import CoverBackdrop from './CoverBackdrop.svelte';
	import GalleryFavorite from './GalleryFavorite.svelte';
	import Card from '#lib/components/ui/Card/Card.svelte';
	import StatusChip from '#lib/components/ui/StatusChip/StatusChip.svelte';
	import type { GalleryOverview } from '#lib/gallery/api.js';
	import {
		galleryExpiryDescription,
		galleryStatus,
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

	let status = $derived(galleryStatus(gallery.states));
	let title = $derived(gallery.title ?? m.gallery_untitled());
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

<li
	class="group relative isolate min-w-0 transition-transform motion-reduce:transition-none {selected
		? 'scale-[0.975]'
		: ''}"
>
	<a
		href={`/dashboard/${gallery.id}`}
		data-slot="gallery-card-surface"
		class="gallery-card-surface gallery-card-link relative grid overflow-hidden text-foreground no-underline focus-visible:outline-2 focus-visible:outline-focus"
	>
		<div
			data-slot="gallery-card-image"
			class="gallery-card-image relative aspect-square bg-[repeating-linear-gradient(45deg,var(--border)_0_10px,var(--panel)_10px_20px)] sm:aspect-[4/3]"
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
		<div data-slot="gallery-card-meta" class="gallery-card-meta min-w-0">
			<div
				data-slot="gallery-card-status"
				class="gallery-status pointer-events-none relative z-[3] flex w-fit max-w-full flex-wrap items-center gap-1"
			>
				<StatusChip
					{status}
					workspace
					title={status === 'expired' ? galleryExpiryDescription(gallery.expires_at) : undefined}
				/>
				{#if gallery.access_code || gallery.favorite}
					<span class="inline-flex items-center gap-1">
						{#if gallery.access_code}
							<StatusChip
								aria-label={m.card_access_code_protected()}
								title={m.card_access_code_protected()}
							>
								<Lock class="size-3.5" aria-hidden="true" />
							</StatusChip>
						{/if}
						{#if gallery.favorite}
							<GalleryFavorite favorite label={m.card_favorited()} animate={animateStar} />
						{/if}
					</span>
				{/if}
			</div>
			<Card
				dataSlot="gallery-card-caption"
				surface="glass"
				class="gallery-caption pointer-events-none media-caption"
			>
				<h2 {title} class="media-caption-title {gallery.title === null ? 'opacity-70' : ''}">
					{#each highlightParts(title, query) as part, index (index)}{#if part.match}<mark
								class="rounded-sm bg-hero/20 text-inherit"
							>
								{part.text}
							</mark>{:else}{part.text}{/if}{/each}
				</h2>
				<p class="media-caption-summary">
					{#each summary as part, index (index)}{#if part.over}<b class="font-semibold text-danger">
								{part.text}
							</b>{:else}{part.text}{/if}{/each}
				</p>
			</Card>
		</div>
	</a>
	{#if onselect}
		<SelectionDisc
			label={m.card_select_gallery({ title })}
			{selected}
			visible={selecting}
			side="left"
			onclick={onselect}
		/>
	{/if}
</li>

<style>
	.gallery-card-surface {
		grid-template-columns: minmax(0, 1fr);
	}
	.gallery-card-image {
		grid-column: 1;
		grid-row: 1;
	}
	:global([data-studio-mode='full_image']) .gallery-card-meta {
		z-index: 2;
		display: grid;
		grid-row: 1;
		grid-column: 1;
		align-self: end;
		gap: 0.5rem;
		margin: 0.5rem;
	}
	:global([data-studio-mode='full_image']) .gallery-status {
		max-width: calc(100% - 2.75rem);
	}
	:global([data-studio-mode='print'] .gallery-caption) {
		position: relative;
		inset: auto;
		border: 0;
		background: var(--panel);
		color: var(--foreground);
		text-shadow: none;
		box-shadow: none;
		backdrop-filter: none;
		-webkit-backdrop-filter: none;
		grid-row: 2;
		grid-column: 1;
		align-self: stretch;
		padding: 0.75rem 0.125rem 0.25rem;
	}
	:global([data-studio-mode='print'] .gallery-card-surface) {
		grid-template-rows: auto 1fr;
		height: 100%;
		padding: 0.375rem;
		background: var(--panel);
	}
	:global([data-studio-mode='print']) .gallery-status {
		z-index: 3;
		align-self: end;
		justify-self: start;
		grid-row: 1;
		grid-column: 1;
		max-width: calc(100% - 3.25rem);
		margin: 0.5rem 2.75rem 0.5rem 0.5rem;
	}
	:global([data-studio-mode='print']) .gallery-card-meta {
		display: contents;
	}
	:global([data-studio-mode='full_image'] .gallery-caption) {
		border-radius: var(--radius-card);
		color: white;
	}
	h2 {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	:global([data-studio-mode='full_image'] .gallery-caption b) {
		color: var(--color-warning-on-dark);
	}
</style>
