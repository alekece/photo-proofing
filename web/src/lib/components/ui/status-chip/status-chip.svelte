<script lang="ts">
	import { cn } from '#lib/utils.js';
	import type { HTMLAttributes } from 'svelte/elements';

	export type GalleryStatus = 'draft' | 'shared' | 'delivered' | 'expired' | 'archived';

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

	type Props = HTMLAttributes<HTMLSpanElement> & { status: GalleryStatus };

	let { status, class: className, ...restProps }: Props = $props();
</script>

<span
	data-slot="status-chip"
	class={cn(
		'inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium tracking-wide',
		tones[status],
		className
	)}
	{...restProps}
>
	{labels[status]}
</span>
