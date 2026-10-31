<script lang="ts">
	import type { Proof } from '../gallery/types';

	let { proof, position, total, onopen }: {
		proof: Proof;
		position: number;
		total: number;
		onopen: () => void;
	} = $props();
</script>

<button type="button" class="thumb" onclick={onopen} aria-label="Open proof {position} of {total}">
	<img src={proof.previewUrl} alt={proof.alt} loading="lazy" />
</button>

<style>
	.thumb {
		display: block;
		width: 100%;
		padding: 0;
		border: 0;
		border-radius: 0.25rem;
		background: none;
		cursor: zoom-in;
		overflow: hidden;
	}
	.thumb:focus-visible {
		outline: 3px solid var(--color-focus);
		outline-offset: 2px;
	}
	img {
		display: block;
		width: 100%;
		aspect-ratio: 3 / 2;
		object-fit: cover;
	}
</style>
