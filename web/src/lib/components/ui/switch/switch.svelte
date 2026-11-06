<script lang="ts">
	import { cn } from '#lib/utils.js';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	type Props = Omit<HTMLButtonAttributes, 'type' | 'role' | 'onchange'> & {
		checked?: boolean;
		onchange?: (checked: boolean) => void;
	};

	let { checked = $bindable(false), onchange, class: className, ...restProps }: Props = $props();
</script>

<button
	type="button"
	role="switch"
	aria-checked={checked}
	data-slot="switch"
	class={cn(
		'relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border border-border-strong transition-colors duration-200 disabled:cursor-default disabled:opacity-50',
		checked ? 'bg-foreground' : 'bg-transparent',
		className
	)}
	onclick={() => {
		checked = !checked;
		onchange?.(checked);
	}}
	{...restProps}
>
	<span
		class="pointer-events-none size-4 rounded-full transition-transform duration-200 ease-pop {checked
			? 'translate-x-6 bg-inverse'
			: 'translate-x-1 bg-foreground'}"
	></span>
</button>
