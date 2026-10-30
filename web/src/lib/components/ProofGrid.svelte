<script lang="ts">
	import type { Proof } from '../gallery/types';
	import ProofThumbnail from './ProofThumbnail.svelte';

	let {
		proofs,
		layout,
		onopen
	}: { proofs: Proof[]; layout: 'grid' | 'masonry'; onopen: (index: number) => void } = $props();
</script>

{#if proofs.length === 0}
	<p>No proofs are available to view.</p>
{:else}
	<ul class="grid" class:masonry={layout === 'masonry'} data-layout={layout} aria-label="Proofs">
		{#each proofs as proof, i (proof.id)}
			<li>
				<ProofThumbnail {proof} position={i + 1} total={proofs.length} onopen={() => onopen(i)} />
			</li>
		{/each}
	</ul>
{/if}

<style>
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(min(100%, 14rem), 1fr));
		gap: 0.75rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	/* CSS columns keep DOM order as the reading order. */
	.masonry {
		display: block;
		column-width: 14rem;
		column-gap: 0.75rem;
	}
	.masonry li {
		break-inside: avoid;
		margin-bottom: 0.75rem;
	}
</style>
