<script module lang="ts">
	export type GalleryStatus = 'draft' | 'shared' | 'delivered' | 'expired' | 'archived';

	/** The colour of each status's dot, shared with the dashboard's status filter. */
	export const statusDots: Record<GalleryStatus, string> = {
		draft: 'bg-muted-foreground',
		shared: 'bg-info',
		delivered: 'bg-ok',
		expired: 'bg-warning',
		archived: 'bg-muted-foreground'
	};
</script>

<script lang="ts">
	import { STATE_LABELS } from '#lib/gallery/states.js';
	import { cn } from '#lib/utils.js';
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	const tones: Record<GalleryStatus, string> = {
		draft: 'border-border-strong text-muted-foreground',
		shared: 'border-foreground text-foreground',
		delivered: 'border-ok text-ok',
		expired: 'border-danger text-danger',
		archived: 'border-border-strong bg-border text-muted-foreground'
	};

	/** On a photo: no outline, a translucent panel behind, and a text colour per status. */
	const coverTones: Record<GalleryStatus, string> = {
		draft: 'text-muted-foreground',
		shared: 'text-info',
		delivered: 'text-ok',
		expired: 'text-warning',
		archived: 'text-muted-foreground'
	};

	type Props = HTMLAttributes<HTMLSpanElement> & {
		dot?: boolean;
		cover?: boolean;
		/** The photographer workspace treatment; other consumers keep their existing palette. */
		workspace?: boolean;
		dataSlot?: string;
	} & (
			| { status: GalleryStatus; children?: never }
			| {
					status?: never;
					children: Snippet;
					'aria-label': string;
			  }
		);

	let {
		status,
		children,
		dot = false,
		cover = false,
		workspace = false,
		dataSlot = 'status-chip',
		class: className,
		role,
		...restProps
	}: Props = $props();
</script>

<span
	data-slot={dataSlot}
	role={role ?? (children ? 'img' : undefined)}
	class={cn(
		'inline-flex items-center justify-center pill',
		status ? 'gap-1.5 border text-xs font-medium tracking-wide' : 'p-1.5',
		workspace || !status
			? 'border-0 bg-panel text-foreground'
			: cover
				? `border-transparent bg-panel/90 font-semibold ${coverTones[status]}`
				: tones[status],
		className
	)}
	{...restProps}
>
	{#if status}
		{#if dot}<span
				data-slot="status-chip-dot"
				aria-hidden="true"
				class={cn(
					'size-[7px] rounded-full',
					workspace || cover ? 'bg-current' : statusDots[status]
				)}
			></span>{/if}
		{STATE_LABELS[status]()}
	{:else}
		{@render children?.()}
	{/if}
</span>
