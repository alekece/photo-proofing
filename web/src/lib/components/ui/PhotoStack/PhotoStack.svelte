<script lang="ts">
	import { Check } from '@lucide/svelte';
	import { thumbnailUrl, type PhotoSet } from '#lib/gallery/photos.js';

	/** Up to three photos fanned like a stack; `done` flies them out and pops a check in. */
	let {
		galleryId,
		photoIds,
		set = 'photos',
		done = false
	}: { galleryId: string; photoIds: string[]; set?: PhotoSet; done?: boolean } = $props();

	const TILTS = ['-rotate-6', 'rotate-3', 'rotate-0'];
	let shown = $derived(photoIds.slice(0, 3));
</script>

<div
	data-slot="photo-stack"
	class="relative grid size-24 flex-none place-items-center"
	aria-hidden="true"
>
	<div class="contents" class:fly={done}>
		{#each shown as id, index (id)}
			<img
				src={thumbnailUrl(galleryId, id, set)}
				alt=""
				class="absolute size-[5.25rem] rounded-control border-2 border-panel object-cover shadow-md {TILTS[
					index
				]}"
			/>
		{:else}
			<span class="size-24 rounded-control border-2 border-dashed border-border-strong"></span>
		{/each}
	</div>
	{#if done}
		<span
			class="pop absolute grid size-16 place-items-center rounded-full bg-ok text-inverse"
			data-slot="photo-stack-check"
		>
			<Check size={32} />
		</span>
	{/if}
</div>

<style>
	.fly > :global(*) {
		animation: fly-out 240ms var(--ease-pop) forwards;
	}
	@keyframes fly-out {
		to {
			opacity: 0;
			transform: translateY(-3rem) rotate(8deg);
		}
	}
	.pop {
		animation: pop 320ms var(--ease-pop) 160ms backwards;
	}
	@keyframes pop {
		from {
			opacity: 0;
			transform: scale(0.4);
		}
	}
</style>
