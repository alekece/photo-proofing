<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Download } from '@lucide/svelte';
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

	const BADGE =
		'absolute right-2.5 bottom-2.5 z-[3] grid size-[34px] place-items-center rounded-full backdrop-blur-[4px] transition-[background-color,opacity] duration-200 focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus @max-[700px]:right-1.5 @max-[700px]:bottom-1.5 @max-[700px]:size-[30px]';
</script>

<div class="group relative animate-photo-in overflow-hidden rounded-[4px]">
	<button
		type="button"
		class="block w-full cursor-zoom-in focus-visible:outline-3 focus-visible:outline-offset-2! focus-visible:outline-focus!"
		onclick={onopen}
		aria-label={m.thumb_open({ position, total })}
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
	{#if proof.downloadUrl}
		<a
			href={proof.downloadUrl}
			download
			aria-label={m.thumb_download({ position })}
			class="{BADGE} bg-panel text-foreground shadow-none"
		>
			<Download size={15} aria-hidden="true" />
		</a>
	{:else if onselect || readonly}
		<FavoriteButton
			{selected}
			label={m.thumb_favorite({ position })}
			disabled={!onselect}
			onclick={onselect}
			class="{BADGE} {selected
				? 'bg-panel text-danger'
				: 'bg-[rgb(32_26_22/0.45)] text-panel'} {!onselect
				? 'cursor-not-allowed opacity-50 group-hover:opacity-50'
				: selected
					? 'cursor-pointer opacity-100'
					: 'cursor-pointer opacity-0 group-hover:opacity-100 pointer-coarse:opacity-100'} focus-visible:opacity-100"
		/>
	{/if}
</div>
