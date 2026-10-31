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
	<ul
		class={layout === 'masonry'
			? 'block columns-[14rem] gap-3'
			: 'grid grid-cols-[repeat(auto-fill,minmax(min(100%,14rem),1fr))] gap-3'}
		data-layout={layout}
		aria-label="Proofs"
	>
		{#each proofs as proof, i (proof.id)}
			<li class={layout === 'masonry' ? 'mb-3 break-inside-avoid' : undefined}>
				<ProofThumbnail {proof} position={i + 1} total={proofs.length} onopen={() => onopen(i)} />
			</li>
		{/each}
	</ul>
{/if}
