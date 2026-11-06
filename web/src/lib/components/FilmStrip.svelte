<script lang="ts">
	let { class: className = '' }: { class?: string } = $props();

	// Two identical runs, so scrolling by exactly half the track loops without a jump.
	const frames = Array.from({ length: 12 }, (_, index) => index % 6);
	const holes = Array.from({ length: 136 }, (_, index) => index);
</script>

<div
	data-slot="film-strip"
	aria-hidden="true"
	class="film overflow-hidden bg-[#0f0c0a] [mask-image:linear-gradient(to_right,transparent,#000_12%,#000_88%,transparent)] {className}"
>
	<div class="track flex w-max">
		{#each [0, 1] as run (run)}
			<div class="grid">
				<div class="flex gap-2 px-1 py-1.5">
					{#each holes as hole (hole)}
						<span class="h-2 w-2.5 shrink-0 rounded-[1px] bg-inverse/15"></span>
					{/each}
				</div>
				<div class="flex gap-3 px-1">
					{#each frames as frame, index (index)}
						<div class="w-48 shrink-0">
							<div
								class="aspect-[3/2] rounded-[2px] bg-gradient-to-br from-[#2a2019] to-[#1c1511]"
							></div>
							<p
								class="pt-1 font-mono text-[10px] tracking-widest whitespace-nowrap text-autofocus/70"
							>
								▸ {frame + 1}
								{frame + 1}A{index % 2 === 1 ? ' PHOTOPROOFING 400' : ''}
							</p>
						</div>
					{/each}
				</div>
				<div class="flex gap-2 px-1 py-1.5">
					{#each holes as hole (hole)}
						<span class="h-2 w-2.5 shrink-0 rounded-[1px] bg-inverse/15"></span>
					{/each}
				</div>
			</div>
		{/each}
	</div>
</div>

<style>
	.track {
		animation: film-scroll 40s linear infinite;
	}
	@keyframes film-scroll {
		to {
			transform: translateX(-50%);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.track {
			animation: none;
		}
	}
</style>
