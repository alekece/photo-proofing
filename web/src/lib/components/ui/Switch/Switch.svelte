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
		'relative inline-flex h-[1.375rem] w-[2.375rem] shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200 disabled:cursor-default disabled:opacity-50',
		checked ? 'bg-ok' : 'bg-[#d6cabb]',
		className
	)}
	onclick={() => {
		checked = !checked;
		onchange?.(checked);
	}}
	{...restProps}
>
	<span
		class="pointer-events-none size-4 rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0/0.2)] transition-transform duration-200 ease-pop {checked
			? 'translate-x-[1.375rem]'
			: 'translate-x-[0.1875rem]'}"
	></span>
</button>
