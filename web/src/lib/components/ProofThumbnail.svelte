<script lang="ts">
	import type { Proof } from '../gallery/types';

	let {
		proof,
		layout,
		position,
		total,
		onopen
	}: {
		proof: Proof;
		layout: 'grid' | 'masonry';
		position: number;
		total: number;
		onopen: () => void;
	} = $props();
</script>

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
