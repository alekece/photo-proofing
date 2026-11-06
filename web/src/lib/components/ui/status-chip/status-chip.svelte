<script module lang="ts">
	export type GalleryStatus = 'draft' | 'shared' | 'delivered' | 'expired' | 'archived';

	/** The colour of each status's dot, shared with the dashboard's status filter. */
	export const statusDots: Record<GalleryStatus, string> = {
		draft: 'bg-muted-foreground',
		shared: 'bg-info',
		delivered: 'bg-ok',
		expired: 'bg-danger',
		archived: 'bg-border-strong'
	};
</script>

<script lang="ts">
	import { cn } from '#lib/utils.js';
	import type { HTMLAttributes } from 'svelte/elements';

	const labels: Record<GalleryStatus, string> = {
		draft: 'Draft',
		shared: 'Shared',
		delivered: 'Delivered',
		expired: 'Expired',
		archived: 'Archived'
	};

	const tones: Record<GalleryStatus, string> = {
		draft: 'border-border-strong text-muted-foreground',
		shared: 'border-foreground text-foreground',
		delivered: 'border-ok text-ok',
		expired: 'border-danger text-danger',
		archived: 'border-border-strong bg-border text-muted-foreground'
	};

	type Props = HTMLAttributes<HTMLSpanElement> & { status: GalleryStatus; dot?: boolean };

	let { status, dot = false, class: className, ...restProps }: Props = $props();
</script>

<span
	data-slot="status-chip"
	class={cn(
		'inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium tracking-wide',
		tones[status],
		className
	)}
	{...restProps}
>
	{#if dot}<span
			data-slot="status-chip-dot"
			aria-hidden="true"
			class={cn('size-1.5 rounded-full', statusDots[status])}
		></span>{/if}
	{labels[status]}
</span>
