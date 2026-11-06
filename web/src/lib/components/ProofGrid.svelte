<script lang="ts">
	import type { Proof } from '../gallery/types';
	import ProofThumbnail from './ProofThumbnail.svelte';

	let {
		proofs,
		layout,
		isSelected = () => false,
		readonly = false,
		onopen,
		onselect
	}: {
		proofs: Proof[];
		layout: 'grid' | 'masonry';
		isSelected?: (id: string) => boolean;
		readonly?: boolean;
		onopen: (index: number) => void;
		onselect?: (id: string) => void;
	} = $props();
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
				<ProofThumbnail
					{proof}
					{layout}
					position={i + 1}
					total={proofs.length}
					selected={isSelected(proof.id)}
					{readonly}
					onopen={() => onopen(i)}
					onselect={onselect && (() => onselect(proof.id))}
				/>
			</li>
		{/each}
	</ul>
{/if}
