<script lang="ts">
	import type { Proof } from '../gallery/types';

	let { proof, position, total, onopen }: {
		proof: Proof;
		position: number;
		total: number;
		onopen: () => void;
	} = $props();
</script>

<button type="button" class="block w-full cursor-zoom-in overflow-hidden rounded-sm focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus"
	onclick={onopen} aria-label="Open proof {position} of {total}">
	<img
		src={proof.previewUrl}
		alt={proof.alt}
		loading="lazy"
		class="block aspect-3/2 w-full object-cover"
	/>
</button>
