<script lang="ts">
	import { Ellipsis } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	import Popover from './ui/Popover/Popover.svelte';

	let { notes }: { notes: string } = $props();

	let text = $state<HTMLElement | null>(null);
	let truncated = $state(false);

	$effect(() => {
		if (!text || typeof ResizeObserver === 'undefined') return;
		const node = text;
		const measure = () => (truncated = node.scrollHeight > node.clientHeight + 1);

		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(node);

		return () => observer.disconnect();
	});
</script>

<div class="min-w-0">
	<p
		bind:this={text}
		class="max-h-20 overflow-hidden text-justify text-sm [overflow-wrap:anywhere] text-muted-foreground"
	>
		{notes}
	</p>
	{#if truncated}
		<Popover
			placement="auto"
			label={m.contact_notes_full()}
			class="overflow-count -ml-2 min-h-8! min-w-8! sm:icon-action"
			rootClass="inline-block"
			panelClass="w-max min-w-0 max-w-[min(24rem,calc(100vw-3.5rem))] max-h-[var(--popover-space,50dvh)] overflow-auto p-3"
		>
			{#snippet trigger()}<Ellipsis size={16} aria-hidden="true" />{/snippet}
			<p class="text-sm [overflow-wrap:anywhere] whitespace-pre-line text-foreground">{notes}</p>
		</Popover>
	{/if}
</div>
