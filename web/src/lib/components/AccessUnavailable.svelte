<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';

	// x, y and size as percentages of the viewfinder, then the bokeh color.
	const BOKEH = [
		['22%', '58%', '34%', '#f2b27a'],
		['8%', '20%', '22%', '#e9c9a0'],
		['70%', '24%', '28%', '#f0a48c'],
		['55%', '62%', '18%', '#cfa98a'],
		['38%', '12%', '14%', '#f6cf8f'],
		['80%', '66%', '20%', '#dba27e'],
		['30%', '40%', '12%', '#e7b7a8']
	];
	const CORNERS = [
		'top-4 left-4 border-t-2 border-l-2',
		'top-4 right-4 border-t-2 border-r-2',
		'bottom-4 left-4 border-b-2 border-l-2',
		'bottom-4 right-4 border-r-2 border-b-2'
	];
</script>

<main
	class="flex flex-1 flex-col items-center justify-center gap-[34px] bg-dark px-6 py-10 text-[#efe6da]"
>
	<div
		role="img"
		aria-label={m.unavailable_viewfinder()}
		class="relative aspect-3/2 w-[min(640px,100%)] overflow-hidden rounded-[4px] bg-[#1c1612] max-[700px]:aspect-3/4"
	>
		<div class="bokeh absolute inset-0">
			{#each BOKEH as [x, y, size, color] (x)}
				<b
					class="absolute rounded-full opacity-55 mix-blend-screen"
					style="left:{x};top:{y};width:{size};height:{size};background:radial-gradient(circle,{color} 0,{color} 55%,transparent 72%)"
				></b>
			{/each}
		</div>
		{#each CORNERS as corner (corner)}
			<span class="absolute size-[26px] border-[rgb(239_230_218/0.7)] {corner}"></span>
		{/each}
		<div
			class="seek absolute top-[38%] left-[42%] h-[12%] w-[16%] border-2 border-[rgb(239_230_218/0.85)]"
		></div>
		<div
			class="absolute inset-x-0 bottom-4 flex items-center justify-center gap-[18px] font-mono text-xs tracking-[0.08em] text-[#cfc3b5] max-[700px]:gap-2.5 max-[700px]:text-[0.625rem]"
		>
			<span>1/125</span>
			<span>F2</span>
			<span class="scale relative flex gap-4 pt-[9px]">
				0.5&nbsp;&nbsp;1&nbsp;&nbsp;3&nbsp;&nbsp;10&nbsp;&nbsp;∞<i
					class="needle absolute -top-1 left-[10%] h-3 w-0.5 bg-warning-on-dark"
				></i>
			</span>
			<span class="bad font-bold text-warning-on-dark">AF</span>
		</div>
	</div>
	<div class="text-center">
		<h1 class="font-serif text-[clamp(28px,4vw,40px)] font-normal italic">
			{m.unavailable_title()}
		</h1>
		<p class="mt-2 text-[#bfb2a4]">
			{m.unavailable_body()}
		</p>
	</div>
	<p class="text-xs tracking-[0.06em] text-muted-foreground">PhotoProofing</p>
</main>

<style>
	.bokeh {
		animation: hunt 3.2s ease-in-out infinite;
	}
	.seek {
		animation: seek 3.2s ease-in-out infinite;
	}
	.needle {
		animation: needle 3.2s ease-in-out infinite;
	}
	.bad {
		animation: blink 3.2s steps(1) infinite;
	}
	.scale::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 5px;
		background: repeating-linear-gradient(90deg, #cfc3b5 0 1px, transparent 1px 8px);
		opacity: 0.6;
	}
	@keyframes hunt {
		0%,
		100% {
			filter: blur(16px);
			transform: scale(1.06);
		}
		22% {
			filter: blur(3px);
			transform: scale(1);
		}
		30% {
			filter: blur(9px);
			transform: scale(1.03);
		}
		55% {
			filter: blur(1.5px);
			transform: scale(0.99);
		}
		64% {
			filter: blur(12px);
			transform: scale(1.05);
		}
		82% {
			filter: blur(5px);
			transform: scale(1.01);
		}
	}
	@keyframes seek {
		0%,
		100% {
			transform: translate(0, 0);
			border-color: rgb(239 230 218 / 0.85);
		}
		22% {
			transform: translate(-120%, 30%);
		}
		30% {
			transform: translate(-120%, 30%);
			border-color: var(--color-warning-on-dark);
		}
		55% {
			transform: translate(110%, -40%);
			border-color: rgb(239 230 218 / 0.85);
		}
		64% {
			transform: translate(110%, -40%);
			border-color: var(--color-warning-on-dark);
		}
		82% {
			transform: translate(20%, 60%);
			border-color: rgb(239 230 218 / 0.85);
		}
	}
	@keyframes needle {
		0%,
		100% {
			left: 85%;
		}
		22% {
			left: 30%;
		}
		30% {
			left: 40%;
		}
		55% {
			left: 12%;
		}
		64% {
			left: 70%;
		}
		82% {
			left: 48%;
		}
	}
	@keyframes blink {
		0%,
		29%,
		36%,
		63%,
		70%,
		100% {
			opacity: 0.35;
		}
		30%,
		64% {
			opacity: 1;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.bokeh,
		.seek,
		.needle,
		.bad {
			animation: none;
		}
	}
</style>
