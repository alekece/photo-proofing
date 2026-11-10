<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';

	const frames = ['f1', 'f2', 'f3', 'f4'];
</script>

<section class="px-3.5 pb-10 sm:px-8 sm:pb-14">
	<EmptyState>
		<div class="stage" aria-hidden="true">
			{#each frames as frame (frame)}
				<div class="frame {frame}"><div class="photo"></div></div>
			{/each}
			<svg class="camera" viewBox="0 0 120 100" fill="none">
				<rect x="6" y="24" width="108" height="66" rx="12" fill="#2a2420" />
				<path d="M38 24l6-12h32l6 12z" fill="#4a3f37" />
				<rect x="12" y="32" width="96" height="10" rx="5" fill="#4a3f37" />
				<circle cx="60" cy="58" r="26" fill="#4a3f37" />
				<g class="iris">
					<circle cx="60" cy="58" r="19" fill="#1d3550" />
					<circle cx="60" cy="58" r="12" fill="#3f6f95" />
					<circle cx="54" cy="52" r="4" fill="#fff" opacity=".7" />
				</g>
				<circle cx="97" cy="36" r="3" fill="#c4663a" />
				<circle class="flash" cx="97" cy="36" r="9" fill="#fff3c4" opacity=".9" />
			</svg>
		</div>
		<h2 class="font-serif text-[1.625rem] text-foreground sm:text-[1.75rem]">
			{m.dashboard_empty()}
		</h2>
		<p class="puns">
			<span>{m.dashboard_empty_pun_1()}</span>
			<span aria-hidden="true">{m.dashboard_empty_pun_2()}</span>
			<span aria-hidden="true">{m.dashboard_empty_pun_3()}</span>
		</p>
	</EmptyState>
</section>

<style>
	.stage {
		position: relative;
		width: min(100%, 32rem);
		height: 16rem;
	}

	.frame {
		position: absolute;
		width: var(--w);
		padding: 6px 6px 20px;
		background: #fff;
		box-shadow:
			0 10px 24px rgb(42 36 32 / 0.18),
			0 1px 3px rgb(42 36 32 / 0.13);
		transform: rotate(var(--r));
		animation: drift var(--d) ease-in-out infinite;
		animation-delay: var(--dl);
	}

	.photo {
		position: relative;
		aspect-ratio: 1;
		overflow: hidden;
		background: #e9e4d8;
	}

	/* The photo emerges once, slowly and blurry, like an instant print, then stays. */
	.photo::before {
		content: '';
		position: absolute;
		inset: 0;
		background: var(--g);
		animation: develop 10s ease-in-out both;
		animation-delay: var(--dev);
	}

	.f1 {
		--w: 92px;
		--r: -9deg;
		--d: 7s;
		--dl: 0s;
		--dev: 1s;
		--g: linear-gradient(160deg, #f4b183, #c4663a 55%, #6b3a5b);
		left: 6%;
		top: 6%;
	}

	.f2 {
		--w: 76px;
		--r: 7deg;
		--d: 8s;
		--dl: -2s;
		--dev: 4s;
		--g: linear-gradient(160deg, #a8c8d8, #4f7f9a 55%, #25384d);
		right: 7%;
		top: 0;
	}

	.f3 {
		--w: 70px;
		--r: 5deg;
		--d: 9s;
		--dl: -4s;
		--dev: 7s;
		--g: linear-gradient(160deg, #e8d28a, #b98a3e 55%, #5c4326);
		left: 14%;
		bottom: 0;
	}

	.f4 {
		--w: 84px;
		--r: -6deg;
		--d: 7.5s;
		--dl: -1s;
		--dev: 10s;
		--g: linear-gradient(160deg, #c9d6a8, #6f8f5a 55%, #2f4a3a);
		right: 12%;
		bottom: 4%;
	}

	.camera {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 7.5rem;
		filter: drop-shadow(0 8px 12px rgb(42 36 32 / 0.2));
		animation: bob 4s ease-in-out infinite;
	}

	.iris {
		transform-origin: 60px 58px;
		animation: iris 8s infinite;
	}

	.flash {
		opacity: 0;
		animation: flash 8s infinite;
	}

	.puns {
		position: relative;
		width: 100%;
		height: 1.5em;
	}

	.puns span {
		position: absolute;
		inset: 0;
		opacity: 0;
		animation: pun 12s infinite;
	}

	.puns span:nth-child(2) {
		animation-delay: 4s;
	}

	.puns span:nth-child(3) {
		animation-delay: 8s;
	}

	@keyframes drift {
		0%,
		100% {
			transform: translateY(0) rotate(var(--r));
		}
		50% {
			transform: translateY(-12px) rotate(calc(var(--r) * -0.6));
		}
	}

	@keyframes develop {
		0% {
			opacity: 0;
			filter: blur(8px) saturate(0) brightness(1.7);
		}
		15% {
			opacity: 0.05;
		}
		100% {
			opacity: 1;
			filter: none;
		}
	}

	@keyframes bob {
		0%,
		100% {
			transform: translate(-50%, -50%);
		}
		50% {
			transform: translate(-50%, -56%);
		}
	}

	@keyframes iris {
		0%,
		22%,
		40%,
		100% {
			transform: scale(1);
		}
		28% {
			transform: scale(0.45);
		}
		34% {
			transform: scale(1.08);
		}
	}

	@keyframes flash {
		0%,
		28%,
		100% {
			opacity: 0;
		}
		30% {
			opacity: 1;
		}
		40% {
			opacity: 0;
		}
	}

	@keyframes pun {
		0% {
			opacity: 0;
			transform: translateY(8px);
		}
		4%,
		29% {
			opacity: 1;
			transform: none;
		}
		33%,
		100% {
			opacity: 0;
			transform: translateY(-8px);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.puns span:first-child {
			opacity: 1;
			animation: none;
		}

		.camera,
		.frame {
			animation: none;
		}
	}
</style>
