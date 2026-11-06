<script lang="ts">
	import { Heart } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	let {
		selected,
		label,
		onclick,
		disabled = false,
		class: className = '',
		children
	}: {
		selected: boolean;
		/** Accessible name; omit it when `children` is the visible label. */
		label?: string;
		onclick?: () => void;
		/** Shows the state without letting it change, as in the owner's preview. */
		disabled?: boolean;
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
	<span
		class="inline-grid {popping && selected ? 'animate-heart-pop' : ''}"
		onanimationend={() => (popping = false)}
	>
		<Heart aria-hidden="true" size={22} fill={selected ? 'currentColor' : 'none'} />
	</span>
	{@render children?.()}
</button>
