<script lang="ts">
	import { Check } from '@lucide/svelte';
	import { thumbnailUrl, type PhotoSet } from '#lib/gallery/photos.js';

	/** Up to three photos fanned like a stack; `done` flies them out and pops a check in. */
	let {
		galleryId,
		photoIds,
		set = 'photos',
		done = false,
		polaroid = false
	}: {
		galleryId: string;
		photoIds: string[];
		set?: PhotoSet;
		done?: boolean;
		/** Up to six photos piled as white-framed prints instead of three fanned thumbnails. */
		polaroid?: boolean;
	} = $props();

	const TILTS = ['-rotate-6', 'rotate-3', 'rotate-0'];
	const PRINT_POSES = [
		'-rotate-12 -translate-x-3 -translate-y-1',
		'rotate-9 translate-x-3 -translate-y-2',
		'-rotate-6 -translate-x-2 translate-y-2',
		'rotate-12 translate-x-3 translate-y-1.5',
		'-rotate-3 -translate-x-1 -translate-y-0.5',
		'rotate-3 translate-x-1 translate-y-0.5'
	];
	let shown = $derived(photoIds.slice(0, polaroid ? 6 : 3));
</script>

<div
	data-slot="photo-stack"
	class="relative grid size-24 flex-none place-items-center justify-self-center"
	aria-hidden="true"
>
	<div class="contents" class:fly={done}>
		{#each shown as id, index (id)}
			<img
				src={thumbnailUrl(galleryId, id, set)}
				alt=""
				class={polaroid
					? `absolute size-[4.25rem] border-x-[3px] border-t-[3px] border-b-4 border-white bg-white object-cover shadow-md ${PRINT_POSES[index]}`
					: `absolute size-[5.25rem] rounded-control border-2 border-panel object-cover shadow-md ${TILTS[index]}`}
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
		animation: fly-out 220ms ease-in forwards;
	}
	@keyframes fly-out {
		to {
			opacity: 0;
			scale: 0.85;
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
