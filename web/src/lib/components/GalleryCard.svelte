<script lang="ts">
	import Placeholder from '#lib/components/Placeholder.svelte';
	import StatusChip from '#lib/components/ui/status-chip/status-chip.svelte';
	import type { Gallery } from '#lib/gallery/api.js';
	import {
		galleryStatus,
		gallerySummary,
		highlightParts,
		journeyProgress,
		UNTITLED_GALLERY
	} from '#lib/gallery/format.js';
	import { previewUrl } from '#lib/gallery/photos.js';

	let {
		gallery,
		query = '',
		news
	}: {
		gallery: Gallery;
		/** The search text to highlight in the title. */
		query?: string;
		/** "New selection" or "New downloads"; set once #72 reports unread activity. */
		news?: string;
	} = $props();

	let status = $derived(galleryStatus(gallery.states));
	let title = $derived(gallery.title ?? UNTITLED_GALLERY);
</script>

<li class="group relative aspect-[4/5] overflow-hidden rounded-card border border-border bg-panel">
	<a
		href={`/dashboard/${gallery.id}`}
		class="absolute inset-0 block text-inverse no-underline outline-none focus-visible:outline-3 focus-visible:-outline-offset-3 focus-visible:outline-focus"
	>
		<div class="absolute inset-0">
			{#if gallery.hero_id}
				<img
					src={previewUrl(gallery.id, gallery.hero_id)}
					alt=""
					loading="lazy"
					class="size-full object-cover transition-transform duration-500 ease-pop group-hover:scale-105"
				/>
			{:else}
				<Placeholder background="sheet" icon label="Your hero photo will appear here" />
			{/if}
		</div>
		<div class="absolute inset-0 bg-gradient-to-t from-dark/85 via-dark/20 to-transparent"></div>
		<div class="absolute inset-x-3 top-3 flex flex-wrap items-start justify-between gap-2">
			<StatusChip {status} dot class="bg-panel" />
			{#if news}
				<span class="rounded-full bg-hero px-2.5 py-0.5 text-xs font-semibold text-dark"
					>{news}</span
				>
			{/if}
		</div>
		<div class="absolute inset-x-0 bottom-0 grid gap-1.5 p-4">
			<h2
				class="brand truncate text-xl {gallery.title === null ? 'text-inverse/70' : 'text-inverse'}"
			>
				{#each highlightParts(title, query) as part, index (index)}{#if part.match}<mark
							class="rounded-sm bg-hero/80 text-dark">{part.text}</mark
						>{:else}{part.text}{/if}{/each}
			</h2>
			<p class="text-sm text-inverse/85">{gallerySummary(gallery)}</p>
			<div
				data-slot="journey-progress"
				aria-hidden="true"
				class="mt-1 h-1 overflow-hidden rounded-full bg-inverse/25"
			>
				<div
					class="h-full rounded-full {status === 'expired' ? 'bg-danger' : 'bg-inverse'}"
					style:width="{journeyProgress(status) * 100}%"
				></div>
			</div>
		</div>
	</a>
</li>
