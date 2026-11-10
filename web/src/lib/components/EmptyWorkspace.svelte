<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import scene from '#lib/assets/empty-dashboard.webp';
	import sceneStill from '#lib/assets/empty-dashboard-still.webp';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
</script>

<section class="px-3.5 pb-10 sm:px-8 sm:pb-14">
	<EmptyState>
		<picture>
			<source media="(prefers-reduced-motion: reduce)" srcset={sceneStill} />
			<img src={scene} alt="" width="512" height="256" class="h-auto w-full max-w-lg" />
		</picture>
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
	}
</style>
