<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	// Two strips of blank frames; each repeats once so the slow roll loops without a jump.
	const strips = [1, 10].map((from) =>
		Array.from({ length: 18 }, (_, index) => {
			const frame = from + (index % 9);

			return frame % 3 ? `▸ ${frame}   ${frame}A` : 'PHOTOPROOFING 400';
		})
	);
</script>

<section class="grid justify-items-center px-3.5 pb-10 sm:px-8 sm:pt-1 sm:pb-14">
	<div
		aria-hidden="true"
		data-slot="contact-sheet"
		class="cs relative aspect-[4/3] w-[min(860px,100%)] overflow-hidden rounded-md sm:aspect-[2/1]"
	>
		<div class="sheet absolute inset-0" aria-hidden="true">
			{#each strips as labels, index (index)}
				<div class="reel {index === 0 ? 'r1' : 'r2'}">
					<div class="frames">
						{#each labels as label, frame (frame)}
							<div class="frame"><span>{label}</span></div>
						{/each}
					</div>
				</div>
			{/each}
		</div>
		<span class="leak" aria-hidden="true"></span>
		<div class="finder" aria-hidden="true">
			<i></i><i></i><i></i><i></i><b></b>
		</div>
		<p class="hud">
			<span>1/125</span><span>F2.8</span><span>ISO 400</span><span class="bad">[ 0 ]</span>
		</p>
	</div>
	<h2 class="mt-[22px] font-serif text-[1.3125rem] leading-[1.2] italic sm:text-[1.625rem]">
		{m.dashboard_empty()}
	</h2>
</section>

<style>
	.cs {
		--fw: 30cqw;
		container-type: inline-size;
		background:
			radial-gradient(60% 70% at 78% 18%, rgb(214 105 63 / 0.16), transparent 70%),
			radial-gradient(90% 90% at 45% 45%, #2e241d, #16120f 72%);
		box-shadow: 0 30px 60px -32px rgb(43 36 32 / 0.8);
	}
	@media (min-width: 640px) {
		.cs {
			--fw: 17cqw;
		}
	}
	.cs::after {
		content: '';
		position: absolute;
		inset: 0;
		background: radial-gradient(75% 75% at 50% 48%, transparent 55%, rgb(0 0 0 / 0.55));
		pointer-events: none;
	}
	.sheet {
		animation: breathe 7s ease-in-out infinite;
	}
	.reel {
		position: absolute;
		left: -12%;
		width: 124%;
		overflow: hidden;
		background: #100c0a;
		padding: calc(var(--fw) * 0.17) 0 calc(var(--fw) * 0.24);
		box-shadow: 0 18px 30px -18px #000;
	}
	.reel::before,
	.reel::after {
		content: '';
		position: absolute;
		left: 0;
		right: 0;
		height: calc(var(--fw) * 0.055);
		background: repeating-linear-gradient(
			90deg,
			transparent 0 calc(var(--fw) * 0.07),
			#2c241e calc(var(--fw) * 0.07) calc(var(--fw) * 0.14),
			transparent calc(var(--fw) * 0.14) calc(var(--fw) * 0.19)
		);
	}
	.reel::before {
		top: calc(var(--fw) * 0.05);
	}
	.reel::after {
		bottom: calc(var(--fw) * 0.05);
	}
	.r1 {
		top: 5%;
		transform: rotate(-3deg);
	}
	.r2 {
		bottom: 15%;
		transform: rotate(2deg);
	}
	@media (max-width: 639px) {
		.r1 {
			top: 4%;
		}
		.r2 {
			bottom: 18%;
		}
	}
	.frames {
		display: flex;
		gap: calc(var(--fw) * 0.09);
		width: max-content;
		animation: roll 160s linear infinite;
	}
	.r2 .frames {
		animation-direction: reverse;
		animation-duration: 200s;
	}
	.frame {
		position: relative;
		width: var(--fw);
		aspect-ratio: 3 / 2;
		border-radius: 2px;
		background: linear-gradient(160deg, #241c16, #1a1410);
		box-shadow: inset 0 0 0 1px rgb(255 240 220 / 0.04);
	}
	.frame span {
		position: absolute;
		left: 0;
		bottom: calc(var(--fw) * -0.15);
		font:
			600 calc(var(--fw) * 0.062) / 1 ui-monospace,
			Menlo,
			monospace;
		letter-spacing: 0.14em;
		color: #d9792e;
		opacity: 0.75;
		white-space: pre;
	}
	.leak {
		position: absolute;
		inset: -10% -40%;
		background: linear-gradient(
			105deg,
			transparent 38%,
			rgb(232 110 50 / 0.26) 47%,
			rgb(255 206 140 / 0.2) 52%,
			transparent 61%
		);
		mix-blend-mode: screen;
		pointer-events: none;
		animation: sweep-leak 13s ease-in-out infinite;
		opacity: 0;
	}
	.finder {
		position: absolute;
		left: 50%;
		top: 44%;
		width: 38cqw;
		aspect-ratio: 3 / 2;
		transform: translate(-50%, -50%);
		z-index: 1;
	}
	@media (max-width: 639px) {
		.finder {
			width: 60cqw;
			top: 46%;
		}
	}
	.finder i {
		position: absolute;
		width: 3.4cqw;
		height: 3.4cqw;
		border: 0 solid rgb(239 230 218 / 0.82);
	}
	.finder i:nth-child(1) {
		left: 0;
		top: 0;
		border-width: 2px 0 0 2px;
	}
	.finder i:nth-child(2) {
		right: 0;
		top: 0;
		border-width: 2px 2px 0 0;
	}
	.finder i:nth-child(3) {
		left: 0;
		bottom: 0;
		border-width: 0 0 2px 2px;
	}
	.finder i:nth-child(4) {
		right: 0;
		bottom: 0;
		border-width: 0 2px 2px 0;
	}
	.finder b {
		position: absolute;
		left: 50%;
		top: 50%;
		width: 6cqw;
		height: 4.2cqw;
		margin: -2.1cqw 0 0 -3cqw;
		border: 1.5px solid rgb(239 230 218 / 0.8);
		animation: lock 7s ease-in-out infinite;
	}
	.finder b::after {
		content: '';
		position: absolute;
		left: 50%;
		top: 50%;
		width: 3px;
		height: 3px;
		margin: -1.5px 0 0 -1.5px;
		border-radius: 50%;
		background: rgb(239 230 218 / 0.8);
	}
	.hud {
		position: absolute;
		left: 0;
		right: 0;
		bottom: 4.5%;
		z-index: 1;
		display: flex;
		justify-content: center;
		gap: 2.6cqw;
		margin: 0;
		font:
			max(9px, 1.45cqw) ui-monospace,
			Menlo,
			monospace;
		color: #cfc3b5;
		letter-spacing: 0.08em;
		font-variant-numeric: tabular-nums;
	}
	.hud .bad {
		color: var(--color-warning-on-dark);
		font-weight: 700;
	}
	@keyframes breathe {
		0%,
		100% {
			filter: blur(0);
			transform: scale(1);
		}
		50% {
			filter: blur(1.4px);
			transform: scale(1.015);
		}
	}
	@keyframes roll {
		to {
			transform: translateX(-50%);
		}
	}
	@keyframes sweep-leak {
		0% {
			transform: translateX(-45%);
			opacity: 0;
		}
		12% {
			opacity: 1;
		}
		50%,
		100% {
			transform: translateX(45%);
			opacity: 0;
		}
	}
	@keyframes lock {
		0%,
		100% {
			transform: scale(1);
			border-color: rgb(239 230 218 / 0.8);
		}
		50% {
			transform: scale(1.18);
			border-color: var(--color-warning-on-dark);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.sheet,
		.frames,
		.leak,
		.finder b {
			animation: none;
		}
	}
</style>
