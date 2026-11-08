<script lang="ts">
	import { cn, type WithElementRef } from '#lib/utils.js';
	import type { HTMLAttributes } from 'svelte/elements';

	type Width = 'narrow' | 'default' | 'wide';

	const widths: Record<Width, string> = {
		narrow: 'max-w-sm',
		default: 'max-w-5xl',
		wide: 'max-w-6xl'
	};

	type Props = WithElementRef<HTMLAttributes<HTMLElement>> & { width?: Width };

	let {
		ref = $bindable(null),
		width = 'default',
		class: className,
		children,
		...restProps
	}: Props = $props();
</script>

<main
	bind:this={ref}
	data-slot="page-shell"
	class={cn('mx-auto grid gap-6 p-6', widths[width], className)}
	{...restProps}
>
	{@render children?.()}
</main>
