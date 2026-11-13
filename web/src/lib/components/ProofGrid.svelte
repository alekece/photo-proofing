<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { GridDensity, GridSpacing, Proof } from '../gallery/types';
	import ProofThumbnail from './ProofThumbnail.svelte';

	let {
		proofs,
		layout,
		density = 'compact',
		spacing = 'compact',
		isSelected = () => false,
		readonly = false,
		onopen,
		onselect,
		commentCount
	}: {
		proofs: Proof[];
		layout: 'grid' | 'masonry';
		density?: GridDensity;
		spacing?: GridSpacing;
		isSelected?: (id: string) => boolean;
		readonly?: boolean;
		onopen: (index: number) => void;
		onselect?: (id: string) => void;
		commentCount?: (id: string) => number;
	} = $props();

	const listClass = $derived(
		layout === 'masonry'
			? [
					'block px-7 pt-3.5 pb-10 @max-[700px]:px-2 @max-[700px]:pt-2 @max-[700px]:pb-[1.875rem]',
					density === 'large'
						? '[columns:2_26rem] @max-[700px]:columns-1'
						: '[columns:2_19rem] @max-[700px]:columns-2 @min-[1400px]:[columns:3_19rem]',
					spacing === 'generous'
						? 'gap-x-5 @max-[700px]:gap-x-3.5'
						: 'gap-x-2.5 @max-[700px]:gap-x-1.5'
				].join(' ')
			: [
					'grid px-7 pt-3.5 pb-10 @max-[700px]:px-2 @max-[700px]:pt-2 @max-[700px]:pb-[1.875rem]',
					density === 'large'
						? 'grid-cols-[repeat(auto-fill,minmax(min(100%,22rem),1fr))] @max-[700px]:grid-cols-1'
						: 'grid-cols-[repeat(auto-fill,minmax(min(100%,15rem),1fr))] @max-[700px]:grid-cols-2',
					spacing === 'generous' ? 'gap-5 @max-[700px]:gap-3.5' : 'gap-2.5 @max-[700px]:gap-1.5'
				].join(' ')
	);

	const itemClass = $derived(
		layout === 'masonry'
			? spacing === 'generous'
				? 'mb-5 break-inside-avoid @max-[700px]:mb-3.5'
				: 'mb-2.5 break-inside-avoid @max-[700px]:mb-1.5'
			: undefined
	);
</script>

{#if proofs.length === 0}
	<p>{m.grid_empty()}</p>
{:else}
	<ul
		class={listClass}
		data-layout={layout}
		data-density={density}
		data-spacing={spacing}
		aria-label={m.grid_label()}
	>
		{#each proofs as proof, i (proof.id)}
			<li class={itemClass}>
				<ProofThumbnail
					{proof}
					{layout}
					position={i + 1}
					total={proofs.length}
					selected={isSelected(proof.id)}
					{readonly}
					commentCount={commentCount?.(proof.id) ?? 0}
					onopen={() => onopen(i)}
					onselect={onselect && (() => onselect(proof.id))}
				/>
			</li>
		{/each}
	</ul>
{/if}
