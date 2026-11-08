<script lang="ts">
	import { Heart } from '@lucide/svelte';
	import ClickPop from '#lib/components/ClickPop.svelte';
	import type { Snippet } from 'svelte';

	let {
		selected,
		label,
		onclick,
		disabled = false,
		size = 17,
		class: className = '',
		children
	}: {
		selected: boolean;
		/** Accessible name; omit it when `children` is the visible label. */
		label?: string;
		onclick?: () => void;
		/** Shows the state without letting it change, as in the owner's preview. */
		disabled?: boolean;
		/** Heart size in pixels. */
		size?: number;
		class?: string;
		/** Visible text beside the heart. */
		children?: Snippet;
	} = $props();

	// Only a click that favorites pops the heart, so photos that load as favorites stay still.
	let popping = $state(false);

	function click() {
		popping = !selected;
		onclick?.();
	}
</script>

<button
	type="button"
	class={className}
	aria-pressed={selected}
	aria-label={label}
	{disabled}
	onclick={click}
>
	<ClickPop active={popping && selected} onend={() => (popping = false)}>
		<Heart aria-hidden="true" {size} fill={selected ? 'currentColor' : 'none'} />
	</ClickPop>
	{@render children?.()}
</button>
