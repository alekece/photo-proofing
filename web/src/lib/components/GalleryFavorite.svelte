<script lang="ts">
	import { Star } from '@lucide/svelte';
	import ClickRipple from './ClickRipple.svelte';
	import StatusChip from './ui/StatusChip/StatusChip.svelte';

	let {
		favorite,
		label,
		title = label,
		interactive = false,
		animate = false,
		onclick,
		class: className = ''
	}: {
		favorite: boolean;
		label: string;
		title?: string;
		interactive?: boolean;
		animate?: boolean;
		onclick?: () => void;
		class?: string;
	} = $props();

	let rippling = $state(false);

	function activate() {
		rippling = true;
		onclick?.();
	}
</script>

{#if interactive}
	<button
		type="button"
		data-slot="gallery-favorite"
		aria-label={label}
		aria-pressed={favorite}
		{title}
		onclick={activate}
		class="relative inline-flex cursor-pointer items-center justify-center pill border-0 bg-panel p-1.5 text-hero transition-opacity focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus {className}"
	>
		{#if rippling}<ClickRipple onend={() => (rippling = false)} />{/if}
		<Star
			class="size-3.5 {animate ? 'motion-safe:animate-heart-pop' : ''}"
			fill={favorite ? 'currentColor' : 'none'}
			aria-hidden="true"
		/>
	</button>
{:else}
	<StatusChip dataSlot="gallery-favorite" aria-label={label} {title} class={className}>
		<Star
			class="size-3.5 fill-current text-hero {animate ? 'motion-safe:animate-heart-pop' : ''}"
			fill="currentColor"
			aria-hidden="true"
		/>
	</StatusChip>
{/if}
