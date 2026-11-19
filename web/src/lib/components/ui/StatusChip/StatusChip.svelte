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

	const workspaceTones: Partial<Record<GalleryStatus, string>> = {
		draft: 'border-border-strong bg-border text-muted-foreground',
		shared: 'border-workspace-green bg-inverse text-workspace-green',
		delivered: 'border-workspace-green bg-workspace-green text-inverse',
		archived: 'border-foreground bg-foreground text-inverse'
	};

	type Props = HTMLAttributes<HTMLSpanElement> & {
		status: GalleryStatus;
		dot?: boolean;
		cover?: boolean;
		/** The photographer workspace treatment; other consumers keep their existing palette. */
		workspace?: boolean;
	};

	let {
		status,
		dot = false,
		cover = false,
		workspace = false,
		class: className,
		...restProps
	}: Props = $props();
	const workspaceTone = $derived(workspace ? workspaceTones[status] : undefined);
</script>

<span
	data-slot="status-chip"
	class={cn(
		'inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium tracking-wide',
		workspaceTone ??
			(cover
				? `border-transparent bg-panel/90 font-semibold ${coverTones[status]}`
				: tones[status]),
		className
	)}
	{...restProps}
>
	{#if dot}<span
			data-slot="status-chip-dot"
			aria-hidden="true"
			class={cn(
				'size-[7px] rounded-full',
				workspaceTone || cover ? 'bg-current' : statusDots[status]
			)}
		></span>{/if}
	{STATE_LABELS[status]()}
</span>
