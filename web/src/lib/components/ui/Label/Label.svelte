<script lang="ts">
	import { cn, type WithElementRef } from '#lib/utils.js';
	import type { HTMLLabelAttributes } from 'svelte/elements';

	type Props = WithElementRef<HTMLLabelAttributes, HTMLLabelElement> & {
		srOnly?: boolean;
	};

	let {
		ref = $bindable(null),
		srOnly = false,
		class: className,
		children,
		...restProps
	}: Props = $props();
</script>

<label
	bind:this={ref}
	data-slot="label"
	class={cn('cn-label', srOnly && 'sr-only', className)}
	{...restProps}
>
	{@render children?.()}
</label>
