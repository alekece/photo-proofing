<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import CopyButton from '#lib/components/CopyButton.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import { cn } from '#lib/utils.js';

	let {
		link,
		label = m.copy_link_label(),
		class: className,
		inputClass,
		buttonClass,
		statusClass = 'basis-full text-xs'
	}: {
		link: string;
		label?: string;
		class?: string;
		inputClass?: string;
		buttonClass?: string;
		statusClass?: string;
	} = $props();

	let input = $state<HTMLInputElement | null>(null);
</script>

<div class={cn('flex flex-wrap gap-x-2 gap-y-2.5', className)}>
	<Input
		bind:ref={input}
		readonly
		value={link}
		aria-label={label}
		class={cn(
			'bg-muted/40 min-h-0 min-w-0 flex-1 cursor-default truncate rounded-lg border-border px-2.5 py-[0.5625rem] font-mono text-sm text-muted-foreground focus-visible:outline-none',
			inputClass
		)}
	/>
	<CopyButton value={link} fallbackInput={input} class={buttonClass} {statusClass} />
</div>
