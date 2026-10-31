<script lang="ts">
	import { cn } from '#lib/utils.js';

	let {
		background,
		icon = false,
		label,
		loading = false,
		class: className
	}: {
		background: 'sheet' | 'gradient';
		icon?: boolean;
		label?: string;
		loading?: boolean;
		class?: string;
	} = $props();
	let shownLabel = $derived(loading ? undefined : label);
</script>

<div
	class={cn(
		'relative h-full w-full overflow-hidden',
		background === 'gradient' && 'bg-gradient-to-br from-violet-200 via-rose-100 to-sky-200',
		background === 'sheet' && 'bg-placeholder-sheet',
		loading && 'pp-breathe',
		className
	)}
	data-background={background}
	aria-hidden={shownLabel ? undefined : 'true'}
>
	{#if background === 'sheet'}
		<div class="absolute inset-3 grid grid-cols-3 grid-rows-2 gap-2" aria-hidden="true">
			<div class="col-span-2 row-span-2 rounded-control bg-placeholder-tile"></div>
			<div class="rounded-control bg-placeholder-tile-strong/70"></div>
			<div class="rounded-control bg-placeholder-tile/80"></div>
		</div>
	{/if}
	{#if (icon || shownLabel) && !loading}
		<div
			class="absolute inset-0 flex flex-col items-center justify-center gap-2 p-4 text-center text-sm text-placeholder-foreground"
		>
			{#if icon}
				<svg
					class="size-8"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
					aria-hidden="true"
					data-testid="placeholder-icon"
				>
					<rect x="3" y="4" width="18" height="16" rx="2" />
					<circle cx="9" cy="10" r="1.5" />
					<path d="m21 16-5-5-8 9" />
				</svg>
			{/if}
			{#if shownLabel}<span>{shownLabel}</span>{/if}
		</div>
	{/if}
</div>

<style>
	.pp-breathe {
		animation: pp-breathe 2.4s ease-in-out infinite;
	}
	@keyframes pp-breathe {
		0%,
		100% {
			filter: brightness(1.04);
		}
		50% {
			filter: brightness(0.9);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.pp-breathe {
			animation: none;
		}
	}
</style>
