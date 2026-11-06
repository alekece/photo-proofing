<script lang="ts">
	import type { Proof } from '../gallery/types';
	import FavoriteButton from './FavoriteButton.svelte';

	let {
		proof,
		layout,
		position,
		total,
		selected = false,
		onopen,
		readonly = false,
		onselect
	}: {
		proof: Proof;
		layout: 'grid' | 'masonry';
		position: number;
		total: number;
		selected?: boolean;
		/** Shows the heart without letting it change, as in the owner's preview. */
		readonly?: boolean;
		onopen: () => void;
		/** Without it the proof cannot be selected, as once the gallery is delivered. */
		onselect?: () => void;
	} = $props();
</script>

<div class="group relative">
	<button
		type="button"
		class="block w-full cursor-zoom-in overflow-hidden rounded-sm focus-visible:outline-3 focus-visible:outline-offset-2! focus-visible:outline-focus!"
		onclick={onopen}
		aria-label="Open proof {position} of {total}"
	>
		<!-- The grid crops every tile alike; masonry keeps each photo's own shape. -->
		<img
			src={proof.thumbnailUrl}
			alt={proof.alt}
			width={proof.width}
			height={proof.height}
			loading="lazy"
			class={layout === 'grid'
				? 'block aspect-3/2 h-auto w-full object-cover'
				: 'block h-auto w-full'}
		/>
	</button>
	{#if onselect || readonly}
		<FavoriteButton
			{selected}
			label="Favorite proof {position}"
			disabled={!onselect}
			onclick={onselect}
			class="absolute top-2 right-2 grid min-h-11 min-w-11 cursor-pointer place-items-center rounded-full bg-viewer-control text-viewer-foreground group-hover:opacity-100 focus-visible:opacity-100 focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus disabled:cursor-default pointer-coarse:opacity-100 {!onselect
				? 'opacity-60'
				: selected
					? 'opacity-100'
					: 'opacity-0'}"
		/>
	{/if}
</div>
