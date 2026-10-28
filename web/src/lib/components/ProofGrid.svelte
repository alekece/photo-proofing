<script lang="ts">
	import type { Proof } from '../gallery/types';
	import ProofThumbnail from './ProofThumbnail.svelte';

	let { proofs, onopen }: { proofs: Proof[]; onopen: (index: number) => void } = $props();
</script>

{#if proofs.length === 0}
	<p>No proofs are available to view.</p>
{:else}
	<ul class="grid" aria-label="Proofs">
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
</style>
