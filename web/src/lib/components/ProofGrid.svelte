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
			? 'block [columns:3_240px] gap-x-2.5 px-7 pt-3.5 pb-10 @max-[700px]:columns-2 @max-[700px]:gap-x-1.5 @max-[700px]:px-2 @max-[700px]:pt-2 @max-[700px]:pb-[1.875rem]'
			: 'grid grid-cols-[repeat(auto-fill,minmax(min(100%,15rem),1fr))] gap-2.5 px-7 pt-3.5 pb-10 @max-[700px]:grid-cols-2 @max-[700px]:gap-1.5 @max-[700px]:px-2 @max-[700px]:pt-2 @max-[700px]:pb-[1.875rem]'}
		data-layout={layout}
		aria-label="Proofs"
	>
		{#each proofs as proof, i (proof.id)}
			<li
				class={layout === 'masonry' ? 'mb-2.5 break-inside-avoid @max-[700px]:mb-1.5' : undefined}
			>
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
