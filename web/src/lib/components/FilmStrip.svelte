<script lang="ts">
	let {
		count = 8,
		class: className = ''
	}: {
		/** How many blank frames one run of film has. */
		count?: number;
		/**
		 * Size and placement. `--h` sets the frame height and `--perf` the perforations' distance
		 * from the edge.
		 */
		class?: string;
	} = $props();

	const TINTS = [
		['#f2b27a', '#c86b4a'],
		['#e9c9a0', '#8a9a7a'],
		['#f0a48c', '#d9c2a0'],
		['#cfa98a', '#7e8ea0'],
		['#f6cf8f', '#b5644a'],
		['#e7b7a8', '#a3b08c'],
		['#f1c49a', '#9a7e6a'],
		['#dba27e', '#c9b48e']
	];
	const frames = $derived(
		Array.from({ length: count }, (_, index) => {
			const [from, to] = TINTS[index % TINTS.length];

			return {
				number: index + 1,
				print: index % 2 === 0,
				style: `--c:${from};--c2:${to};--x:${30 + ((index * 23) % 45)}%;--y:${40 + ((index * 17) % 35)}%;--d:${((index * 1.3) % 9).toFixed(1)}s`
			};
		})
	);
</script>

<!-- Two identical runs, so scrolling by exactly half the track loops without a jump. -->
<div
	data-slot="film-strip"
	aria-hidden="true"
	class="film relative overflow-hidden bg-[#100c0a] pt-[22px] pb-[30px] [--h:118px] @max-[700px]:[--h:84px] {className}"
>
	<div class="reel flex w-max gap-3">
		{#each [0, 1] as run (run)}
			{#each frames as frame (frame.number)}
				<div class="grid gap-[5px]">
					<div
						class="frame relative h-(--h) w-[calc(var(--h)*1.5)] overflow-hidden rounded-[3px] bg-[#211a15]"
					>
						<i class="absolute -inset-1/5 opacity-0 blur-[6px]" style={frame.style}></i>
					</div>
					<span
						class="w-[calc(var(--h)*1.5)] overflow-hidden font-mono text-[9px] leading-none font-semibold tracking-[0.14em] whitespace-nowrap text-[#d9792e] opacity-80"
						>▸&nbsp;{frame.number}&nbsp;&nbsp;&nbsp;{frame.number}A{frame.print
							? '\u00a0\u00a0\u00a0PHOTOPROOFING 400'
							: ''}</span
					>
				</div>
			{/each}
		{/each}
	</div>
</div>

<style>
	.film::before,
	.film::after {
		content: '';
		position: absolute;
		left: 0;
		right: 0;
		height: 8px;
		background: repeating-linear-gradient(
			90deg,
			transparent 0 9px,
			#2c241e 9px 19px,
			transparent 19px 26px
		);
	}
	.film::before {
		top: var(--perf, 7px);
	}
	.film::after {
		bottom: var(--perf, 7px);
	}
	.reel {
		animation: roll 60s linear infinite;
	}
	.frame i {
		background:
			radial-gradient(60% 55% at var(--x) var(--y), var(--c), transparent 70%),
			radial-gradient(40% 35% at calc(100% - var(--x)) 30%, var(--c2), transparent 70%);
		animation: develop 9s ease-in-out infinite;
		animation-delay: var(--d);
	}
	@keyframes roll {
		to {
			transform: translateX(-50%);
		}
	}
	@keyframes develop {
		0%,
		100% {
			opacity: 0;
			filter: blur(14px);
		}
		45%,
		60% {
			opacity: 0.6;
			filter: blur(5px);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.reel {
			animation: none;
		}
		.frame i {
			animation: none;
			opacity: 0.35;
		}
	}
</style>
