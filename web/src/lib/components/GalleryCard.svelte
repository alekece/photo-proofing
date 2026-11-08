<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import StatusChip from '#lib/components/ui/StatusChip/StatusChip.svelte';
	import type { GalleryOverview } from '#lib/gallery/api.js';
	import { galleryStatus, gallerySummary, highlightParts } from '#lib/gallery/format.js';
	import { previewUrl } from '#lib/gallery/photos.js';

	let {
		gallery,
		query = '',
		news
	}: {
		gallery: GalleryOverview;
		/** The search text to highlight in the title. */
		query?: string;
		/** "New selection" or "New downloads" while the gallery has unread activity. */
		news?: string;
	} = $props();

	let status = $derived(galleryStatus(gallery.states));
	let title = $derived(gallery.title ?? m.gallery_untitled());
	let faded = $derived(status === 'expired' || status === 'archived');
</script>

<li
	class="group relative aspect-[3/4] overflow-hidden rounded-card bg-[repeating-linear-gradient(45deg,#efe7dc_0_10px,#f6f0e8_10px_20px)] shadow-[0_14px_30px_-18px_rgb(43_36_32/0.6)] transition-transform duration-250 hover:-translate-y-[3px] sm:aspect-[4/5]"
>
	<a
		href={`/dashboard/${gallery.id}`}
		class="absolute inset-0 block text-panel no-underline outline-none focus-visible:outline-3 focus-visible:-outline-offset-3 focus-visible:outline-focus"
	>
		{#if gallery.hero_id}
			<img
				src={previewUrl(gallery.id, gallery.hero_id)}
				alt=""
				loading="lazy"
				class="size-full object-cover {faded ? 'brightness-90 grayscale-[0.6]' : ''}"
			/>
		{/if}
		<div
			class="absolute inset-0 bg-[linear-gradient(180deg,rgb(43_36_32/0)_40%,rgb(43_36_32/0.82)_100%)]"
		></div>
		<StatusChip
			{status}
			dot
			cover
			class="absolute top-2 right-2 px-2 py-[3px] text-[0.65625rem] sm:top-3 sm:right-3 sm:px-2.5 sm:py-1 sm:text-xs"
		/>
		<div class="absolute inset-x-0 bottom-0 px-3 py-2.5 sm:px-[1.125rem] sm:py-4">
			{#if news}
				<span
					class="mb-2 inline-block rounded-full bg-danger px-[9px] py-[3px] text-[0.6875rem] font-bold tracking-[0.02em] text-inverse"
					>{news}</span
				>
			{/if}
			<h2
				class="mb-1.5 font-serif text-[1.0625rem] leading-[1.1] sm:text-[1.4375rem] {gallery.title ===
				null
					? 'opacity-70'
					: ''}"
			>
				{#each highlightParts(title, query) as part, index (index)}{#if part.match}<mark
							class="rounded-[2px] bg-hero/55 px-px text-inherit">{part.text}</mark
						>{:else}{part.text}{/if}{/each}
			</h2>
			<p class="text-[0.71875rem] opacity-90 sm:text-[0.8125rem]">
				{#each gallerySummary(gallery) as part, index (index)}{#if part.over}<b
							class="font-extrabold text-warning-on-dark">{part.text}</b
						>{:else}{part.text}{/if}{/each}
			</p>
		</div>
	</a>
</li>
